//! Negamax Fail-Soft Alpha-Beta Search engine with selective search extensions.
//!
//! This module implements the main search logic used to determine the best
//! moves:
//! 1. **Fail-Soft Negamax Alpha-Beta Pruning**: Recursively searches the game
//!    tree to find optimal moves while pruning branches that cannot impact the
//!    search outcome.
//! 2. **Aspiration Windows**: Minimizes the search space width around the
//!    previous depth's best score. Widens boundaries progressively on fail-low
//!    (fail-soft lower limit) or fail-high bounds.
//! 3. **Move Ordering Heuristics**: Prioritizes the best transposition table
//!    move, MVV-LVA (Most Valuable Victim - Least Valuable Attacker) capture
//!    heuristics, killer moves, and history heuristic tables to trigger
//!    beta-cutoffs as early as possible.
//! 4. **Quiescence Search**: Solves the horizon effect by searching only
//!    tactical capture sequences until a stable, quiet position is reached.
//! 5. **Selective Search Extensions**:
//!    - **Check Extensions**: Automatically extends the depth by 1 ply when in
//!      check.
//!    - **Singular Extensions**: Verifies if the transposition table move is
//!      exceptionally superior compared to alternative moves at that node. If
//!      so, extends the search by 1 ply. Requires a reduced-depth probe with an
//!      aspiration-like threshold.
//!    - **One-Reply Extensions**: If only a single legal move exists, extends
//!      the depth by 1 ply to prevent arbitrary horizon cutoffs since there is
//!      no branching factor.

use std::cmp::Reverse;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use strum::EnumCount;

use crate::core::{
    Color, MAX_PLY, Move, MoveGenType, MoveList, MoveScore, Piece, PieceType, Position, Square,
    Value, generate_moves,
};
use crate::uci::{UciInfo, UciScore, UciScoreBound};
use crate::{tt_value, value};

mod transposition_table;
pub use transposition_table::*;

/// Tracks search extension and move exclusion parameters for the current
/// branch.
#[derive(Copy, Clone, Debug)]
struct SearchContext {
    pub excluded_move: Move,
    pub extensions: u8,
}

impl Default for SearchContext {
    fn default() -> Self {
        Self {
            extensions: 0,
            excluded_move: Move::null(),
        }
    }
}

impl SearchContext {
    /// Creates a new SearchExtension parameters set.
    pub fn new(extensions: u8, excluded_move: Move) -> Self {
        Self {
            extensions,
            excluded_move,
        }
    }
}

/// Shared context parameters passed down the recursive search stack.
pub struct Search<'a> {
    pos: Position,
    /// Atomic flag set by Thread A to interrupt the search loop.
    stop: Arc<AtomicBool>,
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
    killers: [[Move; 2]; MAX_PLY],
    /// History heuristic table to prioritize frequently successful quiet moves.
    history_table: &'a mut [[[MoveScore; Square::COUNT]; Square::COUNT]; Color::COUNT],
    /// Tracks the maximum search depth reached, including quiescence.
    max_ply: u8,
    /// Channel sender to send UCI info updates back to the main thread.
    tx: Sender<UciInfo>,
}

pub struct SearchParameters<'a> {
    // We take ownership of the position here since we will be modifying it during search.
    pub pos: Position,
    // The time limit for this search
    pub allocated_time: Option<Duration>,
    // The stop flag to signal search interruption from another thread
    pub stop: Arc<AtomicBool>,
    // The max depth to search to
    pub max_depth: i8,
    // The transposition table to use for caching search results
    //
    // See: https://www.chessprogramming.org/Transposition_Table
    pub transposition_table: &'a mut TranspositionTable,
    // The history table for quiet moves
    //
    // See: https://www.chessprogramming.org/History_Heuristic
    pub history_table: &'a mut [[[MoveScore; Square::COUNT]; Square::COUNT]; Color::COUNT],
    // The channel sender to send UCI info updates back to the main thread
    pub tx: Sender<UciInfo>,
    // The current search sequence age for TT entry management
    //
    // See: https://www.chessprogramming.org/Transposition_Table#Aging
    pub age: u8,
}

impl<'a> Search<'a> {
    /// Starts the iterative deepening search loop
    ///
    /// Returns the best move found, its score, and the total nodes searched.
    pub fn start_search(params: SearchParameters) -> (Value, Move, u64) {
        let SearchParameters {
            pos,
            allocated_time,
            stop,
            max_depth,
            transposition_table,
            history_table,
            tx,
            age,
        } = params;

        // Decay history table to make sure old moves do not accumulate indefinitely and
        // overshadow newer moves.
        for side in history_table.iter_mut() {
            for from in side.iter_mut() {
                for to in from.iter_mut() {
                    *to >>= 3;
                }
            }
        }

        let search = Search {
            pos,
            stop,
            nodes: 0,
            start_time: Instant::now(),
            allocated_time,
            transposition_table,
            age,
            killers: [[Move::null(); 2]; MAX_PLY],
            history_table,
            max_ply: 0,
            tx,
        };

        search.search(max_depth)
    }

    /// Starts an iterative deepening search up to the specified maximum depth,
    /// with aspiration windows and UCI info updates.
    fn search(mut self, max_depth: i8) -> (Value, Move, u64) {
        // Fetch the best move from the transposition table to use as the initial guess
        // for best move, Since we might have seen this position before in
        // earlier searches.
        let mut best_move = self
            .transposition_table
            .probe(self.pos.zobrist_hash(), 0)
            .map_or(Move::null(), |entry| entry.best_move);
        let mut last_depth_score = -Value::INFINITY;

        let mut moves = MoveList::new();
        generate_moves(&self.pos, MoveGenType::Legal, &mut moves);

        if moves.is_empty() {
            return (last_depth_score, Move::null(), 0);
        }

        // Iterative deepening: This helps to find good moves faster and allows us to
        // send intermediate results back to the main thread after each depth
        // iteration. It also enables aspiration windows based on the previous depth's
        // score.
        //
        // See: https://www.chessprogramming.org/Iterative_Deepening
        for depth in 1..=max_depth {
            if self.stop.load(Ordering::Relaxed) {
                break;
            }

            // Check if we still have enough time for another iteration to avoid timing out
            // in the middle of a ply. Also known as a Soft-Bound Time Limit.
            //
            // See: https://www.chessprogramming.org/Time_Management#Soft_Bound
            if let Some(limit) = self.allocated_time
                && self.start_time.elapsed() > limit / 4
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
            let mut alpha = -Value::INFINITY;
            let mut beta = Value::INFINITY;
            let mut delta: Value = value!(25); // aspiration window size in centipawns

            if depth >= 5 && !last_depth_score.abs().is_winning() {
                alpha = last_depth_score - delta;
                beta = last_depth_score + delta;
            }

            loop {
                let search_alpha = alpha.max(-Value::INFINITY);
                let search_beta = beta.min(Value::INFINITY);

                let mut curr_alpha = search_alpha;
                best_score = -Value::INFINITY;
                depth_best_move = Move::null();

                for m in moves.iter().copied() {
                    if self.stop.load(Ordering::Relaxed) {
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
                    self.pos.undo_move(m);

                    if score > best_score {
                        best_score = score;
                        depth_best_move = m;
                    }
                    if score > curr_alpha {
                        curr_alpha = score;
                    }
                }

                if self.stop.load(Ordering::Relaxed) {
                    break;
                }

                // If window was already full (-INFINITY, INFINITY), we stop, no re-search.
                if search_alpha == -Value::INFINITY && search_beta == Value::INFINITY {
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

            if !self.stop.load(Ordering::Relaxed) && !depth_best_move.is_null() {
                last_depth_score = best_score;
                best_move = depth_best_move;

                self.send_uci_info(depth, best_score, best_move);
            }
        }

        (last_depth_score, best_move, self.nodes)
    }

    /// Sends UCI info updates back to the main thread after each completed
    /// depth iteration, including the best move, score, principal
    /// variation, nodes searched, time taken, and NPS.
    fn send_uci_info(&self, depth: i8, best_score: Value, best_move: Move) {
        let pv_vec = self.extract_pv(depth, best_move);
        let time_elapsed = self.start_time.elapsed();
        let nps = if time_elapsed.as_secs_f64() > 0.001 {
            Some((self.nodes as f64 / time_elapsed.as_secs_f64()) as u64)
        } else {
            None
        };

        let uci_score = if let Some(mate_plies) = best_score.ply_to_mate_or_mated() {
            let mate_moves = mate_plies.div_ceil(2);
            let sign: i32 = if best_score.raw() > 0 { 1 } else { -1 };
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

        self.tx.send(info).ok();
    }

    #[inline]
    fn update_analytics(&mut self, ply: u8) {
        self.nodes += 1;
        self.max_ply = self.max_ply.max(ply);
    }

    #[inline]
    fn should_stop_search(&self) -> bool {
        if self.stop.load(Ordering::Relaxed) {
            return true;
        }
        // Periodically check if we have exceeded the allocated time budget to avoid
        // timing out in the middle of a ply.
        if self.nodes & 1023 == 0
            && let Some(limit) = self.allocated_time
            && self.start_time.elapsed() >= limit
        {
            self.stop.store(true, Ordering::Relaxed);
            return true;
        }
        false
    }

    /// Performs Fail-Soft Alpha-Beta Negamax Search to a specific depth.
    fn negamax(
        &mut self,
        depth: i8,
        ply: u8,
        mut alpha: Value,
        beta: Value,
        mut ctx: SearchContext,
    ) -> Value {
        self.update_analytics(ply);

        if self.should_stop_search() {
            return Value::ZERO;
        }

        // Game over / rule evaluations (60-move rule, insufficient material,
        // repetitions, perpetual checks)
        if let Some(rule_score) = self.pos.rule_judge(ply) {
            return rule_score;
        }

        let alpha_orig = alpha;

        let mut best_score = -Value::INFINITY;
        let mut best_move = Move::null();

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
                && !value.score.abs().is_winning()
            {
                let rdepth = depth - 3;
                let rbeta = value.score - singular_margin(depth);
                let score = self.negamax(
                    rdepth,
                    ply,
                    rbeta - value!(1),
                    rbeta,
                    SearchContext::new(ctx.extensions, value.best_move),
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
            return self.quiescence(0, ply, alpha, beta);
        }

        let mut moves = MoveList::new();
        generate_moves(&self.pos, MoveGenType::Legal, &mut moves);

        // Stalemate / Checkmate: In Xiangqi, a player with no legal moves loses.
        if moves.is_empty() {
            return Value::mated_in(ply);
        }

        let mut depth = depth;

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
                SearchContext::new(ctx.extensions + ext, Move::null()),
            );
            self.pos.undo_move(m);

            if self.stop.load(Ordering::Relaxed) {
                return Value::ZERO;
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
                if self.pos.piece_at(m.square_to()) == Piece::None {
                    let ply_idx = ply as usize;
                    // We do not check for ply_idx out of bounds since
                    // ply should not exceed MAX_PLY in normal circumstances
                    if self.killers[ply_idx][0] != m {
                        self.killers[ply_idx][1] = self.killers[ply_idx][0];
                        self.killers[ply_idx][0] = m;
                    }
                    let side_idx = self.pos.side_to_move() as usize;
                    let from_idx = m.square_from() as usize;
                    let to_idx = m.square_to() as usize;
                    self.history_table[side_idx][from_idx][to_idx] +=
                        (depth as i32) * (depth as i32);
                }
                break; // Beta cutoff
            }
        }

        // Cache search results to Transposition Table
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

        best_score
    }

    /// Implements Quiescence Search to evaluate tactical capture sequences.
    ///
    /// Prevents the horizon effect by searching captures only until a quiet
    /// position is reached.
    fn quiescence(&mut self, depth: i8, ply: u8, mut alpha: Value, beta: Value) -> Value {
        if self.stop.load(Ordering::Relaxed) {
            return Value::ZERO;
        }
        self.nodes += 1;
        self.max_ply = self.max_ply.max(ply);

        // Check stop signals periodically
        if self.nodes & 1023 == 0
            && let Some(limit) = self.allocated_time
            && self.start_time.elapsed() >= limit
        {
            self.stop.store(true, Ordering::Relaxed);
            return Value::ZERO;
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

        let in_check = self.pos.is_in_check(self.pos.side_to_move());

        // Standing pat: static evaluation provides the lower bound for non-check nodes.
        if !in_check {
            let stand_pat = self.pos.evaluate();
            let eval_side = if self.pos.side_to_move() == Color::White {
                stand_pat
            } else {
                -stand_pat
            };

            if eval_side >= beta {
                return eval_side;
            }
            if eval_side > alpha {
                alpha = eval_side;
            }
        }

        // Generate moves: if in check, we must search all legal evasions to save the
        // King. Otherwise, we only search capture moves.
        let mut moves = MoveList::new();
        if in_check {
            generate_moves(&self.pos, MoveGenType::Legal, &mut moves)
        } else {
            generate_moves(&self.pos, MoveGenType::Captures, &mut moves)
        };

        // Checkmate detection: in check with no legal moves = checkmate
        if in_check && moves.is_empty() {
            return Value::mated_in(ply);
        }

        // Sort captures using MVV-LVA
        self.sort_moves(&mut moves, Move::null(), ply);

        for m in moves {
            self.pos.do_move(m);
            let score = -self.quiescence(depth - 1, ply + 1, -beta, -alpha);
            self.pos.undo_move(m);

            if self.stop.load(Ordering::Relaxed) {
                return Value::ZERO;
            }

            if score >= beta {
                return score;
            }
            if score > alpha {
                alpha = score;
            }
        }

        alpha
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
                let to_piece = self.pos.piece_at(m.square_to());
                if to_piece != Piece::None {
                    // Capture: 10000 + victim_rank * 100 - attacker_rank
                    let victim = get_piece_value_rank(to_piece);
                    let attacker = get_piece_value_rank(self.pos.piece_at(m.square_from()));
                    1_000_000_000 + victim * 10_000_000 - attacker * 100_000
                } else {
                    // Quiet move
                    let ply_idx = ply as usize;
                    // We might not need to check for ply_idx out of bounds since ply should not
                    // exceed MAX_PLY in normal circumstances
                    if m == self.killers[ply_idx][0] {
                        900_000_000
                    } else if m == self.killers[ply_idx][1] {
                        800_000_000
                    } else {
                        let side_idx = self.pos.side_to_move() as usize;
                        let from_idx = m.square_from() as usize;
                        let to_idx = m.square_to() as usize;
                        // History score should not be more than 800_000_000
                        self.history_table[side_idx][from_idx][to_idx]
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
        _ => 0,
    }
}

/// Calculates the margin threshold required for singular extensions.
#[inline]
fn singular_margin(depth: i8) -> Value {
    value!(2 * depth as i16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Position;
    use crate::core::{Color, Move, Square};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
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
        assert_eq!(pos.rule_judge(6), Some(Value::ZERO));

        // Call negamax with depth=1, we should get 0 (draw)
        let stop = Arc::new(AtomicBool::new(false));
        let mut transposition_table = TranspositionTable::new(1);
        let killers = [[Move::null(); 2]; MAX_PLY];
        let mut history_table = [[[0; 90]; 90]; 2];
        let mut ctx = Search {
            pos: pos.clone(),
            stop,
            nodes: 0,
            start_time: Instant::now(),
            allocated_time: None,
            transposition_table: &mut transposition_table,
            age: 1,
            killers,
            history_table: &mut history_table,
            max_ply: 0,
            tx: std::sync::mpsc::channel().0, /* dummy sender since we won't be sending UCI info
                                               * in this test */
        };
        let score = ctx.negamax(
            1,
            6,
            -Value::INFINITY,
            Value::INFINITY,
            SearchContext::default(),
        );
        assert_eq!(score.raw(), 0);
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
        assert_eq!(pos.rule_judge(5), Some(Value::mate_in(5)));
        assert!(pos.is_in_check(Color::Black));

        // Black should win because White is perpetually checking!
        // negamax should return a win score (MATE_VALUE - ply)
        let stop = Arc::new(AtomicBool::new(false));
        let mut transposition_table = TranspositionTable::new(1);
        let killers = [[Move::null(); 2]; MAX_PLY];
        let mut history_table = [[[0; 90]; 90]; 2];
        let mut ctx = Search {
            pos,
            stop,
            nodes: 0,
            start_time: Instant::now(),
            allocated_time: None,
            transposition_table: &mut transposition_table,
            age: 1,
            killers,
            history_table: &mut history_table,
            max_ply: 0,
            tx: std::sync::mpsc::channel().0, /* dummy sender since we won't be sending UCI info
                                               * in this test */
        };
        let score = ctx.negamax(
            1,
            5,
            -Value::INFINITY,
            Value::INFINITY,
            SearchContext::default(),
        );
        assert_eq!(score, Value::mate_in(5));
    }

    #[test]
    fn test_search_max_ply_and_pv() {
        let mut pos = Position::new();
        pos.set("rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w")
            .unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let mut tt = TranspositionTable::new(1);
        let stop = Arc::new(AtomicBool::new(false));

        let mut history_table = [[[0; 90]; 90]; 2];
        let (_score, best_move, _nodes) = Search::start_search(SearchParameters {
            pos,
            allocated_time: None,
            stop,
            max_depth: 2,
            transposition_table: &mut tt,
            history_table: &mut history_table,
            tx,
            age: 0,
        });

        assert!(!best_move.is_null());

        // Drain the channel and check UciInfo values
        let mut max_seldepth = 0;
        let mut has_pv = false;
        while let Ok(info) = rx.try_recv() {
            if let Some(sd) = info.seldepth
                && sd > max_seldepth
            {
                max_seldepth = sd;
            }
            if let Some(pv) = info.pv
                && !pv.is_empty()
            {
                has_pv = true;
            }
        }

        // Depth 2 search must reach at least ply 2 in negamax or quiescence
        assert!(max_seldepth >= 2, "max_seldepth was {}", max_seldepth);
        assert!(has_pv);
    }
}
