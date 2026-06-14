use crate::{
    core::{PackedScore, Piece, Position},
    eval::{eval_params::EvalParams, packed},
};

/// Computes defender count bonuses for both sides.
#[inline]
pub(in crate::eval) fn compute_defender_bonus(pos: &Position) -> PackedScore {
    let mut score = PackedScore::ZERO;

    // Red defenders
    let red_advisors = pos.piece_count(Piece::RedAdvisor) as usize;
    let red_bishops = pos.piece_count(Piece::RedBishop) as usize;
    score += ADVISOR_COUNT_BONUS[red_advisors.min(2)];
    score += BISHOP_COUNT_BONUS[red_bishops.min(2)];

    // Black defenders
    let black_advisors = pos.piece_count(Piece::BlackAdvisor) as usize;
    let black_bishops = pos.piece_count(Piece::BlackBishop) as usize;
    score -= ADVISOR_COUNT_BONUS[black_advisors.min(2)];
    score -= BISHOP_COUNT_BONUS[black_bishops.min(2)];

    score
}

#[inline]
pub(in crate::eval) fn compute_defender_bonus_with_params(
    pos: &Position,
    params: &EvalParams,
) -> PackedScore {
    let mut score = PackedScore::ZERO;

    // Red defenders
    let red_advisors = pos.piece_count(Piece::RedAdvisor) as usize;
    let red_bishops = pos.piece_count(Piece::RedBishop) as usize;
    score += params.advisor_count_bonus[red_advisors.min(2)];
    score += params.bishop_count_bonus[red_bishops.min(2)];

    // Black defenders
    let black_advisors = pos.piece_count(Piece::BlackAdvisor) as usize;
    let black_bishops = pos.piece_count(Piece::BlackBishop) as usize;
    score -= params.advisor_count_bonus[black_advisors.min(2)];
    score -= params.bishop_count_bonus[black_bishops.min(2)];

    score
}

// Tapered bonuses for having 0, 1, or 2 Advisors
pub(in crate::eval) const ADVISOR_COUNT_BONUS: [PackedScore; 3] =
    packed![(-70, -42), (31, -1), (22, 39)];

// Tapered bonuses for having 0, 1, or 2 Bishops (Elephants)
pub(in crate::eval) const BISHOP_COUNT_BONUS: [PackedScore; 3] =
    packed![(-10, -53), (7, 8), (23, 46)];
