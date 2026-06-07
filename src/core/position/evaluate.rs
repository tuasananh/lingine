use strum::EnumCount;

use crate::{
    core::{Color, Position, Square, Value},
    eval::{piece_material_value, piece_square_table_value},
};

impl Position {
    /// Get the on-the-fly calculated material score
    #[inline]
    pub fn material_score(&self) -> Value {
        self.history
            .last()
            .map(|s| s.material_score)
            .unwrap_or(Value::ZERO)
    }

    /// Get the on-the-fly calculated Piece Square Table score
    ///
    /// Find out more: https://www.chessprogramming.org/Piece-Square_Tables
    #[inline]
    pub fn piece_square_table_score(&self) -> Value {
        self.history
            .last()
            .map(|s| s.piece_square_table_score)
            .unwrap_or(Value::ZERO)
    }

    /// Get the complete evaluation score (material + piece-square table) of the
    /// current position from White's perspective
    #[inline]
    pub fn evaluate(&self) -> Value {
        self.material_score() + self.piece_square_table_score()
    }

    /// Computes the complete material and Piece-Square Table scores from
    /// scratch.
    pub fn compute_evaluation_scores(&self) -> (Value, Value) {
        let mut material_score = Value::ZERO;
        let mut piece_square_table_score = Value::ZERO;
        for sq_idx in 0..Square::COUNT {
            let sq = Square::from_repr(sq_idx as u8).unwrap();
            if let Some(piece) = self.board[sq_idx] {
                let color = piece.color();
                let val = piece_material_value(piece, sq);
                let pst = piece_square_table_value(piece.piece_type(), color, sq);
                if color == Color::White {
                    material_score += val;
                    piece_square_table_score += pst;
                } else {
                    material_score -= val;
                    piece_square_table_score -= pst;
                }
            }
        }
        (material_score, piece_square_table_score)
    }
}
