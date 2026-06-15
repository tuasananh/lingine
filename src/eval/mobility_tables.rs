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
    (20, -127),
    (37, -66),
    (49, -49),
    (69, -40),
    (80, -47),
    (88, -37),
    (98, -55),
    (109, -54),
    (122, -64)
];

pub(in crate::eval) const ROOK_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-58, -51),
    (36, -54),
    (61, -3),
    (78, 3),
    (95, -1),
    (115, 0),
    (121, 1),
    (117, 44),
    (127, 39),
    (137, 37),
    (142, 27),
    (156, 12),
    (163, 10),
    (163, 14),
    (164, -15),
    (170, -43),
    (160, -37),
    (152, -41)
];

pub(in crate::eval) const CANNON_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-5, -47),
    (51, -6),
    (59, 5),
    (79, -17),
    (85, -19),
    (99, -24),
    (102, -18),
    (96, -6),
    (100, -16),
    (104, -15),
    (105, -10),
    (111, -18),
    (122, -28),
    (126, -25),
    (124, -30),
    (143, -39),
    (153, -49),
    (178, -63)
];
