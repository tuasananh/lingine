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
pub(super) fn compute_mobility_score(pos: &Position) -> PackedScore {
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

pub(super) fn compute_mobility_score_with_params(
    pos: &Position,
    params: &super::EvalParams,
) -> PackedScore {
    let occupied = pos.bitboard_occupied();
    let red_mobility = compute_side_mobility_with_params(pos, Side::Red, occupied, params);
    let black_mobility = compute_side_mobility_with_params(pos, Side::Black, occupied, params);
    red_mobility - black_mobility
}

pub(in crate::eval) const KNIGHT_MOBILITY_BONUS: [PackedScore; 9] = packed![
    (-39, -82),
    (-17, -41),
    (-6, 5),
    (19, 22),
    (27, 34),
    (38, 45),
    (47, 33),
    (58, 39),
    (56, 47)
];

pub(in crate::eval) const ROOK_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-58, -50),
    (-55, -47),
    (-39, -27),
    (-9, -8),
    (12, 2),
    (26, 19),
    (45, 20),
    (44, 64),
    (49, 69),
    (61, 75),
    (60, 80),
    (75, 82),
    (81, 88),
    (82, 94),
    (77, 70),
    (83, 34),
    (74, 41),
    (61, 47)
];

pub(in crate::eval) const CANNON_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-47, -55),
    (6, 0),
    (9, 23),
    (36, 33),
    (43, 43),
    (55, 50),
    (58, 58),
    (52, 65),
    (55, 62),
    (58, 65),
    (54, 75),
    (64, 59),
    (71, 51),
    (72, 61),
    (64, 61),
    (74, 63),
    (75, 56),
    (84, 47)
];
