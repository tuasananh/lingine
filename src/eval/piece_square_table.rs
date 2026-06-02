//! Precomputed Piece-Square Tables (PST) and static piece material scores.
//!
//! This module defines the positional weighting for all Xiangqi piece types.
//! Positional values are represented from White's (Red's) perspective, and are
//! automatically mirrored both vertically (ranks) and horizontally (files) for
//! Black pieces.
//!
//! Core components:
//! 1. **PST Tables**: Matrix matrices of size 90 mapping square indices to
//!    positional bonuses/penalties.
//!    - Advisors and Bishops are restricted to their valid Palace/side squares
//!      (unreachable squares are 0).
//!    - Pawns, Knights, Cannons, and Rooks receive progression-based bonuses.
//! 2. **Mirrored Black Coordinates**: Flips both the file (8 - file) and rank
//!    (9 - rank) to maintain symmetry.
//! 3. **Dynamic Material Values**:
//!    - Standard pieces: Rook (600), Cannon (285), Knight (270), Elephant
//!      (120), Advisor (110), King (0).
//!    - Pawn: Starts with 30 in its own territory, and increases to 70 once it
//!      crosses the river, reflecting its newly acquired sideways mobility.

use crate::core::{Color, Piece, PieceType, Square, Value};

macro_rules! values {
    ($($x:expr),* $(,)?) => {
        [$(Value::from_raw($x)),*]
    };
}

// Positional piece-square tables from White's (Red's) perspective on the
// 90-square board. Square indices are 0 to 89, rank-major order (rank * 9 +
// file).

#[rustfmt::skip]
const PIECE_SQUARE_TABLE_KING: [Value; 90] = values![
    // Rank 0
    0, 0, 0,  -3,   0,  -3, 0, 0, 0,
    // Rank 1
    0, 0, 0, -10, -15, -10, 0, 0, 0,
    // Rank 2
    0, 0, 0, -20, -25, -20, 0, 0, 0,
    // Ranks 3-9 (Outside palace, unreachable)
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[rustfmt::skip]
const PIECE_SQUARE_TABLE_ADVISOR: [Value; 90] = values![
    // Rank 0
    0, 0, 0,   0,   0,   0, 0, 0, 0,
    // Rank 1
    0, 0, 0,   0,  15,   0, 0, 0, 0,
    // Rank 2
    0, 0, 0,   5,   0,   5, 0, 0, 0,
    // Ranks 3-9 (Unreachable)
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[rustfmt::skip]
const PIECE_SQUARE_TABLE_BISHOP: [Value; 90] = values![
    // Rank 0
    0, 0,   0, 0, 0, 0,   0, 0, 0, // C0, G0 are files 2, 6
    // Rank 1
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    // Rank 2
    -2, 0, 0, 0, 10, 0, 0, 0, -2, // A2, E2, I2 are files 0, 4, 8
    // Rank 3
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    // Rank 4
    0, 0,   5, 0, 0, 0,   5, 0, 0, // C4, G4 are files 2, 6
    // Ranks 5-9 (Unreachable)
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[rustfmt::skip]
const PIECE_SQUARE_TABLE_KNIGHT: [Value; 90] = values![
    // Rank 0 (starting rank)
    -10, -10,  -5,  -5,  -5,  -5,  -5, -10, -10,
    // Rank 1
    -5,   0,   5,   5,   5,   5,   5,   0,  -5,
    // Rank 2
    -3,   5,  10,  10,  10,  10,  10,   5,  -3,
    // Rank 3
    -2,   5,  10,  12,  15,  12,  10,   5,  -2,
    // Rank 4
    -2,   8,  12,  15,  18,  15,  12,   8,  -2,
    // Rank 5 (crossed river)
    0,  10,  15,  20,  22,  20,  15,  10,   0,
    // Rank 6
    0,  12,  18,  22,  25,  22,  18,  12,   0,
    // Rank 7
    -2,  10,  15,  20,  22,  20,  15,  10,  -2,
    // Rank 8
    -5,   5,  10,  12,  15,  12,  10,   5,  -5,
    // Rank 9
    -10,  -5,   0,   0,   0,   0,   0,  -5, -10,
];

#[rustfmt::skip]
const PIECE_SQUARE_TABLE_ROOK: [Value; 90] = values![
    // Rank 0 (starting rank)
    -5,   0,   2,   5,   5,   5,   2,   0,  -5,
    // Rank 1
    0,   2,   5,   8,  10,   8,   5,   2,   0,
    // Rank 2
    0,   2,   5,   8,  10,   8,   5,   2,   0,
    // Rank 3
    2,   5,   8,  10,  12,  10,   8,   5,   2,
    // Rank 4
    2,   5,   8,  10,  12,  10,   8,   5,   2,
    // Rank 5 (crossed river)
    5,   8,  10,  12,  15,  12,  10,   8,   5,
    // Rank 6
    5,  10,  12,  15,  18,  15,  12,  10,   5,
    // Rank 7
    10,  12,  15,  18,  20,  18,  15,  12,  10,
    // Rank 8
    15,  18,  20,  22,  25,  22,  20,  18,  15,
    // Rank 9 (enemy back rank)
    10,  15,  18,  20,  22,  20,  18,  15,  10,
];

#[rustfmt::skip]
const PIECE_SQUARE_TABLE_CANNON: [Value; 90] = values![
    // Rank 0
    -5,   0,   0,   0,   0,   0,   0,   0,  -5,
    // Rank 1
    0,   5,   5,   5,   5,   5,   5,   5,   0,
    // Rank 2
    0,   5,   5,   8,   8,   8,   5,   5,   0,
    // Rank 3
    0,   5,   8,  10,  10,  10,   8,   5,   0,
    // Rank 4
    0,   5,   8,  10,  10,  10,   8,   5,   0,
    // Rank 5 (crossed river)
    2,   8,  10,  12,  12,  12,  10,   8,   2,
    // Rank 6
    2,   8,  10,  12,  12,  12,  10,   8,   2,
    // Rank 7
    0,   5,   8,  10,  10,  10,   8,   5,   0,
    // Rank 8
    0,   2,   5,   5,   5,   5,   5,   2,   0,
    // Rank 9 (enemy back rank)
    5,  10,  10,  10,  10,  10,  10,  10,   5,
];

#[rustfmt::skip]
const PIECE_SQUARE_TABLE_PAWN: [Value; 90] = values![
    // Ranks 0, 1, 2 (deep in own side)
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    // Rank 3 (approaching river)
    0, 0, 2, 0, 3, 0, 2, 0, 0,
    // Rank 4 (river bank)
    2, 2, 5, 5, 8, 5, 5, 2, 2,
    // Rank 5 (crossed river)
    5, 8, 12, 15, 18, 15, 12, 8, 5,
    // Rank 6 (advancing)
    8, 12, 18, 22, 25, 22, 18, 12, 8,
    // Rank 7 (palace entrance)
    10, 15, 22, 26, 30, 26, 22, 15, 10,
    // Rank 8 (deep threat)
    10, 18, 25, 30, 35, 30, 25, 18, 10,
    // Rank 9 (back rank - "dead pawn")
    5,  8, 10, 12, 15, 12, 10,  8,  5,
];

/// Returns the Piece-Square Table (PST) positional value for a given piece type
/// and color on a specific square. For Black pieces, the position is
/// automatically mirrored vertically.
#[inline]
pub fn piece_square_table_value(piece_type: PieceType, color: Color, sq: Square) -> Value {
    let index = if color == Color::White {
        sq as usize
    } else {
        // Mirror rank vertically: 9 - rank, and file horizontally: 8 - file
        let file = sq.file() as usize;
        let rank = sq.rank() as usize;
        let mirrored_rank = 9 - rank;
        let mirrored_file = 8 - file;
        mirrored_rank * 9 + mirrored_file
    };
    match piece_type {
        PieceType::King => PIECE_SQUARE_TABLE_KING[index],
        PieceType::Advisor => PIECE_SQUARE_TABLE_ADVISOR[index],
        PieceType::Bishop => PIECE_SQUARE_TABLE_BISHOP[index],
        PieceType::Knight => PIECE_SQUARE_TABLE_KNIGHT[index],
        PieceType::Rook => PIECE_SQUARE_TABLE_ROOK[index],
        PieceType::Cannon => PIECE_SQUARE_TABLE_CANNON[index],
        PieceType::Pawn => PIECE_SQUARE_TABLE_PAWN[index],
        _ => panic!("Invalid piece type for PST evaluation"),
    }
}

/// Returns a piece's base material value, dynamically adjusting Pawn values
/// based on whether they have crossed the river.
#[inline]
pub fn piece_material_value(piece: Piece, sq: Square) -> Value {
    let val = match piece {
        Piece::None => 0,
        Piece::WhiteRook | Piece::BlackRook => 600,
        Piece::WhiteCannon | Piece::BlackCannon => 285,
        Piece::WhiteKnight | Piece::BlackKnight => 270,
        Piece::WhiteBishop | Piece::BlackBishop => 120, // Elephant
        Piece::WhiteAdvisor | Piece::BlackAdvisor => 110,
        Piece::WhitePawn => {
            if sq.rank() as u8 >= 5 {
                70
            } else {
                30
            }
        }
        Piece::BlackPawn => {
            if sq.rank() as u8 <= 4 {
                70
            } else {
                30
            }
        }
        Piece::WhiteKing | Piece::BlackKing => 0, /* Treated as 0 for incremental score (kings
                                                   * never captured) */
    };

    Value::from_raw(val)
}
