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
    Color, MAX_DEPTH, MAX_PLY, Move, MoveGenType, MoveList, MoveScore, Piece, PieceType, Position,
    Square, Value, generate_moves,
};
use crate::eval::evaluate;
use crate::uci::{GoParameters, UciInfo, UciScore, UciScoreBound};
use crate::{tt_entry_value, value};

mod transposition_table;
pub use transposition_table::*;

/// Represents the alpha-beta search window.
#[derive(Copy, Clone, Debug)]
struct SearchWindow {
    pub alpha: Value,
    pub beta: Value,
}

impl SearchWindow {
    /// Creates a new SearchWindow.
    pub fn new(alpha: Value, beta: Value) -> Self {
        Self { alpha, beta }
    }

    /// Negates the window (reverses and negates bounds) for the next ply in
    /// Negamax.
    pub fn negate(self) -> Self {
        Self {
            alpha: -self.beta,
            beta: -self.alpha,
        }
    }
}

/// Tracks search extension and move exclusion parameters for the current
/// branch.
#[derive(Copy, Clone, Debug)]
struct SearchExtension {
    pub excluded_move: Move,
    pub extensions: u8,
}

impl Default for SearchExtension {
    fn default() -> Self {
        Self {
            extensions: 0,
            excluded_move: Move::null(),
        }
    }
}

impl SearchExtension {
    /// Creates a new SearchExtension parameters set.
    pub fn new(extensions: u8, excluded_move: Move) -> Self {
        Self {
            extensions,
            excluded_move,
        }
    }
}

/// Shared context parameters passed down the recursive search stack.
struct SearchContext<'a> {
    /// Atomic flag set by Thread A to interrupt the search loop.
    pub stop: &'a Arc<AtomicBool>,
    /// Tracks total nodes searched during this `go` invocation.
    pub nodes: &'a mut u64,
    /// The moment when the search was started.
    pub start_time: Instant,
    /// Absolute time budget allowed for this search ply.
    pub time_limit: Option<std::time::Duration>,
    /// Reference to the transposition table.
    pub transposition_table: &'a mut TranspositionTable,
    /// Current search sequence age.
    pub age: u8,
    /// Killer moves tracked per ply to sort high-quality quiet moves.
    pub killers: &'a mut [[Move; 2]; MAX_PLY],
    /// History heuristic table to prioritize frequently successful quiet moves.
    pub history_table: &'a mut [[[MoveScore; Square::COUNT]; Square::COUNT]; Color::COUNT],
    /// Tracks the maximum search depth reached, including quiescence.
    pub max_ply: &'a mut u8,
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

/// Sorts the moves based on their heuristic scores, prioritizing the
/// transposition table best move, killers, and history.
fn sort_moves(pos: &Position, moves: &mut [Move], tt_move: Move, ctx: &SearchContext, ply: u8) {
    moves.sort_by_cached_key(|&m| {
        // Returns a heuristic move-ordering score. Captures are scored highly based
        // on MVV-LVA. Quiet moves get score 0. The transposition table best move is
        // prioritized at the very top.
        let move_score = if m == tt_move {
            2_000_000_000 // Prioritize TT best move above all else
        } else {
            let to_piece = pos.piece_at(m.square_to());
            if to_piece != Piece::None {
                // Capture: 10000 + victim_rank * 100 - attacker_rank
                let victim = get_piece_value_rank(to_piece);
                let attacker = get_piece_value_rank(pos.piece_at(m.square_from()));
                1_000_000_000 + victim * 10_000_000 - attacker * 100_000
            } else {
                // Quiet move
                let ply_idx = ply as usize;
                // We might not need to check for ply_idx out of bounds since ply should not
                // exceed MAX_PLY in normal circumstances
                if m == ctx.killers[ply_idx][0] {
                    900_000_000
                } else if m == ctx.killers[ply_idx][1] {
                    800_000_000
                } else {
                    let side_idx = pos.side_to_move() as usize;
                    let from_idx = m.square_from() as usize;
                    let to_idx = m.square_to() as usize;
                    // History score should not be more than 800_000_000
                    ctx.history_table[side_idx][from_idx][to_idx]
                }
            }
        };

        // Sort it in reverse since we want the best one
        Reverse(move_score)
    });
}

/// Traverses the transposition table to extract the predicted line of moves
/// (Principal Variation).
fn extract_pv(
    pos: &Position,
    tt: &TranspositionTable,
    depth: u8,
    best_move: Move,
) -> Option<Vec<Move>> {
    if best_move.is_null() {
        return None;
    }
    let mut pv = Vec::new();
    pv.push(best_move);

    let mut current_pos = pos.clone();
    current_pos.do_move(best_move);

    let mut visited = std::collections::HashSet::new();
    visited.insert(current_pos.zobrist_hash());

    for ply in 1..depth {
        if let Some(entry) = tt.probe(current_pos.zobrist_hash(), ply) {
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

/// Starts a search from the current position
pub fn search(
    mut pos: Position,
    params: GoParameters,
    transposition_table: &mut TranspositionTable,
    age: u8,
    tx: Sender<UciInfo>,
    time_limit: Option<Duration>,
) -> (Move, Value, u64) {
    let start_time = Instant::now();
    let mut nodes = 0u64;

    let max_depth = params.depth.unwrap_or(MAX_DEPTH as u32) as u8;

    let mut best_move = Move::null();
    let mut last_depth_score = -Value::INFINITY;

    let mut killers = [[Move::null(); 2]; MAX_PLY];
    let mut history_table = [[[0; 90]; 90]; 2];

    for depth in 1..=max_depth {
        if params.stop.load(Ordering::Relaxed) {
            break;
        }

        // Check if we have spent >50% of the allowed time to avoid timing out in next
        // ply
        if let Some(limit) = time_limit
            && start_time.elapsed() > limit / 2
        {
            break;
        }

        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);

        if moves.is_empty() {
            break;
        }

        // Sort root moves to maximize alpha-beta pruning (captures first)
        moves.sort_by_key(|mv| pos.is_empty(mv.square_to()));

        let mut best_score;
        let mut depth_best_move;

        let mut max_ply = 0u8;

        // Aspiration Windows Setup
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

            let mut ctx = SearchContext {
                stop: &params.stop,
                nodes: &mut nodes,
                start_time,
                time_limit,
                transposition_table,
                age,
                killers: &mut killers,
                history_table: &mut history_table,
                max_ply: &mut max_ply,
            };

            let mut curr_alpha = search_alpha;
            best_score = -Value::INFINITY;
            depth_best_move = Move::null();

            for m in moves.iter().copied() {
                if params.stop.load(Ordering::Relaxed) {
                    break;
                }
                pos.do_move(m);
                let score = -negamax(
                    &mut pos,
                    depth - 1,
                    1,
                    SearchWindow::new(-search_beta, -curr_alpha),
                    SearchExtension::default(),
                    &mut ctx,
                );
                pos.undo_move(m);

                if params.stop.load(Ordering::Relaxed) {
                    break;
                }

                if score > best_score {
                    best_score = score;
                    depth_best_move = m;
                }
                if score > curr_alpha {
                    curr_alpha = score;
                }
            }

            if params.stop.load(Ordering::Relaxed) {
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

        if params.stop.load(Ordering::Relaxed) {
            break;
        }

        last_depth_score = best_score;

        // If the search was not aborted, save search outcomes and print UCI progress
        if !params.stop.load(Ordering::Relaxed) {
            if !depth_best_move.is_null() {
                best_move = depth_best_move;
            }

            let pv_vec = extract_pv(&pos, transposition_table, depth, best_move);
            let time_elapsed = start_time.elapsed();
            let nps = if time_elapsed.as_secs_f64() > 0.001 {
                Some((nodes as f64 / time_elapsed.as_secs_f64()) as u64)
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
                seldepth: Some(max_ply as u32),
                nodes: Some(nodes),
                time: Some(time_elapsed),
                nps,
                hashfull: Some(transposition_table.hashfull()),
                score: Some(uci_score),
                pv: pv_vec.map(|pv| pv.into_iter().map(|m| m.to_uci_string()).collect()),
                ..UciInfo::new()
            };

            tx.send(info).ok();
        }
    }

    (best_move, last_depth_score, nodes)
}

/// Implements Quiescence Search to evaluate tactical capture sequences.
///
/// Prevents the horizon effect by searching captures only until a quiet
/// position is reached.
fn quiescence(
    pos: &mut Position,
    mut window: SearchWindow,
    ply: u8,
    qdepth: u8,
    ctx: &mut SearchContext,
) -> Value {
    if ctx.stop.load(Ordering::Relaxed) {
        return Value::ZERO;
    }
    *ctx.nodes += 1;

    if ply > *ctx.max_ply {
        *ctx.max_ply = ply;
    }

    // Check stop signals periodically
    if *ctx.nodes & 1023 == 0
        && let Some(limit) = ctx.time_limit
        && ctx.start_time.elapsed() >= limit
    {
        ctx.stop.store(true, Ordering::Relaxed);
        return Value::ZERO;
    }

    // Base case: to avoid infinite recursion and stack overflow from perpetual
    // checks
    if qdepth >= 12 {
        let stand_pat = evaluate(pos);
        return if pos.side_to_move() == Color::White {
            stand_pat
        } else {
            -stand_pat
        };
    }

    let in_check = pos.is_in_check(pos.side_to_move());

    // Standing pat: static evaluation provides the lower bound for non-check nodes.
    if !in_check {
        let stand_pat = evaluate(pos);
        let eval_side = if pos.side_to_move() == Color::White {
            stand_pat
        } else {
            -stand_pat
        };

        if eval_side >= window.beta {
            return eval_side;
        }
        if eval_side > window.alpha {
            window.alpha = eval_side;
        }
    }

    // Generate moves: if in check, we must search all legal evasions to save the
    // King. Otherwise, we only search capture moves.
    let mut moves = MoveList::new();
    if in_check {
        generate_moves(pos, MoveGenType::Legal, &mut moves)
    } else {
        generate_moves(pos, MoveGenType::Captures, &mut moves)
    };

    // Checkmate detection: in check with no legal moves = checkmate
    if in_check && moves.is_empty() {
        return Value::mated_in(ply);
    }

    // Sort captures using MVV-LVA
    sort_moves(pos, &mut moves, Move::null(), ctx, ply);

    for m in moves {
        pos.do_move(m);
        let score = -quiescence(pos, window.negate(), ply + 1, qdepth + 1, ctx);
        pos.undo_move(m);

        if ctx.stop.load(Ordering::Relaxed) {
            return Value::ZERO;
        }

        if score >= window.beta {
            return score;
        }
        if score > window.alpha {
            window.alpha = score;
        }
    }

    window.alpha
}

/// Calculates the margin threshold required for singular extensions.
#[inline]
fn singular_margin(depth: u8) -> Value {
    value!(2 * depth as i32)
}

/// Performs Fail-Soft Alpha-Beta Negamax Search to a specific depth.
fn negamax(
    pos: &mut Position,
    depth: u8,
    ply: u8,
    mut window: SearchWindow,
    mut ext_control: SearchExtension,
    ctx: &mut SearchContext,
) -> Value {
    if ctx.stop.load(Ordering::Relaxed) {
        return Value::ZERO;
    }
    *ctx.nodes += 1;

    if ply > *ctx.max_ply {
        *ctx.max_ply = ply;
    }

    // Check stop signals periodically
    if *ctx.nodes & 1023 == 0
        && let Some(limit) = ctx.time_limit
        && ctx.start_time.elapsed() >= limit
    {
        ctx.stop.store(true, Ordering::Relaxed);
        return Value::ZERO;
    }

    // Game over / rule evaluations (60-move rule, insufficient material,
    // repetitions, perpetual checks)
    if let Some(rule_score) = pos.rule_judge(ply) {
        return rule_score;
    }

    let mut depth = depth;
    if pos.is_in_check(pos.side_to_move()) && ext_control.extensions < 6 {
        depth += 1;
        ext_control.extensions += 1;
    }

    let alpha_orig = window.alpha;

    let mut is_singular = false;
    let tt_move = if let Some(entry) = ctx.transposition_table.probe(pos.zobrist_hash(), ply) {
        if entry.depth >= depth {
            match entry.flag {
                TranspositionTableFlag::Exact => return entry.score,
                TranspositionTableFlag::Alpha => {
                    if entry.score <= window.alpha {
                        return entry.score;
                    }
                }
                TranspositionTableFlag::Beta => {
                    if entry.score >= window.beta {
                        return entry.score;
                    }
                }
                TranspositionTableFlag::Empty => {
                    unreachable!("Empty flag should not be returned by probe")
                }
            }
        }

        // Singular Extensions
        if depth >= 8
            && ext_control.excluded_move.is_null()
            && ext_control.extensions < 6
            && entry.depth >= depth - 3
            && entry.flag != TranspositionTableFlag::Alpha
            && !entry.score.abs().is_winning()
        {
            let rdepth = depth - 3;
            let rbeta = entry.score - singular_margin(depth);
            let score = negamax(
                pos,
                rdepth,
                ply,
                SearchWindow::new(rbeta - value!(1), rbeta),
                SearchExtension::new(ext_control.extensions, entry.best_move),
                ctx,
            );
            if score < rbeta {
                is_singular = true;
            }
        }

        entry.best_move
    } else {
        Move::null()
    };

    // Validate that the TT move is actually legal in the current position.
    // A stale or collided TT entry may reference a move that is not valid here.
    //
    // Assume the TT move is legal
    // if !tt_move.is_null() && !moves.contains(&tt_move) {
    //     tt_move = Move::none();
    // }

    // Base case: fall back to quiescence search
    if depth == 0 {
        return quiescence(pos, window, ply, 0, ctx);
    }

    let mut moves = MoveList::new();
    generate_moves(pos, MoveGenType::Legal, &mut moves);

    // Stalemate / Checkmate: In Xiangqi, a player with no legal moves loses.
    if moves.is_empty() {
        return Value::mated_in(ply);
    }

    // One-Reply Extensions
    if moves.len() == 1 && ext_control.excluded_move.is_null() && ext_control.extensions < 6 {
        depth += 1;
        ext_control.extensions += 1;
    }

    // Sort moves: prioritize captures via MVV-LVA Heuristic, with TT move
    // prioritized first, killers, and history
    sort_moves(pos, &mut moves, tt_move, ctx, ply);

    let mut best_score = -Value::INFINITY;
    let mut best_move = Move::null();

    for m in moves {
        if m == ext_control.excluded_move {
            continue;
        }

        let mut ext = 0;

        if m == tt_move && is_singular {
            ext = 1;
        }

        pos.do_move(m);
        let score = -negamax(
            pos,
            depth - 1 + ext,
            ply + 1,
            window.negate(),
            SearchExtension::new(ext_control.extensions + ext, Move::null()),
            ctx,
        );
        pos.undo_move(m);

        if ctx.stop.load(Ordering::Relaxed) {
            return Value::ZERO;
        }

        if score > best_score {
            best_score = score;
            best_move = m;
        }
        if best_score > window.alpha {
            window.alpha = best_score;
        }
        if window.alpha >= window.beta {
            // Update killers and history for quiet moves
            if pos.piece_at(m.square_to()) == Piece::None {
                let ply_idx = ply as usize;
                // We do not check for ply_idx out of bounds since
                // ply should not exceed MAX_PLY in normal circumstances
                if ctx.killers[ply_idx][0] != m {
                    ctx.killers[ply_idx][1] = ctx.killers[ply_idx][0];
                    ctx.killers[ply_idx][0] = m;
                }
                let side_idx = pos.side_to_move() as usize;
                let from_idx = m.square_from() as usize;
                let to_idx = m.square_to() as usize;
                ctx.history_table[side_idx][from_idx][to_idx] += (depth as i32) * (depth as i32);
            }
            break; // Beta cutoff
        }
    }

    // Cache search results to Transposition Table
    if !ctx.stop.load(Ordering::Relaxed) {
        let flag = if best_score >= window.beta {
            TranspositionTableFlag::Beta
        } else if best_score <= alpha_orig {
            TranspositionTableFlag::Alpha
        } else {
            TranspositionTableFlag::Exact
        };
        ctx.transposition_table.store(
            pos.zobrist_hash(),
            ply,
            tt_entry_value!(best_score, flag, best_move, depth, ctx.age),
        );
    }

    best_score
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
        let mut nodes = 0;
        let mut transposition_table = TranspositionTable::new(1);
        let mut killers = [[Move::null(); 2]; MAX_PLY];
        let mut history_table = [[[0; 90]; 90]; 2];
        let mut max_ply = 0;
        let mut ctx = SearchContext {
            stop: &stop,
            nodes: &mut nodes,
            start_time: Instant::now(),
            time_limit: None,
            transposition_table: &mut transposition_table,
            age: 1,
            killers: &mut killers,
            history_table: &mut history_table,
            max_ply: &mut max_ply,
        };
        let score = negamax(
            &mut pos,
            1,
            6,
            SearchWindow::new(-Value::INFINITY, Value::INFINITY),
            SearchExtension::default(),
            &mut ctx,
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
        let mut nodes = 0;
        let mut transposition_table = TranspositionTable::new(1);
        let mut killers = [[Move::null(); 2]; MAX_PLY];
        let mut history_table = [[[0; 90]; 90]; 2];
        let mut max_ply = 0;
        let mut ctx = SearchContext {
            stop: &stop,
            nodes: &mut nodes,
            start_time: Instant::now(),
            time_limit: None,
            transposition_table: &mut transposition_table,
            age: 1,
            killers: &mut killers,
            history_table: &mut history_table,
            max_ply: &mut max_ply,
        };
        let score = negamax(
            &mut pos,
            1,
            5,
            SearchWindow::new(-Value::INFINITY, Value::INFINITY),
            SearchExtension::default(),
            &mut ctx,
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
        let params = GoParameters {
            depth: Some(2),
            ..GoParameters::default()
        };

        let (best_move, _score, _nodes) = search(pos, params, &mut tt, 1, tx, None);

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
