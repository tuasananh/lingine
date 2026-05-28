use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::core::movegen::generate_moves;
use crate::core::{
    Position,
    types::{Color, MAX_MOVES, Move, MoveGenType, Piece, PieceType},
};
use crate::eval::evaluate;

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
/// on MVV-LVA. Quiet moves get score 0.
fn get_move_score(pos: &Position, m: Move) -> i32 {
    let to_piece = pos.piece_at(m.square_to());
    if to_piece != Piece::None {
        // Capture: 10000 + victim_rank * 100 - attacker_rank
        let victim = get_piece_value_rank(to_piece);
        let attacker = get_piece_value_rank(pos.piece_at(m.square_from()));
        10000 + victim * 100 - attacker
    } else {
        0
    }
}

/// Sorts the first `count` moves in `moves` using a simple selection sort
/// based on their heuristic scores.
fn sort_moves(pos: &Position, moves: &mut [Move], count: usize) {
    for i in 0..count {
        let mut best_idx = i;
        let mut best_val = get_move_score(pos, moves[i]);
        for (j, mv) in moves.iter().enumerate().take(count).skip(i + 1) {
            let val = get_move_score(pos, *mv);
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
pub fn quiescence(pos: &mut Position, mut alpha: i32, beta: i32, qdepth: i32, ctx: &mut SearchContext) -> i32 {
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
        return if pos.side_to_move() == Color::White { stand_pat } else { -stand_pat };
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

    // Sort captures using MVV-LVA
    sort_moves(pos, &mut moves, count);

    for m in moves.iter().copied().take(count) {
        pos.do_move(m);
        let score = -quiescence(pos, -beta, -alpha, qdepth + 1, ctx);
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

    // Repetition check (Xiangqi rules: perpetual check is a loss for the checking side)
    if pos.is_repetition() {
        return if pos.is_in_check(pos.side_to_move()) {
            MATE_VALUE - ply
        } else {
            0
        };
    }

    // Base case: fall back to quiescence search
    if depth <= 0 {
        return quiescence(pos, alpha, beta, 0, ctx);
    }

    let mut moves = [Move::none(); MAX_MOVES];
    let count = generate_moves(pos, MoveGenType::Legal, &mut moves);

    // Stalemate / Checkmate: In Xiangqi, a player with no legal moves loses.
    if count == 0 {
        return -MATE_VALUE + ply;
    }

    // Sort moves: prioritize captures via MVV-LVA Heuristic
    sort_moves(pos, &mut moves, count);

    let mut best_score = -INFINITY;

    for m in moves.iter().copied().take(count) {
        pos.do_move(m);
        let score = -negamax(pos, depth - 1, ply + 1, -beta, -alpha, ctx);
        pos.undo_move(m);

        if ctx.stop.load(Ordering::Relaxed) {
            return 0;
        }

        if score > best_score {
            best_score = score;
        }
        if best_score > alpha {
            alpha = best_score;
        }
        if alpha >= beta {
            break; // Beta cutoff
        }
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
        assert!(pos.is_repetition());

        // Call negamax with depth=1, we should get 0 (draw)
        let stop = Arc::new(AtomicBool::new(false));
        let mut nodes = 0;
        let mut ctx = SearchContext {
            stop: &stop,
            nodes: &mut nodes,
            start_time: Instant::now(),
            time_limit: None,
        };
        let score = negamax(&mut pos, 1, 6, -INFINITY, INFINITY, &mut ctx);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_repetition_perpetual_check_repetition() {
        let mut pos = Position::new();
        // White King at E0, White Rook at A1, Black King at D9, White Pawn at E4 (to block E-file)
        pos.set("3k5/9/9/9/4P4/9/9/9/R8/4K4 w - - 0 1").unwrap();

        // White Rook checks: A1 to D1 (giving check)
        let r_check1 = Move::new(Square::A1, Square::D1);
        let r_evade1 = Move::new(Square::D1, Square::A1);
        // Black King evades: D9 to E9 (not checking)
        let k_move1 = Move::new(Square::D9, Square::E9);
        let k_move2 = Move::new(Square::E9, Square::D9);

        // 1. White checks
        pos.do_move(r_check1);
        assert!(pos.is_in_check(Color::Black));

        // 2. Black evades
        pos.do_move(k_move1);

        // 3. White moves Rook back to A1 (no check)
        pos.do_move(r_evade1);

        // 4. Black moves King back to D9
        pos.do_move(k_move2);

        // 5. White checks again with Rook to D1 (repetition + check!)
        pos.do_move(r_check1);

        // Now Black turn to move. White just gave the repeating check.
        assert!(pos.is_repetition());
        assert!(pos.is_in_check(Color::Black));

        // Black should win because White is perpetually checking!
        // negamax should return a win score (MATE_VALUE - ply)
        let stop = Arc::new(AtomicBool::new(false));
        let mut nodes = 0;
        let mut ctx = SearchContext {
            stop: &stop,
            nodes: &mut nodes,
            start_time: Instant::now(),
            time_limit: None,
        };
        let score = negamax(&mut pos, 1, 5, -INFINITY, INFINITY, &mut ctx);
        assert_eq!(score, MATE_VALUE - 5);
    }
}
