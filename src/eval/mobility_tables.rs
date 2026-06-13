use super::packed;
use crate::core::{
    Bitboard, PackedScore, PieceType, Position, Side, cannon_captures, knight_attacks, rook_attacks,
};

pub(in crate::eval) const KNIGHT_MOBILITY_BONUS: [PackedScore; 9] = packed![
    (-20, -34),
    (-15, -16),
    (-8, -8),
    (3, 5),
    (11, 15),
    (14, 20),
    (19, 27),
    (25, 33),
    (25, 37)
];

pub(in crate::eval) const ROOK_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-12, -24),
    (9, -24),
    (-18, -26),
    (-6, -7),
    (1, -3),
    (16, 18),
    (18, 21),
    (10, 16),
    (23, 33),
    (17, 30),
    (24, 36),
    (28, 43),
    (34, 50),
    (29, 49),
    (26, 47),
    (36, 46),
    (23, 47),
    (24, 50)
];

pub(in crate::eval) const CANNON_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-15, -13),
    (-7, -54),
    (6, 5),
    (7, 12),
    (13, 14),
    (20, 22),
    (18, 21),
    (20, 24),
    (21, 30),
    (24, 33),
    (21, 30),
    (26, 36),
    (24, 36),
    (29, 43),
    (21, 48),
    (24, 45),
    (42, 48),
    (6, 27)
];

fn compute_side_mobility(pos: &Position, side: Side, occupied: Bitboard) -> PackedScore {
    let mut score = PackedScore::ZERO;
    let friendly = pos.bitboard_by_color(side);
    let enemy = pos.bitboard_by_color(side.opposite());

    // Knights
    let mut knights = pos.bitboard_by_type(PieceType::Knight) & friendly;
    while let Some(from) = knights.pop_lsb() {
        let attacks = knight_attacks(from, occupied) & !friendly;
        let count = attacks.count_ones() as usize;
        score += KNIGHT_MOBILITY_BONUS[count];
    }

    // Rooks
    let mut rooks = pos.bitboard_by_type(PieceType::Rook) & friendly;
    while let Some(from) = rooks.pop_lsb() {
        let attacks = rook_attacks(from, occupied) & !friendly;
        let count = attacks.count_ones() as usize;
        score += ROOK_MOBILITY_BONUS[count];
    }

    // Cannons
    let mut cannons = pos.bitboard_by_type(PieceType::Cannon) & friendly;
    while let Some(from) = cannons.pop_lsb() {
        let attacks =
            (rook_attacks(from, occupied) & !occupied) | (cannon_captures(from, occupied) & enemy);
        let count = attacks.count_ones() as usize;
        score += CANNON_MOBILITY_BONUS[count];
    }

    score
}

/// Computes the mobility score for Knights, Cannons, and Rooks for both sides.
pub fn compute_mobility_score(pos: &Position) -> PackedScore {
    let occupied = pos.bitboard_occupied();
    let red_mobility = compute_side_mobility(pos, Side::Red, occupied);
    let black_mobility = compute_side_mobility(pos, Side::Black, occupied);
    red_mobility - black_mobility
}

fn compute_side_mobility_with_params(
    pos: &Position,
    side: Side,
    occupied: Bitboard,
    params: &super::EvalParams,
) -> PackedScore {
    let mut score = PackedScore::ZERO;
    let friendly = pos.bitboard_by_color(side);
    let enemy = pos.bitboard_by_color(side.opposite());

    // Knights
    let mut knights = pos.bitboard_by_type(PieceType::Knight) & friendly;
    while let Some(from) = knights.pop_lsb() {
        let attacks = knight_attacks(from, occupied) & !friendly;
        let count = attacks.count_ones() as usize;
        score += params.knight_mobility[count];
    }

    // Rooks
    let mut rooks = pos.bitboard_by_type(PieceType::Rook) & friendly;
    while let Some(from) = rooks.pop_lsb() {
        let attacks = rook_attacks(from, occupied) & !friendly;
        let count = attacks.count_ones() as usize;
        score += params.rook_mobility[count];
    }

    // Cannons
    let mut cannons = pos.bitboard_by_type(PieceType::Cannon) & friendly;
    while let Some(from) = cannons.pop_lsb() {
        let attacks =
            (rook_attacks(from, occupied) & !occupied) | (cannon_captures(from, occupied) & enemy);
        let count = attacks.count_ones() as usize;
        score += params.cannon_mobility[count];
    }

    score
}

pub fn compute_mobility_score_with_params(
    pos: &Position,
    params: &super::EvalParams,
) -> PackedScore {
    let occupied = pos.bitboard_occupied();
    let red_mobility = compute_side_mobility_with_params(pos, Side::Red, occupied, params);
    let black_mobility = compute_side_mobility_with_params(pos, Side::Black, occupied, params);
    red_mobility - black_mobility
}
