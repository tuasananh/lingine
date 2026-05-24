use std::fmt::Display;

use derive_more::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};
use strum::EnumCount;

use crate::types::{Color, File, Rank, Square};

#[derive(BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not)]
pub struct Bitboard(u128);

impl Bitboard {
    fn new() -> Self {
        Self(0)
    }

    fn is_occupied(&self, square: Square) -> bool {
        (self.0 & (1u128 << (square as u8))) != 0
    }

    pub const fn const_or(&self, b: Self) -> Self {
        Self(self.0 | b.0)
    }

    pub const fn const_and(&self, b: Self) -> Self {
        Self(self.0 & b.0)
    }

    pub const PALACE: Self = Self(0x70381C0000000000E07038u128);

    pub const fn file(f: File) -> Self {
        let mut bits = 0u128;
        let mut r = 0u8;
        let f = f as u8;

        while r < Rank::COUNT as u8 {
            let square_index = (r * File::COUNT as u8) + f;
            bits |= 1u128 << square_index;
            r += 1;
        }

        Self(bits)
    }

    pub const fn rank(r: Rank) -> Self {
        let mut bits = 0u128;
        let mut f = 0u8;
        let r = r as u8;

        while f < File::COUNT as u8 {
            let square_index = (r * File::COUNT as u8) + f;
            bits |= 1u128 << square_index;
            f += 1;
        }

        Self(bits)
    }

    pub const fn side(color: Color) -> Self {
        match color {
            Color::White => Self::rank(Rank::R0)
                .const_or(Self::rank(Rank::R1))
                .const_or(Self::rank(Rank::R2))
                .const_or(Self::rank(Rank::R3))
                .const_or(Self::rank(Rank::R4)),
            Color::Black => Self::rank(Rank::R5)
                .const_or(Self::rank(Rank::R6))
                .const_or(Self::rank(Rank::R7))
                .const_or(Self::rank(Rank::R8))
                .const_or(Self::rank(Rank::R9)),
        }
    }

    pub const PAWN_FILE: Self = Self::file(File::FA)
        .const_or(Self::file(File::FC))
        .const_or(Self::file(File::FE))
        .const_or(Self::file(File::FG))
        .const_or(Self::file(File::FI));

    pub const fn pawn(color: Color) -> Self {
        let other_side = Self::side(color.opposite());
        let my_side = Self::PAWN_FILE.const_and(match color {
            Color::White => Self::rank(Rank::R3).const_or(Self::rank(Rank::R4)),
            Color::Black => Self::rank(Rank::R5).const_or(Self::rank(Rank::R6)),
        });

        return my_side.const_or(other_side);
    }
}

impl From<Square> for Bitboard {
    fn from(square: Square) -> Self {
        Bitboard(1u128 << (square as u8))
    }
}

impl From<File> for Bitboard {
    fn from(file: File) -> Self {
        Self::file(file)
    }
}

impl From<Rank> for Bitboard {
    fn from(rank: Rank) -> Self {
        Self::rank(rank)
    }
}

impl Display for Bitboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut result = String::from("+---+---+---+---+---+---+---+---+---+\n");

        for rank in (0..Rank::COUNT as u8).rev() {
            for file in 0..File::COUNT as u8 {
                let r = Rank::from_repr(rank).unwrap();
                let f = File::from_repr(file).unwrap();
                let square = Square::from_file_rank(f, r);

                result.push_str(if self.is_occupied(square) {
                    "| X "
                } else {
                    "|   "
                });
            }

            result.push_str("| ");
            result.push_str(&rank.to_string());
            result.push_str("\n+---+---+---+---+---+---+---+---+---+\n");
        }
        result.push_str("  a   b   c   d   e   f   g   h   i\n");

        f.write_str(&result)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        bitboard::Bitboard,
        types::{Color, File, Rank},
    };

    #[test]
    fn test_print_palace() {
        println!("{}", Bitboard::PALACE);
    }

    #[test]
    fn test_print_rank_bitboard() {
        println!("{}", Bitboard::rank(Rank::R0));
        println!("{}", Bitboard::rank(Rank::R1));
        println!("{}", Bitboard::rank(Rank::R2));
    }

    #[test]
    fn test_print_file_bitboard() {
        println!("{}", Bitboard::file(File::FA));
        println!("{}", Bitboard::file(File::FB));
        println!("{}", Bitboard::file(File::FC));
    }

    #[test]
    fn test_print_pawn_file_bitboard() {
        println!("{}", Bitboard::PAWN_FILE);
    }

    #[test]
    fn test_print_white_side_bitboard() {
        println!("{}", Bitboard::side(Color::White));
    }

    #[test]
    fn test_print_black_side_bitboard() {
        println!("{}", Bitboard::side(Color::Black));
    }

    #[test]
    fn test_print_pawn_bitboard() {
        println!("{}", Bitboard::pawn(Color::White));
        println!("{}", Bitboard::pawn(Color::Black));
    }
}
