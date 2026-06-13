use strum::EnumCount;

use crate::core::{PackedScore, Position, Score, Side, Square};

use crate::core::PieceType;
use crate::eval::get_tapered_score;

impl Position {
    /// Get the current evaluation score from the position, with the perspective
    /// of the side to move.
    #[inline]
    pub fn evaluate(&self) -> Score {
        let score_from_red = self.tapered_score();
        match self.side_to_move() {
            Side::Red => score_from_red,
            Side::Black => -score_from_red,
        }
    }

    /// Get the on-the-fly calculated incremental tapered evaluation score
    #[inline]
    pub fn tapered_score(&self) -> Score {
        let base_score = self.state.score;
        let mobility_score = crate::eval::compute_mobility_score(self);
        get_tapered_score(base_score + mobility_score, self.state.phase)
    }

    /// Computes the complete tapered middlegame and endgame evaluation scores
    /// from scratch.
    pub fn compute_tapered_evaluation_scores(&self) -> PackedScore {
        let mut score = PackedScore::ZERO;
        for sq_idx in 0..Square::COUNT {
            if let Some(piece) = self.board[sq_idx] {
                let sq = Square::from_repr(sq_idx as u8).unwrap();
                let color = piece.color();
                let val = crate::eval::piece_material_value_tapered(piece, sq);
                let pst =
                    crate::eval::piece_square_table_value_tapered(piece.piece_type(), color, sq);
                let piece_total = val + pst;
                score += color.signum() * piece_total;
            }
        }
        score
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

    /// Get the on-the-fly calculated incremental mid- and end-game score.
    #[inline]
    pub fn score(&self) -> PackedScore {
        self.state.score
    }

    /// Get the on-the-fly calculated incremental game phase.
    #[inline]
    pub fn phase(&self) -> i32 {
        self.state.phase
    }
}
