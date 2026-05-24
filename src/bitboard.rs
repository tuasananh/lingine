use std::fmt::Display;

use derive_more::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};
use strum::EnumCount;

use crate::types::{Color, File, Rank, Square};

/// Represents the occupancy or attack targets on the 90-square Xiangqi board.
/// Packaged inside a `u128` wrapper where bits 0 to 89 correspond to the squares A0 (0) to I9 (89).
/// The upper bits (90 to 127) are unused.
#[derive(
    BitAnd,
    BitAndAssign,
    BitOr,
    BitOrAssign,
    BitXor,
    BitXorAssign,
    Not,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
)]
pub struct Bitboard(pub u128);

impl Default for Bitboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Bitboard {
    /// Creates a new, completely empty bitboard (all bits set to 0).
    #[inline(always)]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Verifies if a given square bit is active (set to 1) in the bitboard.
    #[inline(always)]
    pub fn is_occupied(&self, square: Square) -> bool {
        (self.0 & (1u128 << (square as u8))) != 0
    }

    /// Activates (sets to 1) the bit corresponding to the given square.
    #[inline(always)]
    pub fn set_bit(&mut self, square: Square) {
        self.0 |= 1u128 << (square as u8);
    }

    /// Deactivates (clears to 0) the bit corresponding to the given square.
    #[inline(always)]
    pub fn clear_bit(&mut self, square: Square) {
        self.0 &= !(1u128 << (square as u8));
    }

    /// Returns the number of active squares (bits set to 1) in the bitboard.
    #[inline(always)]
    pub fn count_ones(&self) -> u32 {
        self.0.count_ones()
    }

    /// Pops (extracts and clears) the Least Significant Bit (LSB) from the bitboard,
    /// returning the corresponding `Square`. Returns `None` if the bitboard is empty.
    /// Used for rapid, zero-overhead sequence generation and iterator-like popping of pieces.
    #[inline(always)]
    pub fn pop_lsb(&mut self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            let lsb = self.0.trailing_zeros() as u8;
            self.0 &= self.0 - 1; // Clears the lowest set bit
            Square::from_repr(lsb)
        }
    }

    /// A const-compatible bitwise OR operator for building compile-time static lookup tables.
    #[inline(always)]
    pub const fn const_or(&self, b: Self) -> Self {
        Self(self.0 | b.0)
    }

    /// A const-compatible bitwise AND operator for compile-time filtering.
    #[inline(always)]
    pub const fn const_and(&self, b: Self) -> Self {
        Self(self.0 & b.0)
    }

    /// A compile-time precomputed mask representing both Palace zones (3x3 squares at the center-bottom and center-top).
    /// Used to validate Advisor and King moves which are strictly restricted to the Palace.
    ///
    /// * **White Palace**: Squares D0, E0, F0, D1, E1, F1, D2, E2, F2 (indexes 3..5, 12..14, 21..23).
    /// * **Black Palace**: Squares D7, E7, F7, D8, E8, F8, D9, E9, F9 (indexes 66..68, 75..77, 84..86).
    ///
    /// Combined bitwise mask = `0x70381C0000000000E07038u128` (active bits corresponding exactly to these indices).
    pub const PALACE: Self = Self(0x70381C0000000000E07038u128);

    /// Constructs a bitboard with all bits active along the specified vertical column (`File`).
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

    /// Constructs a bitboard with all bits active along the specified horizontal row (`Rank`).
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

    /// Returns a precomputed bitboard covering one half of the board (5 ranks) for a given player:
    ///
    /// * **White Side**: Ranks R0 to R4 (indices 0 to 44).
    /// * **Black Side**: Ranks R5 to R9 (indices 45 to 89).
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

    /// A precomputed mask representing vertical pawn starting paths (Files FA, FC, FE, FG, FI).
    /// In Xiangqi, Pawns can only cross the river and move sideways on files A, C, E, G, I before promotion.
    pub const PAWN_FILE: Self = Self::file(File::FA)
        .const_or(Self::file(File::FC))
        .const_or(Self::file(File::FE))
        .const_or(Self::file(File::FG))
        .const_or(Self::file(File::FI));

    /// Computes the valid Pawn board zone for the given color.
    /// Prior to crossing the river (opponent's side), Pawns can only move straight forward.
    /// Once they cross, they can move forward or horizontally sideways (left/right).
    pub const fn pawn(color: Color) -> Self {
        let other_side = Self::side(color.opposite());
        let my_side = Self::PAWN_FILE.const_and(match color {
            Color::White => Self::rank(Rank::R3).const_or(Self::rank(Rank::R4)),
            Color::Black => Self::rank(Rank::R5).const_or(Self::rank(Rank::R6)),
        });

        my_side.const_or(other_side)
    }
}

impl From<Square> for Bitboard {
    #[inline(always)]
    fn from(square: Square) -> Self {
        Bitboard(1u128 << (square as u8))
    }
}

impl From<File> for Bitboard {
    #[inline(always)]
    fn from(file: File) -> Self {
        Self::file(file)
    }
}

impl From<Rank> for Bitboard {
    #[inline(always)]
    fn from(rank: Rank) -> Self {
        Self::rank(rank)
    }
}

impl Display for Bitboard {
    /// Renders a beautiful visual ASCII representation of the 90-square board,
    /// highly useful for debugging and logging board states.
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
