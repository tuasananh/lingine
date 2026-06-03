//! Core domain types for the Lingine Xiangqi engine.
//!
//! This module defines the fundamental types used throughout the chess engine,
//! including representations for players (`Color`), board coordinates
//! (`Square`, `File`, `Rank`), chess pieces (`Piece`, `PieceType`), search
//! scores (`Value`), and a highly optimized 16-bit compact representation for
//! moves (`Move`).

use std::fmt::Display;

use arrayvec::ArrayVec;
use derive_more::{Add, AddAssign, Display, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use strum::{EnumCount, EnumIter, FromRepr};

use crate::search::TranspositionTableFlag;

/// Represents the hash key for Zobrist position hashing.
pub type TranspositionTableKey = u64;

/// Represents the score of a move, typically used in move ordering heuristics.
pub type MoveScore = i32;

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
pub struct Value(i32);

#[macro_export]
macro_rules! value {
    ($expr:expr) => {
        Value::from_raw($expr)
    };
}

impl Value {
    pub const ZERO: Value = Value(0);
    pub const DRAW: Value = Value(0);
    pub const MATE: Value = Value(100_000);
    pub const INFINITY: Value = Value(10_000_000);
    pub const MATE_IN_MAX_PLY: Value = Value(Self::MATE.0 - MAX_PLY as i32);
    pub const MATED_IN_MAX_PLY: Value = Value(-Self::MATE.0 + MAX_PLY as i32);

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
            Self(self.0 + ply as i32)
        } else if self.is_losing() {
            Self(self.0 - ply as i32)
        } else {
            *self
        }
    }

    /// Get a score that is ply independent, useful for
    /// [`crate::search::TranspositionTable::probe`]
    #[inline]
    pub const fn ply_dependent(self, ply: u8) -> Self {
        if self.is_winning() {
            Self(self.0 - ply as i32)
        } else if self.is_losing() {
            Self(self.0 + ply as i32)
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
        Value(Self::MATE.0 - ply as i32)
    }

    /// Value for mated in some ply
    #[inline]
    pub const fn mated_in(ply: u8) -> Self {
        Value(-Self::MATE.0 + ply as i32)
    }

    /// Gets the value from a raw [`i32`]
    #[inline]
    pub const fn from_raw(val: i32) -> Self {
        Value(val)
    }

    /// Turns into a i32
    #[inline]
    pub const fn raw(&self) -> i32 {
        self.0
    }

    /// Gets the value from the perspective of Red
    #[inline]
    pub const fn abs(self) -> Self {
        Value(self.0.abs())
    }
}

/// Represents the two players in a Xiangqi game: White (Red) or Black.
#[rustfmt::skip]
#[derive(FromRepr, EnumCount, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Color {
    White,
    Black,
}

impl Color {
    /// Returns the opposing player's color.
    #[inline]
    pub const fn opposite(&self) -> Self {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

/// Represents the 10 ranks (horizontal rows) of a Xiangqi board, from R0 to R9.
#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Rank {
    R0, R1, R2, R3, R4, R5, R6, R7, R8, R9,
}

/// Represents the 9 files (vertical columns) of a Xiangqi board, from FA to FI (corresponds to 'a' to 'i').
#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum File {
    FA, FB, FC, FD, FE, FF, FG, FH, FI,
}

/// Represents the 90 coordinate squares on the $9 \times 10$ Xiangqi board.
/// Enumerated in rank-major order from A0 (0) to I9 (89).
#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Square {
    A0, B0, C0, D0, E0, F0, G0, H0, I0,
    A1, B1, C1, D1, E1, F1, G1, H1, I1,
    A2, B2, C2, D2, E2, F2, G2, H2, I2,
    A3, B3, C3, D3, E3, F3, G3, H3, I3,
    A4, B4, C4, D4, E4, F4, G4, H4, I4,
    A5, B5, C5, D5, E5, F5, G5, H5, I5,
    A6, B6, C6, D6, E6, F6, G6, H6, I6,
    A7, B7, C7, D7, E7, F7, G7, H7, I7,
    A8, B8, C8, D8, E8, F8, G8, H8, I8,
    A9, B9, C9, D9, E9, F9, G9, H9, I9,
}

impl Square {
    /// Constructs a `Square` from its corresponding `File` and `Rank`.
    /// Maps using `rank_index * 9 + file_index` because each rank spans 9
    /// vertical files.
    #[inline]
    pub fn from_file_rank(file: File, rank: Rank) -> Self {
        let file_index = file as u8;
        let rank_index = rank as u8;
        let square_index = rank_index * 9 + file_index;
        Self::from_repr(square_index).unwrap()
    }

    /// Extracts the vertical column (`File`) of the square.
    #[inline]
    pub fn file(&self) -> File {
        File::from_repr((*self as u8) % 9).unwrap()
    }

    /// Extracts the horizontal row (`Rank`) of the square.
    #[inline]
    pub fn rank(&self) -> Rank {
        Rank::from_repr((*self as u8) / 9).unwrap()
    }
}

/// Represents the types of pieces in Xiangqi.
/// Includes virtual/helper pieces (`KnightTo` and `PawnTo`) used in precomputed attacker logic.
#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PieceType {
    None, Rook, Advisor, Cannon, Pawn, Knight, Bishop, King
}

/// Represents standard Xiangqi pieces, categorized by color and piece type.
/// White pieces are represented by values 1-7, and Black pieces by values 9-15.
#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Piece {
    None,
    WhiteRook,     WhiteAdvisor, WhiteCannon, WhitePawn, WhiteKnight, WhiteBishop, WhiteKing, 
    BlackRook, BlackAdvisor, BlackCannon, BlackPawn, BlackKnight, BlackBishop, BlackKing, 
}

impl Piece {
    /// Extracts the `Color` of the piece, or returns `None` if it is `None`.
    #[inline]
    pub const fn color(&self) -> Option<Color> {
        match *self {
            Piece::None => None,
            Piece::WhiteRook
            | Piece::WhiteAdvisor
            | Piece::WhiteCannon
            | Piece::WhitePawn
            | Piece::WhiteKnight
            | Piece::WhiteBishop
            | Piece::WhiteKing => Some(Color::White),
            _ => Some(Color::Black),
        }
    }

    /// Extracts the `PieceType` of the piece.
    #[inline]
    pub const fn piece_type(&self) -> PieceType {
        match *self {
            Piece::None => PieceType::None,
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

/// Types of moves that can be requested during move generation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveGenType {
    Captures,
    Quiets,
    Evasions,
    PseudoLegal,
    Legal,
}

/// A compact 16-bit move representation designed for performance:
///
/// * **Bits 0 - 6**: Destination square (0 to 89, fits in 7 bits since
///   $2^7=128$).
/// * **Bits 7 - 13**: Origin square (0 to 89, fits in 7 bits).
/// * **Bits 14 - 15**: Transposition Table flags:
///  * `00` = Null Entry
///  * `01` = Exact
///  * `10` = Alpha
///  * `11` = Beta
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Move(u16);

impl Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} to {:?}", self.square_from(), self.square_to())
    }
}

impl Move {
    /// Constructs a basic quiet or capture move from an origin and destination
    /// square.
    #[inline]
    pub fn new(from: Square, to: Square) -> Self {
        Self((to as u16) | ((from as u16) << 7))
    }

    /// Constructs a move with extra flags (e.g. check/promotion).
    #[inline]
    pub fn new_with_flag(from: Square, to: Square, flag: TranspositionTableFlag) -> Self {
        Self((to as u16) | ((from as u16) << 7) | ((flag as u16) << 14))
    }

    /// Extracts the starting square index by shifting past the destination
    /// bits.
    #[inline]
    pub fn square_from(&self) -> Square {
        Square::from_repr(((self.0 >> 7) & 0x7F) as u8).unwrap()
    }

    /// Extracts the target square index by masking the lower 7 bits.
    #[inline]
    pub fn square_to(&self) -> Square {
        Square::from_repr((self.0 & 0x7F) as u8).unwrap()
    }

    /// Extracts the transposition table flags from the upper 2 bits of the move
    /// encoding.
    #[inline]
    pub const fn flag(&self) -> TranspositionTableFlag {
        TranspositionTableFlag::from_repr((self.0 >> 14) as u8).unwrap()
    }

    #[inline]
    pub const fn store_flag(&mut self, flag: TranspositionTableFlag) {
        self.0 = (self.0 & 0x3FFF) | ((flag as u16) << 14);
    }

    #[inline]
    pub const fn with_flag(&self, flag: TranspositionTableFlag) -> Self {
        Self((self.0 & 0x3FFF) | ((flag as u16) << 14))
    }

    #[inline]
    pub const fn no_flag(&self) -> Self {
        Self(self.0 & 0x3FFF)
    }

    /// Represents an empty/non-existent move.
    #[inline]
    pub const fn null() -> Self {
        Self(0)
    }

    /// Checks if the move is null.
    #[inline]
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// Converts the move into its UCI string format
    pub fn to_uci_string(&self) -> String {
        if self.is_null() {
            return "null".to_string();
        }
        let from = self.square_from();
        let to = self.square_to();
        let from_file = (b'a' + from.file() as u8) as char;
        let from_rank = (b'0' + from.rank() as u8) as char;
        let to_file = (b'a' + to.file() as u8) as char;
        let to_rank = (b'0' + to.rank() as u8) as char;
        format!("{}{}{}{}", from_file, from_rank, to_file, to_rank)
    }
}

/// The maximum number of pseudo-legal moves in any given Xiangqi position
/// (typically <= 120).
const MAX_MOVES: usize = 128;
pub const MAX_PLY: usize = 128;
pub const MAX_DEPTH: usize = 64;

/// A stack-allocated array vector that holds up to `MAX_MOVES` without heap
/// allocation.
pub type MoveList = ArrayVec<Move, MAX_MOVES>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_opposite() {
        assert_eq!(Color::White.opposite(), Color::Black);
        assert_eq!(Color::Black.opposite(), Color::White);
    }

    #[test]
    fn test_square_conversions() {
        // Test from_file_rank
        let sq_a0 = Square::from_file_rank(File::FA, Rank::R0);
        assert_eq!(sq_a0, Square::A0);
        assert_eq!(sq_a0.file(), File::FA);
        assert_eq!(sq_a0.rank(), Rank::R0);

        let sq_i9 = Square::from_file_rank(File::FI, Rank::R9);
        assert_eq!(sq_i9, Square::I9);
        assert_eq!(sq_i9.file(), File::FI);
        assert_eq!(sq_i9.rank(), Rank::R9);

        let sq_e4 = Square::from_file_rank(File::FE, Rank::R4);
        assert_eq!(sq_e4, Square::E4);
        assert_eq!(sq_e4.file(), File::FE);
        assert_eq!(sq_e4.rank(), Rank::R4);
    }

    #[test]
    fn test_piece_properties() {
        assert_eq!(Piece::None.color(), None);
        assert_eq!(Piece::None.piece_type(), PieceType::None);

        assert_eq!(Piece::WhiteRook.color(), Some(Color::White));
        assert_eq!(Piece::WhiteRook.piece_type(), PieceType::Rook);

        assert_eq!(Piece::BlackKing.color(), Some(Color::Black));
        assert_eq!(Piece::BlackKing.piece_type(), PieceType::King);

        assert_eq!(Piece::WhiteAdvisor.color(), Some(Color::White));
        assert_eq!(Piece::WhiteAdvisor.piece_type(), PieceType::Advisor);

        assert_eq!(Piece::BlackCannon.color(), Some(Color::Black));
        assert_eq!(Piece::BlackCannon.piece_type(), PieceType::Cannon);

        assert_eq!(Piece::WhitePawn.color(), Some(Color::White));
        assert_eq!(Piece::WhitePawn.piece_type(), PieceType::Pawn);

        assert_eq!(Piece::BlackKnight.color(), Some(Color::Black));
        assert_eq!(Piece::BlackKnight.piece_type(), PieceType::Knight);

        assert_eq!(Piece::WhiteBishop.color(), Some(Color::White));
        assert_eq!(Piece::WhiteBishop.piece_type(), PieceType::Bishop);
    }

    #[test]
    fn test_move_encoding() {
        let m_quiet = Move::new(Square::A0, Square::I9);
        assert_eq!(m_quiet.square_from(), Square::A0);
        assert_eq!(m_quiet.square_to(), Square::I9);
        assert_eq!(m_quiet.flag(), TranspositionTableFlag::Empty);
        assert!(!m_quiet.is_null());

        let m_flags = Move::new_with_flag(Square::E4, Square::E5, TranspositionTableFlag::Beta);
        assert_eq!(m_flags.square_from(), Square::E4);
        assert_eq!(m_flags.square_to(), Square::E5);
        assert_eq!(m_flags.flag(), TranspositionTableFlag::Beta);
        assert!(!m_flags.is_null());

        let m_none = Move::null();
        assert!(m_none.is_null());

        assert_eq!(format!("{}", m_quiet), "A0 to I9");
    }

    #[test]
    fn test_value_display_as_i32() {
        let v = Value(12345);
        assert_eq!(v.to_string(), "12345");
        let v = Value(-67890);
        assert_eq!(v.to_string(), "-67890");
    }
}
