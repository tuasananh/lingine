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
    (-50, -50),
    (-30, -30),
    (-10, -10),
    (0, 0),
    (10, 10),
    (20, 20),
    (30, 30),
    (40, 40),
    (50, 50)
];

pub(in crate::eval) const ROOK_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-50, -50),
    (-40, -40),
    (-30, -30),
    (-20, -20),
    (-10, -10),
    (0, 0),
    (5, 5),
    (10, 10),
    (15, 15),
    (20, 20),
    (25, 25),
    (30, 30),
    (35, 35),
    (40, 40),
    (45, 45),
    (50, 50),
    (55, 55),
    (60, 60)
];

pub(in crate::eval) const CANNON_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-50, -50),
    (-40, -40),
    (-30, -30),
    (-20, -20),
    (-10, -10),
    (0, 0),
    (5, 5),
    (10, 10),
    (15, 15),
    (20, 20),
    (25, 25),
    (30, 30),
    (35, 35),
    (40, 40),
    (45, 45),
    (50, 50),
    (55, 55),
    (60, 60)
];
