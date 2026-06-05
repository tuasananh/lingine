//! Static position evaluation for Xiangqi.
//!
//! This module performs a static evaluation of a given board position, yielding
//! a `Value` in centipawns. The evaluation is conducted from White's
//! perspective:
//! - Positive values favor White.
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

#[cfg(test)]
mod tests {
    use crate::core::{MoveGenType, MoveList, Piece, Position, Value, generate_moves};
    use crate::value;

    #[test]
    fn test_initial_material_evaluation() {
        let mut pos = Position::new();
        // Set standard starting FEN
        pos.set("rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1")
            .unwrap();
        // Since board is symmetric, evaluation must be exactly 0
        assert_eq!(pos.evaluate(), Value::DRAW);
    }

    #[test]
    fn test_captured_rook_evaluation() {
        let mut pos = Position::new();
        // Standard starting FEN but without White's left Rook (at A0)
        pos.set("rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/1HEAKAEHR w - - 0 1")
            .unwrap();
        // White is down a Rook (-600). The Rook at A0 has a PST value of -5.
        // Therefore, without this Rook, White loses:
        // - -600 material
        // - -5 positional PST value (net: -595)
        // - -6 Rook mobility value (attacks A1, A2; 2 * 3; net: -601)
        assert_eq!(pos.evaluate(), value!(-601));
    }

    #[test]
    fn test_crossed_river_pawn_evaluation() {
        let mut pos = Position::new();
        // Position with White Pawn crossed at E5 (rank index 5), and Black Pawn crossed
        // at E4 (rank index 4) Set up White Pawn at E5 (index 49) and Black
        // Pawn at E4 (index 40) White Pawn is crossed: +70. Black Pawn is
        // crossed: -70. Balance = 0.
        pos.set("4k4/9/9/9/4p3p/4P4/9/9/9/4K4 w - - 0 1").unwrap();
        assert_eq!(pos.piece_count(Piece::WhitePawn), 1);
        assert_eq!(pos.piece_count(Piece::BlackPawn), 2); // E4, I4

        let mut pos2 = Position::new();
        // Simple case: just a White Pawn at E4 (uncrossed) vs. Black Pawn at E5
        // (uncrossed)
        pos2.set("4k4/9/9/9/4p4/4P4/9/9/9/4K4 w - - 0 1").unwrap();
        // Both are uncrossed: White Pawn rank 4 < 5 (+30). Black Pawn rank 5 > 4 (-30).
        // Balance = 0. Let's verify that symmetric position still yields 0
        assert_eq!(pos2.evaluate(), Value::DRAW);

        // Move White Pawn to E6 (crossed: material 70, PST: 25) and keep Black Pawn at
        // E5 (uncrossed: material -30, PST: -8). Net = 70 - 30 + 25 - 8 = 57.
        let mut pos3 = Position::new();
        pos3.set("4k4/9/9/4P4/4p4/9/9/9/9/4K4 w - - 0 1").unwrap();
        assert_eq!(pos3.evaluate(), value!(57));
    }

    #[test]
    fn test_incremental_evaluation_consistency() {
        let positions = vec![
            "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1".to_string(),
            "4k4/9/9/4P4/4p4/9/9/9/9/4K4 w - - 0 1".to_string(),
            "r1bakab1r/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1".to_string(),
        ];

        for fen in positions {
            let mut pos = Position::new();
            pos.set(&fen).unwrap();

            // Generate all pseudo-legal moves
            let mut moves = MoveList::new();
            generate_moves(&pos, MoveGenType::PseudoLegal, &mut moves);

            for m in moves {
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
                        "Material mismatch for move {}",
                        m
                    );
                    assert_eq!(
                        pos.piece_square_table_score(),
                        expected_pst,
                        "PST mismatch for move {}",
                        m
                    );

                    // Undo the move
                    pos.undo_move(m);

                    // Assert scores rolled back perfectly
                    assert_eq!(
                        pos.material_score(),
                        pre_material,
                        "Material rollback mismatch for move {}",
                        m
                    );
                    assert_eq!(
                        pos.piece_square_table_score(),
                        pre_pst,
                        "PST rollback mismatch for move {}",
                        m
                    );
                }
            }
        }
    }

    #[test]
    fn test_positional_evaluation() {
        let mut pos = Position::new();
        // 1. Initial symmetric position: evaluate_positional_features must be 0
        pos.set("rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1")
            .unwrap();
        assert_eq!(pos.evaluate_positional_features(), Value::ZERO);

        // 2. Symmetric custom position: evaluate_positional_features must be 0
        pos.set("4k4/9/9/9/4r3p/4R3P/9/9/9/4K4 w - - 0 1").unwrap();
        assert_eq!(pos.evaluate_positional_features(), Value::ZERO);

        // 3. Asymmetric position: verify it does not crash and yields a non-zero value
        pos.set("4k4/9/9/9/4r4/3R5/9/9/9/4K4 w - - 0 1").unwrap();
        assert_ne!(pos.evaluate_positional_features(), Value::ZERO);
    }
}
