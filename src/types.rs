use std::ops::{Index, IndexMut};

use arrayvec::ArrayVec;
use strum::{EnumCount, EnumIter, FromRepr};

pub type Key = u64;
pub type Value = i32;

#[rustfmt::skip]
#[derive(FromRepr, EnumCount)]
#[repr(u8)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub const fn opposite(&self) -> Self {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter)]
#[repr(u8)]
pub enum Rank {
    R0, R1, R2, R3, R4, R5, R6, R7, R8, R9,
}

#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter)]
#[repr(u8)]
pub enum File {
    FA, FB, FC, FD, FE, FF, FG, FH, FI,
}

#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter)]
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
    pub fn from_file_rank(file: File, rank: Rank) -> Self {
        let file_index = file as u8;
        let rank_index = rank as u8;
        let square_index = rank_index * 9 + file_index;
        Self::from_repr(square_index).unwrap()
    }
}

#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter)]
#[repr(u8)]
pub enum PieceType {
    NoPiece, Rook, Advisor, Cannon, Pawn, Knight, Bishop, King, KnightTo, PawnTo
}

impl PieceType {
    pub const ALL_PIECE: PieceType = PieceType::NoPiece;
}

#[rustfmt::skip]
#[derive(FromRepr, EnumCount, EnumIter)]
#[repr(u8)]
pub enum Piece {
    NoPiece,
    WhiteRook,     WhiteAdvisor, WhiteCannon, WhitePawn, WhiteKnight, WhiteBishop, WhiteKing, 
    BlackRook = 9, BlackAdvisor, BlackCannon, BlackPawn, BlackKnight, BlackBishop, BlackKing, 
}

const BLOOM_FILTER_SIZE: usize = 1 << 14;

pub struct BloomFilter {
    table: [u8; BLOOM_FILTER_SIZE],
}

impl BloomFilter {
    pub fn new() -> Self {
        Self {
            table: [0; BLOOM_FILTER_SIZE],
        }
    }
}

impl Index<Key> for BloomFilter {
    type Output = u8;

    fn index(&self, key: Key) -> &Self::Output {
        let index = (key as usize) % BLOOM_FILTER_SIZE;
        &self.table[index]
    }
}

impl IndexMut<Key> for BloomFilter {
    fn index_mut(&mut self, key: Key) -> &mut Self::Output {
        let index = (key as usize) % BLOOM_FILTER_SIZE;
        &mut self.table[index]
    }
}

// A move needs 16 bits to be stored
//
// bit  0- 6: destination square (from 0 to 89)
// bit  7-13: origin square (from 0 to 89)
//
// Special cases are Move::none() and Move::null(). We can sneak these in because
// in any normal move the destination square and origin square are always different,
// but Move::none() and Move::null() have the same origin and destination square.

#[derive(Debug, Clone, Copy)]
pub struct Move(u16);

impl Move {
    pub fn square_from(&self) -> Square {
        todo!();
    }
    pub fn square_to(&self) -> Square {
        todo!();
    }
}

impl From<&str> for Move {
    fn from(value: &str) -> Self {
        let bytes = value.as_bytes();
        if bytes.len() != 4 {
            panic!("Invalid move string: {}", value);
        }
        let from_file = File::from_repr(bytes[0] - b'a').unwrap();
        let from_rank = Rank::from_repr(bytes[1] - b'0').unwrap();
        let to_file = File::from_repr(bytes[2] - b'a').unwrap();
        let to_rank = Rank::from_repr(bytes[3] - b'0').unwrap();

        let from_square = Square::from_file_rank(from_file, from_rank);
        let to_square = Square::from_file_rank(to_file, to_rank);

        // Encode the move as a u16
        let move_value = (to_square as u16) | ((from_square as u16) << 7);
        Move(move_value)
    }
}

pub const MAX_MOVES: usize = 128;
pub type MoveList = ArrayVec<Move, MAX_MOVES>;
