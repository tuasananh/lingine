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
pub fn quiescence(pos: &mut Position, mut alpha: i32, beta: i32, ctx: &mut SearchContext) -> i32 {
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
        let score = -quiescence(pos, -beta, -alpha, ctx);
        pos.undo_move(m);

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

    // Repetition check (returns 0 for draw)
    if pos.is_repetition() {
        return 0;
    }

    // Base case: fall back to quiescence search
    if depth <= 0 {
        return quiescence(pos, alpha, beta, ctx);
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
