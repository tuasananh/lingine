use crate::core::{Piece, Square};

/// Holds Zobrist random numbers used for fast, incremental position hashing.
/// Positional hashes are updated via XOR operations during do_move/undo_move,
/// entirely avoiding full-board hash recalculations.
pub(super) struct ZobristTable {
    /// Random keys for every piece type on every one of the 90 squares:
    /// Categorized as `pieces[piece_index][square_index]`.
    pub pieces: [[u64; Square::COUNT]; Piece::COUNT],
    /// XOR'ed into the hash if it is Black's turn to move.
    pub side: u64,
}

pub(super) static ZOBRIST: ZobristTable = {
    const SEED: u64 = 202416124 ^ 202400076 ^ 2416167;
    let mut pieces = [[0u64; Square::COUNT]; Piece::COUNT];
    let mut piece_idx = 0;

    const fn gen_next(mut value: u64) -> u64 {
        value = value
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        value
    }

    let mut value = SEED;
    while piece_idx < Piece::COUNT {
        let mut square_idx = 0;
        while square_idx < Square::COUNT {
            value = gen_next(value);
            pieces[piece_idx][square_idx] = value;
            square_idx += 1;
        }
        piece_idx += 1;
    }
    let side = gen_next(value);
    ZobristTable { pieces, side }
};
