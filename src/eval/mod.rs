//! Static position evaluation for Xiangqi.
//!
//! This module performs a static evaluation of a given board position, yielding
//! a `Value` in centipawns. The evaluation is conducted from Red's
//! perspective:
//! - Positive values favor Red.
//! - Negative values favor Black.
//!
//! The evaluation function is highly optimized and relies entirely on
//! incrementally updated scores: `evaluate = material_score +
//! piece_square_table_score`
//!
//! Details:
//! 1. **Material Score**: Cumulative weight of remaining pieces.
//! 2. **Piece-Square Table (PST) Score**: Positional values reflecting the
//!    developmental guidance, tactical positioning, and river crossing dynamics
//!    for each piece type.

mod piece_square_table;
pub use piece_square_table::*;

use crate::core::{Piece, Score, Square};

/// Returns a piece's base material value, dynamically adjusting Pawn values
/// based on whether they have crossed the river.
#[inline]
pub const fn piece_material_value(piece: Piece, sq: Square) -> Score {
    match piece {
        Piece::RedRook | Piece::BlackRook => 600,
        Piece::RedCannon | Piece::BlackCannon => 285,
        Piece::RedKnight | Piece::BlackKnight => 270,
        Piece::RedBishop | Piece::BlackBishop => 120, // Elephant
        Piece::RedAdvisor | Piece::BlackAdvisor => 110,
        Piece::RedPawn => {
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
        Piece::RedKing | Piece::BlackKing => 0, /* Treated as 0 for incremental score (kings
                                                   * never captured) */
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{MoveGenType, MoveList, Piece, Position, generate_moves, score};

    #[test]
    fn test_initial_material_evaluation() {
        let mut pos = Position::new();
        // Set standard starting FEN
        pos.set("rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1")
            .unwrap();
        // Since board is symmetric, evaluation must be exactly 0
        assert_eq!(pos.evaluate(), score::DRAW);
    }

    #[test]
    fn test_captured_rook_evaluation() {
        let mut pos = Position::new();
        // Standard starting FEN but without Red's left Rook (at A0)
        pos.set("rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/1HEAKAEHR w - - 0 1")
            .unwrap();
        // Red is down a Rook (-600). The Rook at A0 has a PST value of -5.
        // Therefore, without this Rook, Red loses both -600 material and -5
        // positional value, resulting in net: -600 - (-5) = -595.
        assert_eq!(pos.evaluate(), -595);
    }

    #[test]
    fn test_crossed_river_pawn_evaluation() {
        let mut pos = Position::new();
        // Position with Red Pawn crossed at E5 (rank index 5), and Black Pawn crossed
        // at E4 (rank index 4) Set up Red Pawn at E5 (index 49) and Black
        // Pawn at E4 (index 40) Red Pawn is crossed: +70. Black Pawn is
        // crossed: -70. Balance = 0.
        pos.set("4k4/9/9/9/4p3p/4P4/9/9/9/4K4 w - - 0 1").unwrap();
        assert_eq!(pos.piece_count(Piece::RedPawn), 1);
        assert_eq!(pos.piece_count(Piece::BlackPawn), 2); // E4, I4

        let mut pos2 = Position::new();
        // Simple case: just a Red Pawn at E4 (uncrossed) vs. Black Pawn at E5
        // (uncrossed)
        pos2.set("4k4/9/9/9/4p4/4P4/9/9/9/4K4 w - - 0 1").unwrap();
        // Both are uncrossed: Red Pawn rank 4 < 5 (+30). Black Pawn rank 5 > 4 (-30).
        // Balance = 0. Let's verify that symmetric position still yields 0
        assert_eq!(pos2.evaluate(), score::DRAW);

        // Move Red Pawn to E6 (crossed: material 70, PST: 25) and keep Black Pawn at
        // E5 (uncrossed: material -30, PST: -8). Net = 70 - 30 + 25 - 8 = 57.
        let mut pos3 = Position::new();
        pos3.set("4k4/9/9/4P4/4p4/9/9/9/9/4K4 w - - 0 1").unwrap();
        assert_eq!(pos3.evaluate(), 57);
    }

    #[test]
    fn test_incremental_evaluation_consistency() {
        let positions = [
            "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1".to_string(),
            "4k4/9/9/4P4/4p4/9/9/9/9/4K4 w - - 0 1".to_string(),
            "r1bakab1r/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1".to_string(),
        ];

        for (idx, fen) in positions.iter().enumerate() {
            let mut pos = Position::new();
            pos.set(fen).unwrap();

            // Generate all pseudo-legal moves
            let mut moves = MoveList::new();
            generate_moves(&pos, MoveGenType::PseudoLegal, &mut moves);

            for (m_idx, m) in moves.iter().copied().enumerate() {
                if pos.legal(m) {
                    let pre_material = pos.material_score();
                    let pre_pst = pos.piece_square_table_score();

                    // Do the move
                    pos.do_move(m);

                    // Compute scores from scratch
                    let (expected_material, expected_pst) = pos.compute_evaluation_scores();

                    // Assert incremental score matches full board scan
                    assert_eq!(
                        pos.material_score(),
                        expected_material,
                        "Material mismatch for FEN {} move {}: {}",
                        idx,
                        m_idx,
                        m
                    );
                    assert_eq!(
                        pos.piece_square_table_score(),
                        expected_pst,
                        "PST mismatch for FEN {} move {}: {}",
                        idx,
                        m_idx,
                        m
                    );

                    // Undo the move
                    pos.undo_move();

                    // Assert scores rolled back perfectly
                    assert_eq!(
                        pos.material_score(),
                        pre_material,
                        "Material rollback mismatch for FEN {} move {}: {}",
                        idx,
                        m_idx,
                        m
                    );
                    assert_eq!(
                        pos.piece_square_table_score(),
                        pre_pst,
                        "PST rollback mismatch for FEN {} move {}: {}",
                        idx,
                        m_idx,
                        m
                    );
                }
            }
        }
    }
}
