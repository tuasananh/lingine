use crate::core::{
    Position,
    types::{Color, PieceType, Value},
};

/// Base material values for each piece type in centipawns.
const VAL_ROOK: i32 = 600;
const VAL_CANNON: i32 = 285;
const VAL_KNIGHT: i32 = 270;
const VAL_BISHOP: i32 = 120; // Elephant
const VAL_ADVISOR: i32 = 110;
const VAL_PAWN_UNCROSSED: i32 = 30;
const VAL_PAWN_CROSSED: i32 = 70;

/// Performs static evaluation of the given position in centipawns from
/// White's perspective.
///
/// Positive values indicate an advantage for White, while negative values
/// indicate an advantage for Black.
pub fn evaluate(pos: &Position) -> Value {
    let mut score = 0;

    // Evaluate sliding and leaping piece material
    score += VAL_ROOK
        * (pos.bitboard_by_type(PieceType::Rook) & pos.bitboard_by_color(Color::White)).count_ones()
            as i32;
    score -= VAL_ROOK
        * (pos.bitboard_by_type(PieceType::Rook) & pos.bitboard_by_color(Color::Black)).count_ones()
            as i32;

    score += VAL_CANNON
        * (pos.bitboard_by_type(PieceType::Cannon) & pos.bitboard_by_color(Color::White))
            .count_ones() as i32;
    score -= VAL_CANNON
        * (pos.bitboard_by_type(PieceType::Cannon) & pos.bitboard_by_color(Color::Black))
            .count_ones() as i32;

    score += VAL_KNIGHT
        * (pos.bitboard_by_type(PieceType::Knight) & pos.bitboard_by_color(Color::White))
            .count_ones() as i32;
    score -= VAL_KNIGHT
        * (pos.bitboard_by_type(PieceType::Knight) & pos.bitboard_by_color(Color::Black))
            .count_ones() as i32;

    score += VAL_BISHOP
        * (pos.bitboard_by_type(PieceType::Bishop) & pos.bitboard_by_color(Color::White))
            .count_ones() as i32;
    score -= VAL_BISHOP
        * (pos.bitboard_by_type(PieceType::Bishop) & pos.bitboard_by_color(Color::Black))
            .count_ones() as i32;

    score += VAL_ADVISOR
        * (pos.bitboard_by_type(PieceType::Advisor) & pos.bitboard_by_color(Color::White))
            .count_ones() as i32;
    score -= VAL_ADVISOR
        * (pos.bitboard_by_type(PieceType::Advisor) & pos.bitboard_by_color(Color::Black))
            .count_ones() as i32;

    // Evaluate White Pawns (Red Pawns, advancing upwards)
    let mut white_pawns =
        pos.bitboard_by_type(PieceType::Pawn) & pos.bitboard_by_color(Color::White);
    while let Some(sq) = white_pawns.pop_lsb() {
        let is_crossed = sq.rank() as u8 >= 5;
        score += if is_crossed {
            VAL_PAWN_CROSSED
        } else {
            VAL_PAWN_UNCROSSED
        };
    }

    // Evaluate Black Pawns (advancing downwards)
    let mut black_pawns =
        pos.bitboard_by_type(PieceType::Pawn) & pos.bitboard_by_color(Color::Black);
    while let Some(sq) = black_pawns.pop_lsb() {
        let is_crossed = sq.rank() as u8 <= 4;
        score -= if is_crossed {
            VAL_PAWN_CROSSED
        } else {
            VAL_PAWN_UNCROSSED
        };
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_material_evaluation() {
        let mut pos = Position::new();
        // Set standard starting FEN
        pos.set("rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1")
            .unwrap();
        // Since board is symmetric, evaluation must be exactly 0
        assert_eq!(evaluate(&pos), 0);
    }

    #[test]
    fn test_captured_rook_evaluation() {
        let mut pos = Position::new();
        // Standard starting FEN but without White's left Rook (at A0)
        pos.set("rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/1HEAKAEHR w - - 0 1")
            .unwrap();
        // White is down a Rook (-600)
        assert_eq!(evaluate(&pos), -600);
    }

    #[test]
    fn test_crossed_river_pawn_evaluation() {
        let mut pos = Position::new();
        // Position with White Pawn crossed at E5 (rank index 5), and Black Pawn crossed at E4 (rank index 4)
        // Set up White Pawn at E5 (index 49) and Black Pawn at E4 (index 40)
        // White Pawn is crossed: +70. Black Pawn is crossed: -70.
        // Balance = 0.
        pos.set("4k4/9/9/9/4p3p/4P4/9/9/9/4K4 w - - 0 1").unwrap();
        assert_eq!(pos.piece_count(crate::core::types::Piece::WhitePawn), 1);
        assert_eq!(pos.piece_count(crate::core::types::Piece::BlackPawn), 2); // E4, I4

        let mut pos2 = Position::new();
        // Simple case: just a White Pawn at E4 (uncrossed) vs. Black Pawn at E5 (uncrossed)
        pos2.set("4k4/9/9/9/4p4/4P4/9/9/9/4K4 w - - 0 1").unwrap();
        // Both are uncrossed: White Pawn rank 4 < 5 (+30). Black Pawn rank 5 > 4 (-30). Balance = 0.
        assert_eq!(evaluate(&pos2), 0);

        // Move White Pawn to E5 (crossed, +70) and keep Black Pawn at E5 (uncrossed, -30). Balance = +40.
        let mut pos3 = Position::new();
        pos3.set("4k4/9/9/4P4/4p4/9/9/9/9/4K4 w - - 0 1").unwrap();
        assert_eq!(evaluate(&pos3), 40);
    }
}
