use std::fmt::Display;

use derive_more::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};
use strum::EnumCount;

use crate::core::types::{File, Rank, Side, Square};

/// Bitboard representation of the 9x10 Xiangqi board.
///
/// The board has 90 squares, represented by the first 90 bits (0..89) of a
/// `u128`. Bit indices are mapped in rank-major order using `rank_index * 9 +
/// file_index`. Bits 90 to 127 are unused.
///
/// High performance is achieved by utilizing bitwise operations for move
/// generation, board occupancy tracking, and attack calculations.
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
    Default,
)]
pub struct Bitboard(u128);

impl Bitboard {
    /// Creates a new, completely empty bitboard (all bits set to 0).
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Make a bitboard directly from a raw `u128` value.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the provided `u128` value only has bits set
    /// in the range 0..89, as bits 90 to 127 are unused and may lead to
    /// undefined behavior
    #[inline]
    pub const unsafe fn from_raw(bitboard: u128) -> Self {
        Self(bitboard)
    }

    /// Retrieves the raw `u128` value of the bitboard, useful for low-level
    /// operations or debugging.
    #[inline]
    pub const fn raw(&self) -> u128 {
        self.0
    }

    /// Checks if the bitboard is completely empty (no active squares).
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Verifies if a given square bit is active (set to 1) in the bitboard.
    #[inline]
    pub const fn is_occupied(&self, square: Square) -> bool {
        (self.0 & (1u128 << (square as u8))) != 0
    }

    /// Activates (sets to 1) the bit corresponding to the given square.
    #[inline]
    pub const fn set_bit(&mut self, square: Square) {
        self.0 |= 1u128 << (square as u8);
    }

    /// Deactivates (clears to 0) the bit corresponding to the given square.
    #[inline]
    pub const fn clear_bit(&mut self, square: Square) {
        self.0 &= !(1u128 << (square as u8));
    }

    /// Returns the number of active squares (bits set to 1) in the bitboard.
    #[inline]
    pub const fn count_ones(&self) -> u32 {
        self.0.count_ones()
    }

    /// Pops (extracts and clears) the Least Significant Bit (LSB) from the
    /// bitboard, returning the corresponding `Square`. Returns `None` if
    /// the bitboard is empty. Used for rapid, zero-overhead sequence
    /// generation and iterator-like popping of pieces.
    #[inline]
    pub const fn pop_lsb(&mut self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            let lsb = self.0.trailing_zeros() as u8;
            self.0 &= self.0 - 1; // Clears the lowest set bit
            Square::from_repr(lsb)
        }
    }

    /// A const-compatible bitwise OR operator for building compile-time
    /// constants lookup tables.
    #[inline]
    pub const fn const_or(&self, b: Self) -> Self {
        Self(self.0 | b.0)
    }

    /// A const-compatible bitwise AND operator for building compile-time
    /// constants
    #[inline]
    pub const fn const_and(&self, b: Self) -> Self {
        Self(self.0 & b.0)
    }

    /// Constructs a bitboard with all bits active along the specified vertical
    /// column (`File`).
    pub const fn from_file(f: File) -> Self {
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

    /// Constructs a bitboard with all bits active along the specified
    /// horizontal row (`Rank`).
    pub const fn from_rank(r: Rank) -> Self {
        Self(0x1FFu128 << (r as u8 * 9))
    }

    /// Returns a precomputed bitboard covering one half of the board (5 ranks)
    /// for a given player:
    pub const fn side(side: Side) -> Self {
        match side {
            Side::Red => Self::from_rank(Rank::R0)
                .const_or(Self::from_rank(Rank::R1))
                .const_or(Self::from_rank(Rank::R2))
                .const_or(Self::from_rank(Rank::R3))
                .const_or(Self::from_rank(Rank::R4)),
            Side::Black => Self::from_rank(Rank::R5)
                .const_or(Self::from_rank(Rank::R6))
                .const_or(Self::from_rank(Rank::R7))
                .const_or(Self::from_rank(Rank::R8))
                .const_or(Self::from_rank(Rank::R9)),
        }
    }

    /// Returns a precomputed bitboard covering the palace of the given player
    pub const fn palace(side: Side) -> Self {
        let ranks = match side {
            Side::Red => Self::from_rank(Rank::R0)
                .const_or(Self::from_rank(Rank::R1))
                .const_or(Self::from_rank(Rank::R2)),
            Side::Black => Self::from_rank(Rank::R7)
                .const_or(Self::from_rank(Rank::R8))
                .const_or(Self::from_rank(Rank::R9)),
        };

        ranks.const_and(
            Self::from_file(File::FD)
                .const_or(Self::from_file(File::FE))
                .const_or(Self::from_file(File::FF)),
        )
    }

    pub const PALACE: Self = Self::palace(Side::Red).const_or(Self::palace(Side::Black));
}

impl From<Square> for Bitboard {
    #[inline]
    fn from(square: Square) -> Self {
        Bitboard(1u128 << (square as u8))
    }
}

impl From<File> for Bitboard {
    #[inline]
    fn from(file: File) -> Self {
        Self::from_file(file)
    }
}

impl From<Rank> for Bitboard {
    #[inline]
    fn from(rank: Rank) -> Self {
        Self::from_rank(rank)
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
    use crate::core::{
        bitboard::Bitboard,
        types::{File, Rank, Side},
    };

    #[test]
    fn test_print_rank_bitboard() {
        println!("{}", Bitboard::from_rank(Rank::R0));
        println!("{}", Bitboard::from_rank(Rank::R1));
        println!("{}", Bitboard::from_rank(Rank::R2));
    }

    #[test]
    fn test_print_file_bitboard() {
        println!("{}", Bitboard::from_file(File::FA));
        println!("{}", Bitboard::from_file(File::FB));
        println!("{}", Bitboard::from_file(File::FC));
    }

    #[test]
    fn test_print_red_side_bitboard() {
        println!("{}", Bitboard::side(Side::Red));
    }

    #[test]
    fn test_print_black_side_bitboard() {
        println!("{}", Bitboard::side(Side::Black));
    }
}
