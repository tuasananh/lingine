use crate::core::{PackedScore, Position, Score};

use crate::eval::{calculate_phase, evaluate, tapered_score_from_scratch};

impl Position {
    /// Get the current evaluation score from the position, with the perspective
    /// of the side to move.
    #[inline]
    pub fn evaluate(&self) -> Score {
        self.side_to_move().signum() * evaluate(self)
    }

    /// Computes the complete tapered middlegame and endgame evaluation scores
    /// from scratch.
    pub fn tapered_score_from_scratch(&self) -> PackedScore {
        tapered_score_from_scratch(self)
    }

    /// Calculates the current phase from the active board pieces.
    #[inline]
    pub fn calculate_board_phase(&self) -> u8 {
        calculate_phase(self)
    }

    /// Get the on-the-fly calculated incremental mid- and end-game score.
    #[inline]
    pub fn score(&self) -> PackedScore {
        self.state.score
    }

    /// Get the on-the-fly calculated incremental game phase.
    #[inline]
    pub fn phase(&self) -> u8 {
        self.state.phase
    }
}
