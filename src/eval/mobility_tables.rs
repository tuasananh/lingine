use super::packed;
use crate::core::{
    Bitboard, PackedScore, PieceType, Position, Side, cannon_captures, knight_attacks, rook_attacks,
};

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

pub(in crate::eval) const KNIGHT_MOBILITY_BONUS: [PackedScore; 9] = packed![
    (-22, -45),
    (-16, -18),
    (-10, -10),
    (2, 4),
    (11, 15),
    (15, 21),
    (19, 27),
    (25, 33),
    (26, 38)
];

pub(in crate::eval) const ROOK_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-5, -9),
    (11, -27),
    (-15, -27),
    (-4, -5),
    (3, -5),
    (16, 18),
    (20, 20),
    (10, 17),
    (25, 35),
    (18, 34),
    (25, 37),
    (28, 43),
    (34, 50),
    (29, 49),
    (24, 47),
    (35, 41),
    (21, 45),
    (20, 46)
];

pub(in crate::eval) const CANNON_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-20, -6),
    (1, -77),
    (7, 8),
    (8, 15),
    (15, 16),
    (22, 24),
    (18, 21),
    (21, 23),
    (21, 30),
    (23, 34),
    (19, 28),
    (26, 36),
    (21, 33),
    (28, 42),
    (17, 48),
    (18, 42),
    (41, 41),
    (1, 22)
];
