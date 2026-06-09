use strum::{EnumCount, FromRepr};

/// Represents the two players in a Xiangqi game: Red (Red) or Black.
#[derive(FromRepr, EnumCount, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Side {
    Red,
    Black,
}

impl Side {
    /// Returns the opposing player's color.
    #[inline]
    pub const fn opposite(&self) -> Self {
        match self {
            Side::Red => Side::Black,
            Side::Black => Side::Red,
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
