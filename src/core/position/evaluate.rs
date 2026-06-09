use strum::EnumCount;

use crate::core::{Position, Score, Side, Square, score};

use crate::core::PieceType;

impl Position {
    /// Get the on-the-fly calculated tapered evaluation score
    #[inline]
    pub fn tapered_score(&self) -> Score {
        if let Some(state) = self.history.last() {
            crate::eval::get_tapered_score(state.mg_score, state.eg_score, state.phase)
        } else {
            score::ZERO
        }
    }

    /// Computes the complete tapered middlegame and endgame evaluation scores
    /// from scratch.
    pub fn compute_tapered_evaluation_scores(&self) -> (Score, Score) {
        let mut mg_score = 0;
        let mut eg_score = 0;
        for sq_idx in 0..Square::COUNT {
            let sq = Square::from_repr(sq_idx as u8).unwrap();
            if let Some(piece) = self.board[sq_idx] {
                let color = piece.color();
                let val = crate::eval::piece_material_value_tapered(piece, sq);
                let pst =
                    crate::eval::piece_square_table_value_tapered(piece.piece_type(), color, sq);
                let piece_total = val + pst;
                if color == Side::Red {
                    mg_score += piece_total.mg;
                    eg_score += piece_total.eg;
                } else {
                    mg_score -= piece_total.mg;
                    eg_score -= piece_total.eg;
                }
            }
        }
        (mg_score, eg_score)
    }

    /// Calculates the current phase from the active board pieces.
    #[inline]
    pub fn calculate_board_phase(&self) -> i32 {
        crate::eval::calculate_phase(
            self.bitboard_by_type(PieceType::Rook),
            self.bitboard_by_type(PieceType::Cannon),
            self.bitboard_by_type(PieceType::Knight),
            self.bitboard_by_type(PieceType::Advisor),
            self.bitboard_by_type(PieceType::Bishop),
        )
    }

    /// Get the on-the-fly calculated incremental middlegame score.
    #[inline]
    pub fn mg_score(&self) -> Score {
        self.history
            .last()
            .map(|s| s.mg_score)
            .unwrap_or(score::ZERO)
    }

    /// Get the on-the-fly calculated incremental endgame score.
    #[inline]
    pub fn eg_score(&self) -> Score {
        self.history
            .last()
            .map(|s| s.eg_score)
            .unwrap_or(score::ZERO)
    }

    /// Get the on-the-fly calculated incremental game phase.
    #[inline]
    pub fn phase(&self) -> i32 {
        self.history.last().map(|s| s.phase).unwrap_or(0)
    }
}
