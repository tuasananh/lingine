use std::ops::Index;

use crate::{core::Side, impl_from_repr};

/// Represents the types of pieces in Xiangqi.
/// Includes virtual/helper pieces (`KnightTo` and `PawnTo`) used in precomputed attacker logic.
#[rustfmt::skip]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PieceType {
    Rook = 0, Advisor, Cannon, Pawn, Knight, Bishop, King
}

impl<T> Index<PieceType> for [T; PieceType::COUNT] {
    type Output = T;

    fn index(&self, index: PieceType) -> &Self::Output {
        unsafe { self.get_unchecked(index as usize) }
    }
}

impl_from_repr!(PieceType);

impl PieceType {
    pub const COUNT: usize = Self::King as usize + 1;

    #[inline]
    pub const fn to_piece(&self, side: Side) -> Piece {
        match side {
            Side::Red => unsafe { std::mem::transmute::<u8, Piece>(*self as u8) },
            Side::Black => unsafe {
                std::mem::transmute::<u8, Piece>(*self as u8 + Piece::BlackRook as u8)
            },
        }
    }

    pub fn all() -> impl DoubleEndedIterator<Item = Self> {
        (Self::Rook as u8..=Self::King as u8).map(|x| unsafe { std::mem::transmute(x) })
    }
}

/// Represents standard Xiangqi pieces, categorized by color and piece type.
/// Red pieces are represented by values 1-7, and Black pieces by values 9-15.
#[rustfmt::skip]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Piece {
    RedRook = 0, RedAdvisor, RedCannon, RedPawn, RedKnight, RedBishop, RedKing,
    BlackRook = 8, BlackAdvisor, BlackCannon, BlackPawn, BlackKnight, BlackBishop, BlackKing,
}

impl<T> Index<Piece> for [T; Piece::COUNT] {
    type Output = T;

    fn index(&self, index: Piece) -> &Self::Output {
        unsafe { self.get_unchecked(index as usize) }
    }
}

impl Piece {
    pub const COUNT: usize = Self::BlackKing as usize + 1;

    /// # Safety
    ///
    /// This will only be valid if [`repr`] < [`Self::COUNT`] and [`repr`] is not the missing
    /// variant.
    #[inline]
    pub const unsafe fn from_repr_unchecked(repr: u8) -> Self {
        unsafe { std::mem::transmute(repr) }
    }

    const MISSING: u8 = Self::RedKing as u8 + 1;
    #[inline]
    pub const fn from_repr(repr: u8) -> Option<Self> {
        if repr < Self::COUNT as u8 && repr != Self::MISSING {
            Some(unsafe { std::mem::transmute::<u8, Self>(repr) })
        } else {
            None
        }
    }

    pub fn all() -> impl DoubleEndedIterator<Item = Self> {
        (Self::RedRook as u8..=Self::BlackKing as u8).filter_map(Self::from_repr)
    }

    /// Extracts the `Color` of the piece, or returns `None` if it is `None`.
    #[inline]
    pub const fn color(&self) -> Side {
        unsafe { std::mem::transmute((*self as u8) >> 3) }
    }

    /// Extracts the `PieceType` of the piece.
    #[inline]
    pub const fn piece_type(&self) -> PieceType {
        unsafe { std::mem::transmute((*self as u8) & 7) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_properties() {
        assert_eq!(Piece::RedRook.color(), (Side::Red));
        assert_eq!(Piece::RedRook.piece_type(), PieceType::Rook);

        assert_eq!(Piece::BlackKing.color(), (Side::Black));
        assert_eq!(Piece::BlackKing.piece_type(), PieceType::King);

        assert_eq!(Piece::RedAdvisor.color(), (Side::Red));
        assert_eq!(Piece::RedAdvisor.piece_type(), PieceType::Advisor);

        assert_eq!(Piece::BlackCannon.color(), (Side::Black));
        assert_eq!(Piece::BlackCannon.piece_type(), PieceType::Cannon);

        assert_eq!(Piece::RedPawn.color(), (Side::Red));
        assert_eq!(Piece::RedPawn.piece_type(), PieceType::Pawn);

        assert_eq!(Piece::BlackKnight.color(), (Side::Black));
        assert_eq!(Piece::BlackKnight.piece_type(), PieceType::Knight);

        assert_eq!(Piece::RedBishop.color(), (Side::Red));
        assert_eq!(Piece::RedBishop.piece_type(), PieceType::Bishop);
    }
}
