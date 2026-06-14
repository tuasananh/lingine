use crate::core::{
    Score,
    types::{impl_from_repr, impl_index},
};

/// Represents the two players in a Xiangqi game: Red (Red) or Black.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Side {
    Red,
    Black,
}

impl_index!(Side);
impl_from_repr!(Side);

impl Side {
    pub const COUNT: usize = Self::Black as usize + 1;

    /// Returns the opposing player's color.
    #[inline]
    pub const fn opposite(&self) -> Self {
        match self {
            Side::Red => Side::Black,
            Side::Black => Side::Red,
        }
    }

    /// Returns the number representing the sign of self
    ///
    /// * 1 for Side::Red
    /// * -1 for Side::Black
    #[inline]
    pub const fn signum(&self) -> Score {
        match self {
            Side::Red => 1,
            Side::Black => -1,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::Side;

    #[test]
    fn test_side_opposite() {
        assert_eq!(Side::Red.opposite(), Side::Black);
        assert_eq!(Side::Black.opposite(), Side::Red);
    }
}
