use std::{fmt::Display, num::NonZeroU16};

use crate::core::Square;

/// Represents the score of a move, typically used in move ordering heuristics.
pub(crate) type MoveScore = i32;

/// A compact 16-bit move representation designed for performance:
///
/// * **Bits 0 - 6**: Destination square (0 to 89, fits in 7 bits since
///   $2^7=128$).
/// * **Bits 7 - 13**: Origin square (0 to 89, fits in 7 bits).
/// * **Bits 14 - 15**: Reserved.
///
/// The move is guaranteed to be non-zero, so we can use NonZeroU16 for
/// better optimization when using Option<Move>.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move(NonZeroU16);

impl Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_uci_string())
    }
}

impl Move {
    /// Constructs a basic quiet or capture move from an origin and destination
    /// square.
    #[inline]
    pub const fn new(from: Square, to: Square) -> Self {
        assert!(
            from as u16 != to as u16,
            "Move cannot have the same origin and destination square"
        );
        Self(unsafe { NonZeroU16::new_unchecked((to as u16) | ((from as u16) << 7)) })
    }

    /// Extracts the starting square index by shifting past the destination
    /// bits.
    #[inline]
    pub const fn from(&self) -> Square {
        unsafe { std::mem::transmute(((self.0.get() >> 7) & 0x7F) as u8) }
    }

    /// Extracts the target square index by masking the lower 7 bits.
    #[inline]
    pub const fn to(&self) -> Square {
        unsafe { std::mem::transmute((self.0.get() & 0x7F) as u8) }
    }

    /// Converts the move into its UCI string format
    pub fn to_uci_string(&self) -> String {
        let from = self.from();
        let to = self.to();
        let from_file = (b'a' + from.file() as u8) as char;
        let from_rank = (b'0' + from.rank() as u8) as char;
        let to_file = (b'a' + to.file() as u8) as char;
        let to_rank = (b'0' + to.rank() as u8) as char;
        format!("{}{}{}{}", from_file, from_rank, to_file, to_rank)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::Square;

    use super::Move;

    #[test]
    fn test_move_encoding() {
        let m_quiet = Move::new(Square::A0, Square::I9);
        assert_eq!(m_quiet.from(), Square::A0);
        assert_eq!(m_quiet.to(), Square::I9);
        assert_eq!(format!("{}", m_quiet), "a0i9");
    }
}
