use crate::core::{Bitboard, File, Rank};

/// Horizontal rank attack masks for Rooks and Cannons, indexed by the square's
/// file position (0–8) and a 9-bit rank occupancy (0–511).
///
/// The 9-bit occupancy encodes which of the 9 squares on the same rank are
/// occupied (bit i = square at file i).
#[derive(Clone, Copy)]
pub(in crate::core::movegen) struct RankEntry {
    /// Rook slides: all reachable squares left/right, stopping at (and
    /// including) the first blocker on each side.
    pub rook_slides: [u16; 1 << File::COUNT],
    /// Cannon captures: skips over exactly one screen piece and marks the
    /// first occupied square beyond it (the target). Does NOT include the
    /// screen square or squares between cannon and screen.
    pub cannon_captures: [u16; 1 << File::COUNT],
    /// Cannon attack ray: all squares strictly behind the screen piece,
    /// up to and including the capture target. Used to detect whether a
    /// cannon x-rays through a given square.
    pub cannon_attack_ray: [u16; 1 << File::COUNT],
}

/// Vertical file attack masks for Rooks and Cannons, indexed by the square's
/// rank position (0–9) and a 10-bit file occupancy (0–1023).
///
/// Analogous to [`RankEntry`] but for the vertical axis. The 10-bit occupancy
/// encodes which of the 10 squares on the same file are occupied.
#[derive(Clone, Copy)]
pub(in crate::core::movegen) struct FileEntry {
    /// Rook slides along the file, stopping at the first blocker each way.
    pub rook_slides: [u16; 1 << Rank::COUNT],
    /// Cannon captures along the file (over exactly one screen).
    pub cannon_captures: [u16; 1 << Rank::COUNT],
    /// Cannon attack ray along the file (all squares behind the screen).
    pub cannon_attack_ray: [u16; 1 << Rank::COUNT],
}

/// One entry in the magic lookup table for a leaper piece on a single square.
#[derive(Clone, Copy)]
pub(in crate::core::movegen) struct Magic<const SIZE: usize> {
    /// Bitmask of squares that affect this piece's mobility from this square
    /// (blocking squares only — not the piece's own square or destinations).
    pub mask: u128,
    /// Magic multiplier chosen so that `(occ & mask) * magic >> SHIFT` maps
    /// each distinct subset of `mask` to a unique index into `attacks`.
    pub magic: u128,
    /// Precomputed attack bitboard for each possible occupancy subset.
    pub attacks: [Bitboard; SIZE],
}

impl<const SIZE: usize> Magic<SIZE> {
    /// Number of bits to shift after multiplication to obtain the table index.
    /// `SIZE` must be a power of two; `SHIFT = 128 - log2(SIZE)`.
    const SHIFT: u32 = 128 - SIZE.trailing_zeros();

    /// Look up the attack bitboard for the given board occupancy.
    #[inline]
    pub const fn attack(&self, occupied: Bitboard) -> Bitboard {
        // 1. Keep only the squares relevant to this piece's mobility.
        // 2. Multiply by the magic to scatter those bits into the high bits.
        // 3. Shift to a small index and return the precomputed attack set.
        let idx = (occupied.raw() & self.mask).wrapping_mul(self.magic) >> Self::SHIFT;
        self.attacks[idx as usize]
    }
}

/// Selects which attack function `build_magics` delegates to.
pub(super) enum LeaperType {
    Knight,
    Bishop,
    /// Reverse-knight (squares that attack a given square). Shares Bishop's
    /// blocking-square directions because the elbow square is diagonal.
    KnightTo,
}
