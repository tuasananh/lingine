use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::core::movegen::generate_moves;
use crate::core::{
    Position,
    types::{Color, MAX_MOVES, Move, MoveGenType, Piece, PieceType},
};
use crate::eval::evaluate;

pub mod transposition_table;
pub use transposition_table::{
    TranspositionTable, TranspositionTableEntry, TranspositionTableFlag,
};

pub const INFINITY: i32 = 1_000_000;
pub const MATE_VALUE: i32 = 100_000;

/// Shared context parameters passed down the recursive search stack.
pub struct SearchContext<'a> {
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
    pub killers: &'a mut [[Move; 2]; 128],
    /// History heuristic table to prioritize frequently successful quiet moves.
    pub history_table: &'a mut [[[i32; 90]; 90]; 2],
}

/// Simple helper to rank piece types for MVV-LVA move ordering.
fn get_piece_value_rank(p: Piece) -> i32 {
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

/// Returns a heuristic move-ordering score. Captures are scored highly based
/// on MVV-LVA. Quiet moves get score 0. The transposition table best move is
/// prioritized at the very top.
fn get_move_score(
    pos: &Position,
    m: Move,
    tt_move: Move,
    killers: &[[Move; 2]; 128],
    history_table: &[[[i32; 90]; 90]; 2],
    ply: i32,
) -> i32 {
    if m == tt_move && !m.is_none() {
        return 20000; // Prioritize TT best move above all else
    }
    let to_piece = pos.piece_at(m.square_to());
    if to_piece != Piece::None {
        // Capture: 10000 + victim_rank * 100 - attacker_rank
        let victim = get_piece_value_rank(to_piece);
        let attacker = get_piece_value_rank(pos.piece_at(m.square_from()));
        10000 + victim * 100 - attacker
    } else {
        // Quiet move
        let ply_idx = ply as usize;
        if ply_idx < 128 {
            if m == killers[ply_idx][0] {
                return 9000;
            }
            if m == killers[ply_idx][1] {
                return 8000;
            }
        }
        let side_idx = pos.side_to_move() as usize;
        let from_idx = m.square_from() as usize;
        let to_idx = m.square_to() as usize;
        // History score capped at 7000
        history_table[side_idx][from_idx][to_idx]
    }
}

/// Sorts the first `count` moves in `moves` using a simple selection sort
/// based on their heuristic scores, prioritizing the transposition table best move, killers, and history.
fn sort_moves(
    pos: &Position,
    moves: &mut [Move],
    count: usize,
    tt_move: Move,
    killers: &[[Move; 2]; 128],
    history_table: &[[[i32; 90]; 90]; 2],
    ply: i32,
) {
    for i in 0..count {
        let mut best_idx = i;
        let mut best_val = get_move_score(pos, moves[i], tt_move, killers, history_table, ply);
        for (j, mv) in moves.iter().enumerate().take(count).skip(i + 1) {
            let val = get_move_score(pos, *mv, tt_move, killers, history_table, ply);
            if val > best_val {
                best_val = val;
                best_idx = j;
            }
        }
        if best_idx != i {
            moves.swap(i, best_idx);
        }
    }
}

/// Implements Quiescence Search to evaluate tactical capture sequences.
///
/// Prevents the horizon effect by searching captures only until a quiet position is reached.
pub fn quiescence(
    pos: &mut Position,
    mut alpha: i32,
    beta: i32,
    ply: i32,
    qdepth: i32,
    ctx: &mut SearchContext,
) -> i32 {
    if ctx.stop.load(Ordering::Relaxed) {
        return 0;
    }
    *ctx.nodes += 1;

    // Check stop signals periodically
    if *ctx.nodes & 1023 == 0 {
        if ctx.stop.load(Ordering::Relaxed) {
            return 0;
        }
        if let Some(limit) = ctx.time_limit
            && ctx.start_time.elapsed() >= limit
        {
            ctx.stop.store(true, Ordering::Relaxed);
            return 0;
        }
    }

    // Base case: to avoid infinite recursion and stack overflow from perpetual checks
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

        if eval_side >= beta {
            return eval_side;
        }
        if eval_side > alpha {
            alpha = eval_side;
        }
    }

    // Generate moves: if in check, we must search all legal evasions to save the King.
    // Otherwise, we only search capture moves.
    let mut moves = [Move::none(); MAX_MOVES];
    let count = if in_check {
        generate_moves(pos, MoveGenType::Legal, &mut moves)
    } else {
        generate_moves(pos, MoveGenType::Captures, &mut moves)
    };

    // Checkmate detection: in check with no legal moves = checkmate
    if in_check && count == 0 {
        return -MATE_VALUE + ply;
    }

    // Sort captures using MVV-LVA
    sort_moves(
        pos,
        &mut moves,
        count,
        Move::none(),
        ctx.killers,
        ctx.history_table,
        ply,
    );

    for m in moves.iter().copied().take(count) {
        pos.do_move(m);
        let score = -quiescence(pos, -beta, -alpha, ply + 1, qdepth + 1, ctx);
        pos.undo_move(m);

        if ctx.stop.load(Ordering::Relaxed) {
            return 0;
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

/// Performs Fail-Soft Alpha-Beta Negamax Search to a specific depth.
pub fn negamax(
    pos: &mut Position,
    depth: i32,
    ply: i32,
    mut alpha: i32,
    beta: i32,
    ctx: &mut SearchContext,
) -> i32 {
    if ctx.stop.load(Ordering::Relaxed) {
        return 0;
    }
    *ctx.nodes += 1;

    // Check stop signals periodically
    if *ctx.nodes & 1023 == 0 {
        if ctx.stop.load(Ordering::Relaxed) {
            return 0;
        }
        if let Some(limit) = ctx.time_limit
            && ctx.start_time.elapsed() >= limit
        {
            ctx.stop.store(true, Ordering::Relaxed);
            return 0;
        }
    }

    // Game over / rule evaluations (60-move rule, insufficient material, repetitions, perpetual checks)
    if let Some(rule_score) = pos.rule_judge(ply) {
        return rule_score;
    }

    let alpha_orig = alpha;

    // Transposition Table Probing
    let mut tt_move = Move::none();
    if let Some(entry) = ctx.transposition_table.probe(pos.zobrist_hash) {
        tt_move = entry.best_move;
        if entry.depth >= depth as i16 {
            let tt_score = TranspositionTable::score_from_transposition(entry.score, ply);
            match entry.flag {
                TranspositionTableFlag::Exact => return tt_score,
                TranspositionTableFlag::Alpha => {
                    if tt_score <= alpha {
                        return tt_score;
                    }
                }
                TranspositionTableFlag::Beta => {
                    if tt_score >= beta {
                        return tt_score;
                    }
                }
            }
        }
    }

    // Base case: fall back to quiescence search
    if depth <= 0 {
        return quiescence(pos, alpha, beta, ply, 0, ctx);
    }

    let mut moves = [Move::none(); MAX_MOVES];
    let count = generate_moves(pos, MoveGenType::Legal, &mut moves);

    // Stalemate / Checkmate: In Xiangqi, a player with no legal moves loses.
    if count == 0 {
        return -MATE_VALUE + ply;
    }

    // Validate that the TT move is actually legal in the current position.
    // A stale or collided TT entry may reference a move that is not valid here.
    if !tt_move.is_none() && !moves[..count].contains(&tt_move) {
        tt_move = Move::none();
    }

    // Sort moves: prioritize captures via MVV-LVA Heuristic, with TT move prioritized first, killers, and history
    sort_moves(
        pos,
        &mut moves,
        count,
        tt_move,
        ctx.killers,
        ctx.history_table,
        ply,
    );

    let mut best_score = -INFINITY;
    let mut best_move = Move::none();

    for m in moves.iter().copied().take(count) {
        pos.do_move(m);
        let score = -negamax(pos, depth - 1, ply + 1, -beta, -alpha, ctx);
        pos.undo_move(m);

        if ctx.stop.load(Ordering::Relaxed) {
            return 0;
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
            if pos.piece_at(m.square_to()) == Piece::None {
                let ply_idx = ply as usize;
                if ply_idx < 128 && ctx.killers[ply_idx][0] != m {
                    ctx.killers[ply_idx][1] = ctx.killers[ply_idx][0];
                    ctx.killers[ply_idx][0] = m;
                }
                let side_idx = pos.side_to_move() as usize;
                let from_idx = m.square_from() as usize;
                let to_idx = m.square_to() as usize;
                ctx.history_table[side_idx][from_idx][to_idx] =
                    (ctx.history_table[side_idx][from_idx][to_idx] + depth * depth).min(7000);
            }
            break; // Beta cutoff
        }
    }

    // Cache search results to Transposition Table
    if !ctx.stop.load(Ordering::Relaxed) {
        let flag = if best_score >= beta {
            TranspositionTableFlag::Beta
        } else if best_score <= alpha_orig {
            TranspositionTableFlag::Alpha
        } else {
            TranspositionTableFlag::Exact
        };
        ctx.transposition_table.store(
            pos.zobrist_hash,
            depth as i16,
            TranspositionTable::score_to_transposition(best_score, ply),
            flag,
            best_move,
            ctx.age,
        );
    }

    best_score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::position::Position;
    use crate::core::types::{Color, Move, Square};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    #[test]
    fn test_repetition_draw_repetition() {
        let mut pos = Position::new();
        // Setup a simple position with files D and E blocked by pawns to avoid King-facing checks
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
        assert_eq!(pos.rule_judge(6), Some(0));

        // Call negamax with depth=1, we should get 0 (draw)
        let stop = Arc::new(AtomicBool::new(false));
        let mut nodes = 0;
        let mut transposition_table = TranspositionTable::new(1);
        let mut killers = [[Move::none(); 2]; 128];
        let mut history_table = [[[0; 90]; 90]; 2];
        let mut ctx = SearchContext {
            stop: &stop,
            nodes: &mut nodes,
            start_time: Instant::now(),
            time_limit: None,
            transposition_table: &mut transposition_table,
            age: 1,
            killers: &mut killers,
            history_table: &mut history_table,
        };
        let score = negamax(&mut pos, 1, 6, -INFINITY, INFINITY, &mut ctx);
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

        // Now Black turn to move. White just gave the repeating check on all turns in the loop.
        assert_eq!(pos.rule_judge(5), Some(100_000 - 5));
        assert!(pos.is_in_check(Color::Black));

        // Black should win because White is perpetually checking!
        // negamax should return a win score (MATE_VALUE - ply)
        let stop = Arc::new(AtomicBool::new(false));
        let mut nodes = 0;
        let mut transposition_table = TranspositionTable::new(1);
        let mut killers = [[Move::none(); 2]; 128];
        let mut history_table = [[[0; 90]; 90]; 2];
        let mut ctx = SearchContext {
            stop: &stop,
            nodes: &mut nodes,
            start_time: Instant::now(),
            time_limit: None,
            transposition_table: &mut transposition_table,
            age: 1,
            killers: &mut killers,
            history_table: &mut history_table,
        };
        let score = negamax(&mut pos, 1, 5, -INFINITY, INFINITY, &mut ctx);
        assert_eq!(score, MATE_VALUE - 5);
    }
}
