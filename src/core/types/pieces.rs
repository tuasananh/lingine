use strum::{EnumCount, EnumIter, FromRepr};

use crate::core::Side;

/// Represents the types of pieces in Xiangqi.
/// Includes virtual/helper pieces (`KnightTo` and `PawnTo`) used in precomputed attacker logic.
#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PieceType {
    Rook, Advisor, Cannon, Pawn, Knight, Bishop, King
}

/// Represents standard Xiangqi pieces, categorized by color and piece type.
/// White pieces are represented by values 1-7, and Black pieces by values 9-15.
#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Piece {
    WhiteRook, WhiteAdvisor, WhiteCannon, WhitePawn, WhiteKnight, WhiteBishop, WhiteKing,
    BlackRook, BlackAdvisor, BlackCannon, BlackPawn, BlackKnight, BlackBishop, BlackKing,
}

impl Piece {
    /// Extracts the `Color` of the piece, or returns `None` if it is `None`.
    #[inline]
    pub const fn color(&self) -> Side {
        match *self {
            Piece::WhiteRook
            | Piece::WhiteAdvisor
            | Piece::WhiteCannon
            | Piece::WhitePawn
            | Piece::WhiteKnight
            | Piece::WhiteBishop
            | Piece::WhiteKing => Side::Red,
            _ => Side::Black,
        }
    }

    /// Extracts the `PieceType` of the piece.
    #[inline]
    pub const fn piece_type(&self) -> PieceType {
        match *self {
            Piece::WhiteRook | Piece::BlackRook => PieceType::Rook,
            Piece::WhiteAdvisor | Piece::BlackAdvisor => PieceType::Advisor,
            Piece::WhiteCannon | Piece::BlackCannon => PieceType::Cannon,
            Piece::WhitePawn | Piece::BlackPawn => PieceType::Pawn,
            Piece::WhiteKnight | Piece::BlackKnight => PieceType::Knight,
            Piece::WhiteBishop | Piece::BlackBishop => PieceType::Bishop,
            Piece::WhiteKing | Piece::BlackKing => PieceType::King,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_properties() {
        assert_eq!(Piece::WhiteRook.color(), (Side::Red));
        assert_eq!(Piece::WhiteRook.piece_type(), PieceType::Rook);

        assert_eq!(Piece::BlackKing.color(), (Side::Black));
        assert_eq!(Piece::BlackKing.piece_type(), PieceType::King);

        assert_eq!(Piece::WhiteAdvisor.color(), (Side::Red));
        assert_eq!(Piece::WhiteAdvisor.piece_type(), PieceType::Advisor);

        assert_eq!(Piece::BlackCannon.color(), (Side::Black));
        assert_eq!(Piece::BlackCannon.piece_type(), PieceType::Cannon);

        assert_eq!(Piece::WhitePawn.color(), (Side::Red));
        assert_eq!(Piece::WhitePawn.piece_type(), PieceType::Pawn);

        assert_eq!(Piece::BlackKnight.color(), (Side::Black));
        assert_eq!(Piece::BlackKnight.piece_type(), PieceType::Knight);

        assert_eq!(Piece::WhiteBishop.color(), (Side::Red));
        assert_eq!(Piece::WhiteBishop.piece_type(), PieceType::Bishop);
    }
}
