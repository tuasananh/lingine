use super::packed;
use crate::core::{
    Bitboard, PackedScore, PieceType, Position, Side, cannon_captures, knight_attacks, rook_attacks,
};

const KNIGHT_MOBILITY_BONUS: [PackedScore; 9] = packed![
    (-10, -15),
    (-5, -8),
    (0, 0),
    (4, 6),
    (8, 12),
    (12, 18),
    (16, 24),
    (20, 30),
    (24, 36),
];

const ROOK_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-20, -30),
    (-15, -22),
    (-10, -15),
    (-5, -7),
    (0, 0),
    (4, 6),
    (8, 12),
    (12, 18),
    (16, 24),
    (20, 30),
    (24, 36),
    (28, 42),
    (30, 46),
    (32, 50),
    (34, 54),
    (36, 58),
    (38, 62),
    (40, 66),
];

const CANNON_MOBILITY_BONUS: [PackedScore; 18] = packed![
    (-15, -20),
    (-10, -13),
    (-5, -7),
    (0, 0),
    (4, 5),
    (8, 10),
    (12, 15),
    (16, 20),
    (20, 25),
    (24, 30),
    (28, 35),
    (30, 40),
    (32, 44),
    (34, 48),
    (36, 52),
    (38, 56),
    (40, 60),
    (42, 64),
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
