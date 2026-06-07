use strum::{EnumCount, EnumIter, FromRepr};

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

#[cfg(test)]
mod tests {
    use crate::core::{File, Rank, Square};

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
}
