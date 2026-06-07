use derive_more::{Add, AddAssign, Display, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use crate::core::MAX_PLY;

/// Represents evaluation values or search scores.
#[derive(
    Display,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Neg,
    Sub,
    SubAssign,
    Add,
    AddAssign,
    Div,
    DivAssign,
    Mul,
    MulAssign,
    Default,
)]
pub struct Score(i16);

#[macro_export]
macro_rules! score {
    ($expr:expr) => {
        Score::from($expr)
    };
}

impl Score {
    pub const ZERO: Score = Score(0);
    pub const DRAW: Score = Score(0);
    pub const MATE: Score = Score(32_000);
    pub const INFINITY: Score = Score(32_001);
    pub const MATE_IN_MAX_PLY: Score = Score(Self::MATE.0 - MAX_PLY as i16);
    pub const MATED_IN_MAX_PLY: Score = Score(-Self::MATE.0 + MAX_PLY as i16);

    /// Checks whether the score is winning (mate in some plies)
    #[inline]
    pub const fn is_winning(&self) -> bool {
        self.0 >= Self::MATE_IN_MAX_PLY.0
    }

    /// Checks whether the score is losing (mated mate in some plies)
    #[inline]
    pub const fn is_losing(&self) -> bool {
        self.0 <= Self::MATED_IN_MAX_PLY.0
    }

    /// Get a score that is ply independent, useful for
    /// [`crate::search::TranspositionTable::store`]
    #[inline]
    pub const fn ply_independent(&self, ply: u8) -> Self {
        if self.is_winning() {
            Self(self.0 + ply as i16)
        } else if self.is_losing() {
            Self(self.0 - ply as i16)
        } else {
            *self
        }
    }

    /// Get a score that is ply independent, useful for
    /// [`crate::search::TranspositionTable::probe`]
    #[inline]
    pub const fn ply_dependent(self, ply: u8) -> Self {
        if self.is_winning() {
            Self(self.0 - ply as i16)
        } else if self.is_losing() {
            Self(self.0 + ply as i16)
        } else {
            self
        }
    }

    /// Gets the number of ply until we have a mate or get mated
    #[inline]
    pub const fn ply_to_mate_or_mated(&self) -> Option<u8> {
        if self.is_winning() {
            Some((Self::MATE.0 - self.0) as u8)
        } else if self.is_losing() {
            Some((self.0 + Self::MATE.0) as u8)
        } else {
            None
        }
    }

    /// Value for mate in some ply
    #[inline]
    pub const fn mate_in(ply: u8) -> Self {
        Score(Self::MATE.0 - ply as i16)
    }

    /// Value for mated in some ply
    #[inline]
    pub const fn mated_in(ply: u8) -> Self {
        Score(-Self::MATE.0 + ply as i16)
    }

    /// Gets the value from a raw [`i16`]
    #[inline]
    pub const fn from_raw(val: i16) -> Self {
        Score(val)
    }

    /// Turns into a i16
    #[inline]
    pub const fn raw(&self) -> i16 {
        self.0
    }

    /// Gets the value from the perspective of Red
    #[inline]
    pub const fn abs(self) -> Self {
        Score(self.0.abs())
    }
}

impl From<i16> for Score {
    #[inline]
    fn from(val: i16) -> Self {
        Score(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_display_as_i16() {
        let v = Score(12345);
        assert_eq!(v.to_string(), "12345");
        let v = Score(-6790);
        assert_eq!(v.to_string(), "-6790");
    }
}
