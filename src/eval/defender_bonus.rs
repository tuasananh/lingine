use crate::{
    core::{PackedScore, Piece, Position},
    eval::{eval_params::EvalParams, packed},
};

/// Computes defender count bonuses for both sides.
#[inline]
pub(in crate::eval) fn compute_defender_bonus(pos: &Position) -> PackedScore {
    // Red defenders
    ADVISOR_COUNT_BONUS[pos.piece_count(Piece::RedAdvisor) as usize]
    + BISHOP_COUNT_BONUS[pos.piece_count(Piece::RedBishop) as usize]
    // Black defenders
    - ADVISOR_COUNT_BONUS[pos.piece_count(Piece::BlackAdvisor) as usize]
    - BISHOP_COUNT_BONUS[pos.piece_count(Piece::BlackBishop) as usize]
}

#[inline]
pub(in crate::eval) fn compute_defender_bonus_with_params(
    pos: &Position,
    params: &EvalParams,
) -> PackedScore {
    // Red defenders
    params.advisor_count_bonus[pos.piece_count(Piece::RedAdvisor) as usize]
    + params.bishop_count_bonus[pos.piece_count(Piece::RedBishop) as usize]
    // Black defenders
    - params.advisor_count_bonus[pos.piece_count(Piece::BlackAdvisor) as usize]
    - params.bishop_count_bonus[pos.piece_count(Piece::BlackBishop) as usize]
}

// Tapered bonuses for having 0, 1, or 2 Advisors
pub(in crate::eval) const ADVISOR_COUNT_BONUS: [PackedScore; 3] =
    packed![(-45, 34), (50, 18), (26, -14)];

// Tapered bonuses for having 0, 1, or 2 Bishops (Elephants)
pub(in crate::eval) const BISHOP_COUNT_BONUS: [PackedScore; 3] =
    packed![(-23, 40), (13, 17), (48, -25)];
