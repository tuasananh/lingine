use std::ops::Index;

use strum::{EnumCount, EnumIter, FromRepr};

use crate::core::Side;

/// Represents the types of pieces in Xiangqi.
/// Includes virtual/helper pieces (`KnightTo` and `PawnTo`) used in precomputed attacker logic.
#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter, Clone, Copy, PartialEq, Eq, Debug)]
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

/// Represents standard Xiangqi pieces, categorized by color and piece type.
/// Red pieces are represented by values 1-7, and Black pieces by values 9-15.
#[rustfmt::skip]
#[derive(FromRepr, EnumIter, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Piece {
    RedRook = 0, RedAdvisor, RedCannon, RedPawn, RedKnight, RedBishop, RedKing,
    BlackRook = 8, BlackAdvisor, BlackCannon, BlackPawn, BlackKnight, BlackBishop, BlackKing,
}

impl Piece {
    pub const COUNT: usize = Self::BlackKing as usize + 1;
    // piece_red + SEPARATOR == piece_black
    pub const SEPARATOR: u8 = 8;
}

impl<T> Index<Piece> for [T; Piece::COUNT] {
    type Output = T;

    fn index(&self, index: Piece) -> &Self::Output {
        unsafe { self.get_unchecked(index as usize) }
    }
}

impl Piece {
    /// Extracts the `Color` of the piece, or returns `None` if it is `None`.
    #[inline]
    pub const fn color(&self) -> Side {
        Side::from_repr((*self as u8) >> 3).unwrap()
    }

    /// Extracts the `PieceType` of the piece.
    #[inline]
    pub const fn piece_type(&self) -> PieceType {
        PieceType::from_repr((*self as u8) & 7).unwrap()
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
