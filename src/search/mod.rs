use std::cmp::Reverse;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::{
    Color, Move, MoveGenType, MoveList, Piece, PieceType, Position, Score, generate_moves, score,
};
use crate::tt_value;
use crate::uci::{RunningStatus, UciInfo, UciScore, UciScoreBound};

mod history_moves;
mod killer_moves;
mod transposition_table;

pub use history_moves::*;
pub use killer_moves::*;
pub use transposition_table::*;

/// Tracks search extension and move exclusion parameters for the current
/// branch.
#[derive(Copy, Clone, Debug, Default)]
struct SearchContext {
    pub excluded_move: Move,
    pub extensions: u8,
}

/// Shared context parameters passed down the recursive search stack.
pub struct Searcher<'a> {
    pos: Position,
    /// Track the running status of the search, may be stopped in another
    /// thread.
    keep_running: Arc<RunningStatus>,
    /// Tracks total nodes searched during this `go` invocation.
    nodes: u64,
    /// The moment when the search was started.
    start_time: Instant,
    /// Absolute time budget allowed for this search ply.
    allocated_time: Option<std::time::Duration>,
    /// Reference to the transposition table.
    transposition_table: &'a mut TranspositionTable,
    /// Current search sequence age.
    age: u8,
    /// Killer moves tracked per ply to sort high-quality quiet moves.
    killer_moves: KillerMoves,
    /// History heuristic table to prioritize frequently successful quiet moves.
    history_moves: &'a mut HistoryMoves,
    /// Tracks the maximum search depth reached, including quiescence.
    max_ply: u8,
}

pub struct SearcherParameters<'a> {
    // We take ownership of the position here since we will be modifying it during search.
    pub pos: Position,
    // The time limit for this search
    pub allocated_time: Option<Duration>,
    // The stop flag to signal search interruption from another thread
    pub keep_running: Arc<RunningStatus>,
    // The max depth to search to
    pub max_depth: i8,
    // The transposition table to use for caching search results
    //
    // See: https://www.chessprogramming.org/Transposition_Table
    pub transposition_table: &'a mut TranspositionTable,
    // The history table for quiet moves
    //
    // See: https://www.chessprogramming.org/History_Heuristic
    pub history_moves: &'a mut HistoryMoves,
    // The current search sequence age for TT entry management
    //
    // See: https://www.chessprogramming.org/Transposition_Table#Aging
    pub age: u8,
}

impl<'a> Searcher<'a> {
    /// Starts the iterative deepening search loop
    ///
    /// Returns the best move found, its score, and the total nodes searched.
    pub fn start_search(params: SearcherParameters) -> (Score, Move, u64) {
        let SearcherParameters {
            pos,
            allocated_time,
            keep_running: is_running,
            max_depth,
            transposition_table,
            history_moves,
            age,
        } = params;

        // Decay history table to make sure old moves do not accumulate indefinitely and
        // overshadow newer moves.

        history_moves.decay();

        let search = Searcher {
            pos,
            keep_running: is_running,
            nodes: 0,
            start_time: Instant::now(),
            allocated_time,
            transposition_table,
            age,
            killer_moves: KillerMoves::new(),
            history_moves,
            max_ply: 0,
        };

        search.search(max_depth)
    }

    /// Starts an iterative deepening search up to the specified maximum depth,
    /// with aspiration windows and UCI info updates.
    fn search(mut self, max_depth: i8) -> (Score, Move, u64) {
        self.keep_running.set(true);
        // Fetch the best move from the transposition table to use as the initial guess
        // for best move, Since we might have seen this position before in
        // earlier searches.
        let mut best_move = self
            .transposition_table
            .probe(self.pos.zobrist_hash(), 0)
            .map_or(Move::NULL, |entry| entry.best_move);
        let mut last_depth_score = -score::INFINITY;

        let mut moves = MoveList::new();
        generate_moves(&self.pos, MoveGenType::Legal, &mut moves);

        if moves.is_empty() {
            return (last_depth_score, Move::NULL, 0);
        }

        // Iterative deepening: This helps to find good moves faster and allows us to
        // send intermediate results back to the main thread after each depth
        // iteration. It also enables aspiration windows based on the previous depth's
        // score.
        //
        // See: https://www.chessprogramming.org/Iterative_Deepening
        for depth in 1..=max_depth {
            if !self.keep_running.get() {
                break;
            }

            // Check if we still have enough time for another iteration to avoid timing out
            // in the middle of a ply. Also known as a Soft-Bound Time Limit.
            //
            // See: https://www.chessprogramming.org/Time_Management#Soft_Bound
            if let Some(limit) = self.allocated_time
                && self.start_time.elapsed() > limit / 2
            {
                break;
            }

            // Sort moves at the root based on the previous depth's best move and heuristics
            // to maximize alpha-beta pruning efficiency in the next iteration.
            self.sort_moves(&mut moves, best_move, 0);

            let mut best_score;
            let mut depth_best_move;

            // Aspiration Windows
            //
            // We set a narrow window around the previous depth's score to try to
            // trigger more beta cutoffs and speed up the search. If the search fails low or
            // high, we widen the window and re-search until we get a stable
            // score within the window.
            //
            // See: https://www.chessprogramming.org/Aspiration_Windows
            let mut alpha = -score::INFINITY;
            let mut beta = score::INFINITY;
            let mut delta: Score = 25; // aspiration window size in centipawns

            if depth >= 5 && !score::is_winning(last_depth_score.abs()) {
                alpha = last_depth_score - delta;
                beta = last_depth_score + delta;
            }

            loop {
                let search_alpha = alpha.max(-score::INFINITY);
                let search_beta = beta.min(score::INFINITY);

                let mut curr_alpha = search_alpha;
                best_score = -score::INFINITY;
                depth_best_move = Move::NULL;

                for m in moves.iter().copied() {
                    if !self.keep_running.get() {
                        break;
                    }
                    self.pos.do_move(m);
                    let score = -self.negamax(
                        depth - 1,
                        1,
                        -search_beta,
                        -curr_alpha,
                        SearchContext::default(),
                    );
                    self.pos.undo_move();

                    if score > best_score {
                        best_score = score;
                        depth_best_move = m;
                    }
                    if score > curr_alpha {
                        curr_alpha = score;
                    }
                }

                if !self.keep_running.get() {
                    break;
                }

                // If window was already full (-INFINITY, INFINITY), we stop, no re-search.
                if search_alpha == -score::INFINITY && search_beta == score::INFINITY {
                    break;
                }

                // Check fail-low / fail-high
                if best_score <= search_alpha {
                    // Fail low: score worse or equal to alpha. Widen alpha.
                    alpha -= delta;
                    beta = best_score + delta;
                    delta *= 2;
                } else if best_score >= search_beta {
                    // Fail high: score better or equal to beta. Widen beta.
                    beta += delta;
                    alpha = best_score - delta;
                    delta *= 2;
                } else {
                    // Stable score inside window!
                    break;
                }
            }

            if self.keep_running.get() && !depth_best_move.is_null() {
                last_depth_score = best_score;
                best_move = depth_best_move;

                self.send_uci_info(depth, best_score, best_move);
            }
        }

        self.keep_running.set(false);

        (last_depth_score, best_move, self.nodes)
    }

    /// Sends UCI info updates back to the main thread after each completed
    /// depth iteration, including the best move, score, principal
    /// variation, nodes searched, time taken, and NPS.
    fn send_uci_info(&self, depth: i8, best_score: Score, best_move: Move) {
        let pv_vec = self.extract_pv(depth, best_move);
        let time_elapsed = self.start_time.elapsed();
        let nps = if time_elapsed.as_secs_f64() > 0.001 {
            Some((self.nodes as f64 / time_elapsed.as_secs_f64()) as u64)
        } else {
            None
        };

        let uci_score = if let Some(mate_plies) = score::ply_to_mate_or_mated(best_score) {
            let mate_moves = mate_plies.div_ceil(2);
            let sign: i32 = if best_score > 0 { 1 } else { -1 };
            UciScoreBound {
                score: UciScore::Mate(sign * mate_moves as i32),
                bound: None,
            }
        } else {
            UciScoreBound {
                score: UciScore::Centipawns(best_score),
                bound: None,
            }
        };

        let info = UciInfo {
            depth: Some(depth as u32),
            seldepth: Some(self.max_ply as u32),
            nodes: Some(self.nodes),
            time: Some(time_elapsed),
            nps,
            hashfull: Some(self.transposition_table.hashfull()),
            score: Some(uci_score),
            pv: pv_vec.map(|pv| pv.into_iter().map(|m| m.to_uci_string()).collect()),
            ..UciInfo::new()
        };

        println!("{info}");
    }

    #[inline]
    fn update_analytics(&mut self, ply: u8) {
        self.nodes += 1;
        self.max_ply = self.max_ply.max(ply);
    }

    #[inline]
    fn should_stop_search(&self) -> bool {
        if !self.keep_running.get() {
            return true;
        }
        // Periodically check if we have exceeded the allocated time budget to avoid
        // timing out in the middle of a ply.
        if self.nodes & 1023 == 0
            && let Some(limit) = self.allocated_time
            && self.start_time.elapsed() >= limit
        {
            // Timed out, stop the search
            self.keep_running.set(false);
            return true;
        }
        false
    }

    /// Performs Fail-Soft Alpha-Beta Negamax Search to a specific depth.
    fn negamax(
        &mut self,
        depth: i8,
        ply: u8,
        mut alpha: Score,
        beta: Score,
        mut ctx: SearchContext,
    ) -> Score {
        self.update_analytics(ply);

        if self.should_stop_search() {
            return score::ZERO;
        }

        // Game over / rule evaluations (60-move rule, insufficient material,
        // repetitions, perpetual checks)
        if let Some(rule_score) = self.pos.rule_judge(ply) {
            return rule_score;
        }

        let alpha_orig = alpha;

        let mut best_score = -score::INFINITY;
        let mut best_move = Move::NULL;

        let mut depth = depth;

        // In general, we should do extensions before probing TT, since the depth might
        // increase after extensions, and we want to probe TT with the correct depth.

        // Check Extensions: If the side to move is in check,
        // we extend the search depth by 1 ply to give the engine a better chance to
        // find a defensive resource and avoid missing critical moves that could
        // save the King.
        //
        // See: https://www.chessprogramming.org/Check_Extensions
        if self.pos.is_in_check(self.pos.side_to_move()) && ctx.extensions < 6 {
            depth += 1;
            ctx.extensions += 1;
        }

        let mut is_singular = false;
        let tt_value = self.transposition_table.probe(self.pos.zobrist_hash(), ply);
        if let Some(value) = &tt_value {
            if value.depth >= depth {
                match value.flag {
                    TranspositionTableFlag::Exact => return value.score,
                    TranspositionTableFlag::Alpha => {
                        if value.score <= alpha {
                            return value.score;
                        }
                    }
                    TranspositionTableFlag::Beta => {
                        if value.score >= beta {
                            return value.score;
                        }
                    }
                    TranspositionTableFlag::Empty => {
                        unreachable!("Empty flag should not be returned by probe")
                    }
                }
            } else {
                // Even though the TT entry is not deep enough to be directly used, we can still
                // use the best move for move ordering and singular extensions.
                best_move = value.best_move;
            }

            // Singular Extensions: We want to check if TT move is a critical move, and that
            // removing it would cause us to be at a really bad position (score
            // drops significantly below beta). If so, we want to extend the
            // search to give the engine a better change to evaluate this move.
            //
            // See: https://www.chessprogramming.org/Singular_Extensions
            if depth >= 8
                && ctx.excluded_move.is_null()
                && ctx.extensions < 6
                && value.depth >= depth - 3
                && value.flag != TranspositionTableFlag::Alpha
                && !score::is_winning(value.score.abs())
            {
                let rdepth = depth - 3;
                let rbeta = value.score - singular_margin(depth);
                let score = self.negamax(
                    rdepth,
                    ply,
                    rbeta - 1,
                    rbeta,
                    SearchContext {
                        extensions: ctx.extensions,
                        excluded_move: value.best_move,
                    },
                );

                // Score fails low, that means the TT move is really good, and that the
                // alternatives are much worse. We should extend this node to find the best move
                // after this critical move.
                if score < rbeta {
                    is_singular = true;
                }
            }
        };

        // Base case: fall back to quiescence search
        if depth == 0 {
            return self.quiescence_search(0, ply, alpha, beta);
        }

        let mut moves = MoveList::new();
        generate_moves(&self.pos, MoveGenType::Legal, &mut moves);

        // Stalemate / Checkmate: In Xiangqi, a player with no legal moves loses.
        if moves.is_empty() {
            return score::mated_in(ply);
        }

        // One Reply Extensions: If there is only one legal move available,
        // we extend the search, as it is likely a critical position.
        //
        // See: https://www.chessprogramming.org/One_Reply_Extensions
        if moves.len() == 1 && ctx.excluded_move.is_null() && ctx.extensions < 6 {
            depth += 1;
            ctx.extensions += 1;
        }

        // Sort moves: prioritize captures via MVV-LVA Heuristic, with TT move
        // prioritized first, killers, and history
        self.sort_moves(&mut moves, best_move, ply);

        for m in moves {
            if m == ctx.excluded_move {
                continue;
            }

            let mut ext = 0;

            if let Some(value) = &tt_value
                && m == value.best_move
                && is_singular
            {
                ext = 1;
            }

            self.pos.do_move(m);
            let score = -self.negamax(
                depth - 1 + ext as i8,
                ply + 1,
                -beta,
                -alpha,
                SearchContext {
                    extensions: (ctx.extensions + ext),
                    ..Default::default()
                },
            );
            self.pos.undo_move();

            if !self.keep_running.get() {
                return score::ZERO;
            }

            if score > best_score {
                best_score = score;
                best_move = m;
            }
            if best_score > alpha {
                alpha = best_score;
            }
            if alpha >= beta {
                // Update killers and history for quiet moves
                if self.pos.is_empty(m.to()) {
                    self.killer_moves.update(m, ply);
                    self.history_moves
                        .increase(self.pos.side_to_move(), m, depth);
                }
                break; // Beta cutoff
            }
        }

        // Cache search results to Transposition Table
        if self.keep_running.get() {
            let flag = if best_score >= beta {
                TranspositionTableFlag::Beta
            } else if best_score <= alpha_orig {
                TranspositionTableFlag::Alpha
            } else {
                TranspositionTableFlag::Exact
            };
            self.transposition_table.store(
                self.pos.zobrist_hash(),
                ply,
                tt_value!(best_score, flag, best_move, depth, self.age),
            );
        }

        best_score
    }

    /// Implements Quiescence Search to evaluate tactical capture sequences.
    ///
    /// Prevents the horizon effect by searching captures only until a quiet
    /// position is reached.
    fn quiescence_search(&mut self, depth: i8, ply: u8, mut alpha: Score, beta: Score) -> Score {
        self.update_analytics(ply);

        if self.should_stop_search() {
            return score::ZERO;
        }

        // Base case: to avoid infinite recursion and stack overflow from perpetual
        // checks
        if depth <= -12 {
            let stand_pat = self.pos.evaluate();
            return if self.pos.side_to_move() == Color::White {
                stand_pat
            } else {
                -stand_pat
            };
        }

        if let Some(rule_score) = self.pos.rule_judge(ply) {
            return rule_score;
        }

        let in_check = self.pos.is_in_check(self.pos.side_to_move());

        let alpha_orig = alpha;
        let mut best_score = -score::INFINITY;

        // Standing pat: static evaluation provides the lower bound for non-check nodes.
        if !in_check {
            let stand_pat = self.pos.evaluate();
            best_score = if self.pos.side_to_move() == Color::White {
                stand_pat
            } else {
                -stand_pat
            };

            // If the static evaluation is already good enough to cause a beta cutoff, we
            // can prune this node without searching captures. This is the
            // essence of quiescence search: we only search captures if the
            // position is "noisy" (i.e. in check or has potential captures that
            // could change the evaluation significantly).
            if best_score >= beta {
                return best_score;
            }
            if best_score > alpha {
                alpha = best_score;
            }
        }

        let mut best_move = Move::NULL;

        let tt_value = self.transposition_table.probe(self.pos.zobrist_hash(), ply);
        if let Some(value) = &tt_value {
            if value.depth >= depth {
                match value.flag {
                    TranspositionTableFlag::Exact => return value.score,
                    TranspositionTableFlag::Alpha => {
                        if value.score <= alpha {
                            return value.score;
                        }
                    }
                    TranspositionTableFlag::Beta => {
                        if value.score >= beta {
                            return value.score;
                        }
                    }
                    TranspositionTableFlag::Empty => {
                        unreachable!("Empty flag should not be returned by probe")
                    }
                }
            } else {
                // Even though the TT entry is not deep enough to be directly used, we can still
                // use the best move for move ordering and singular extensions.
                best_move = value.best_move;
            }
        };

        let mut moves = MoveList::new();
        generate_moves(
            &self.pos,
            // Generate moves: if in check, we must search all legal evasions to save the
            // King. Otherwise, we only search capture moves.
            if in_check {
                MoveGenType::Legal
            } else {
                MoveGenType::Captures
            },
            &mut moves,
        );

        // Checkmate detection: in check with no legal evasions = checkmate
        if in_check && moves.is_empty() {
            return score::mated_in(ply);
        }

        // Sort captures using MVV-LVA
        self.sort_moves(&mut moves, best_move, ply);

        for m in moves {
            self.pos.do_move(m);
            let score = -self.quiescence_search(depth - 1, ply + 1, -beta, -alpha);
            self.pos.undo_move();
            if !self.keep_running.get() {
                return score::ZERO;
            }
            if score > best_score {
                best_score = score;
                best_move = m;
            }
            if best_score > alpha {
                alpha = best_score;
            }
            if alpha >= beta {
                // beta cutoff
                break;
            }
        }

        if self.keep_running.get() {
            let flag = if best_score >= beta {
                TranspositionTableFlag::Beta
            } else if best_score <= alpha_orig {
                TranspositionTableFlag::Alpha
            } else {
                TranspositionTableFlag::Exact
            };
            self.transposition_table.store(
                self.pos.zobrist_hash(),
                ply,
                tt_value!(best_score, flag, best_move, depth, self.age),
            );
        }

        best_score
    }

    /// Sorts the moves based on their heuristic scores, prioritizing the
    /// transposition table best move, killers, and history.
    #[inline]
    fn sort_moves(&self, moves: &mut [Move], tt_move: Move, ply: u8) {
        moves.sort_unstable_by_key(|&m| {
            // Returns a heuristic move-ordering score. Captures are scored highly based
            // on MVV-LVA. Quiet moves get score 0. The transposition table best move is
            // prioritized at the very top.
            let move_score = if m == tt_move {
                2_000_000_000 // Prioritize TT best move above all else
            } else {
                if let Some(to_piece) = self.pos.piece_at(m.to()) {
                    // Capture: 10000 + victim_rank * 100 - attacker_rank
                    let victim = get_piece_value_rank(to_piece);
                    let attacker = get_piece_value_rank(
                        self.pos
                            .piece_at(m.from())
                            .expect("Move from square should always have a piece"),
                    );
                    1_000_000_000 + victim * 10_000_000 - attacker * 100_000
                } else {
                    if self.killer_moves.contains(m, ply) {
                        900_000_000
                    } else {
                        self.history_moves.get(self.pos.side_to_move(), m)
                    }
                }
            };

            // Sort it in reverse since we want the best one
            Reverse(move_score)
        });
    }

    /// Traverses the transposition table to extract the predicted line of moves
    /// (Principal Variation).
    fn extract_pv(&self, depth: i8, best_move: Move) -> Option<Vec<Move>> {
        if best_move.is_null() {
            return None;
        }
        let mut pv = Vec::new();
        pv.push(best_move);

        let mut current_pos = self.pos.clone();
        current_pos.do_move(best_move);

        let mut visited = std::collections::HashSet::new();
        visited.insert(current_pos.zobrist_hash());

        for ply in 1..depth as u8 {
            if let Some(entry) = self
                .transposition_table
                .probe(current_pos.zobrist_hash(), ply)
            {
                if entry.flag == TranspositionTableFlag::Empty {
                    break;
                }
                let m = entry.best_move;
                // Here we do not need to check for null move
                // since we know that a non-empty TT entry must have a valid move.
                let mut moves = MoveList::new();
                generate_moves(&current_pos, MoveGenType::Legal, &mut moves);
                // The TT move might be illegal
                if !moves.contains(&m) {
                    eprintln!(
                        "debug: PV extraction: TT move {} at ply {} is not legal in the current position, stopping PV extraction",
                        m.to_uci_string(),
                        ply
                    );
                    break;
                }
                pv.push(m);
                current_pos.do_move(m);

                let hash = current_pos.zobrist_hash();
                if visited.contains(&hash) {
                    break;
                }
                visited.insert(hash);
            } else {
                break;
            }
        }

        Some(pv)
    }
}

/// Simple helper to rank piece types for MVV-LVA move ordering.
#[inline]
const fn get_piece_value_rank(p: Piece) -> i32 {
    match p.piece_type() {
        PieceType::King => 8,
        PieceType::Rook => 7,
        PieceType::Cannon => 6,
        PieceType::Knight => 5,
        PieceType::Advisor => 4,
        PieceType::Bishop => 3,
        PieceType::Pawn => 2,
    }
}

/// Calculates the margin threshold required for singular extensions.
#[inline]
fn singular_margin(depth: i8) -> Score {
    2 * depth as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Position;
    use crate::core::{Color, Move, Square};
    use std::sync::Arc;
    use std::time::Instant;

    #[test]
    fn test_repetition_draw_repetition() {
        let mut pos = Position::new();
        // Setup a simple position with files D and E blocked by pawns to avoid
        // King-facing checks
        pos.set("4k4/9/9/9/3PP4/9/9/9/9/4K4 w - - 0 1").unwrap();
        // Setup moves to repeat: King moves back and forth
        let w_move1 = Move::new(Square::E0, Square::D0);
        let w_move2 = Move::new(Square::D0, Square::E0);
        let b_move1 = Move::new(Square::E9, Square::D9);
        let b_move2 = Move::new(Square::D9, Square::E9);

        // White moves to D0
        pos.do_move(w_move1);
        // Black moves to D9
        pos.do_move(b_move1);
        // White moves back to E0
        pos.do_move(w_move2);
        // Black moves back to E9
        pos.do_move(b_move2);

        // Now White moves to D0 again (first repetition check at ply 5)
        pos.do_move(w_move1);

        // Black moves to D9 again (repeating the state at ply 1)
        pos.do_move(b_move1);
        // This completed a repetition, neither side is in check.
        assert_eq!(pos.rule_judge(6), Some(score::ZERO));

        // Call negamax with depth=1, we should get 0 (draw)
        let mut transposition_table = TranspositionTable::new(1);
        let killers = KillerMoves::default();
        let mut history_moves = HistoryMoves::default();
        let mut ctx = Searcher {
            pos: pos.clone(),
            keep_running: Arc::new(RunningStatus::default()),
            nodes: 0,
            start_time: Instant::now(),
            allocated_time: None,
            transposition_table: &mut transposition_table,
            age: 1,
            killer_moves: killers,
            history_moves: &mut history_moves,
            max_ply: 0,
        };
        let score = ctx.negamax(
            1,
            6,
            -score::INFINITY,
            score::INFINITY,
            SearchContext::default(),
        );
        assert_eq!(score, 0);
    }

    #[test]
    fn test_repetition_perpetual_check_repetition() {
        let mut pos = Position::new();
        // White King at E0, White Rook at D1, Black King at D9
        pos.set("3k5/9/9/9/9/9/9/9/3R5/4K4 w - - 0 1").unwrap();

        // White Rook checks: D1 to D8 (giving check)
        let r_check1 = Move::new(Square::D1, Square::D8);
        // Black King evades: D9 to E9 (not checking)
        let k_move1 = Move::new(Square::D9, Square::E9);
        // White Rook checks again: D8 to E8 (giving check)
        let r_check2 = Move::new(Square::D8, Square::E8);
        // Black King evades: E9 to D9
        let k_move2 = Move::new(Square::E9, Square::D9);
        // White Rook checks again: E8 to D8 (giving check)
        let r_check3 = Move::new(Square::E8, Square::D8);

        // 1. White checks
        pos.do_move(r_check1);
        assert!(pos.is_in_check(Color::Black));

        // 2. Black evades
        pos.do_move(k_move1);

        // 3. White checks again
        pos.do_move(r_check2);
        assert!(pos.is_in_check(Color::Black));

        // 4. Black moves King back to D9
        pos.do_move(k_move2);

        // 5. White checks again with Rook to D8 (repetition + check!)
        pos.do_move(r_check3);

        // Now Black turn to move. White just gave the repeating check on all turns in
        // the loop.
        assert_eq!(pos.rule_judge(5), Some(score::mate_in(5)));
        assert!(pos.is_in_check(Color::Black));

        // Black should win because White is perpetually checking!
        // negamax should return a win score (MATE_VALUE - ply)
        let mut transposition_table = TranspositionTable::new(1);
        let killers = KillerMoves::default();
        let mut history_moves = HistoryMoves::default();
        let mut ctx = Searcher {
            pos,
            keep_running: Arc::new(RunningStatus::default()),
            nodes: 0,
            start_time: Instant::now(),
            allocated_time: None,
            transposition_table: &mut transposition_table,
            age: 1,
            killer_moves: killers,
            history_moves: &mut history_moves,
            max_ply: 0,
        };
        ctx.keep_running.set(true);
        let score = ctx.negamax(
            1,
            5,
            -score::INFINITY,
            score::INFINITY,
            SearchContext::default(),
        );
        assert_eq!(score, score::mate_in(5));
    }
}
