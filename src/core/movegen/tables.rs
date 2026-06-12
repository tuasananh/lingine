use strum::EnumCount;

use crate::core::{Bitboard, File, Rank, Side, Square};

// ============================================================================
// Table entry types
// ============================================================================

/// Horizontal rank attack masks for Rooks and Cannons, indexed by the square's
/// file position (0–8) and a 9-bit rank occupancy (0–511).
///
/// The 9-bit occupancy encodes which of the 9 squares on the same rank are
/// occupied (bit i = square at file i).
#[derive(Clone, Copy)]
pub struct RankEntry {
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
pub struct FileEntry {
    /// Rook slides along the file, stopping at the first blocker each way.
    pub rook_slides: [u16; 1 << Rank::COUNT],
    /// Cannon captures along the file (over exactly one screen).
    pub cannon_captures: [u16; 1 << Rank::COUNT],
    /// Cannon attack ray along the file (all squares behind the screen).
    pub cannon_attack_ray: [u16; 1 << Rank::COUNT],
}

// ============================================================================
// Palace piece helpers (King + Advisor)
// ============================================================================

/// Returns the bitmask of valid destinations from `from_idx` for a piece that
/// steps in `dirs` and must stay within the same palace half.
const fn palace_step_attacks(from_idx: usize, dirs: &[(i8, i8); 4]) -> u128 {
    let f = (from_idx % 9) as i8;
    let r = (from_idx / 9) as i8;

    let in_white = (f >= 3 && f <= 5) && (r >= 0 && r <= 2);
    let in_black = (f >= 3 && f <= 5) && (r >= 7 && r <= 9);
    if !in_white && !in_black {
        return 0;
    }

    let mut mask = 0u128;
    let mut i = 0;
    while i < 4 {
        let (df, dr) = dirs[i];
        let nf = f + df;
        let nr = r + dr;
        let in_same = if in_white {
            (nf >= 3 && nf <= 5) && (nr >= 0 && nr <= 2)
        } else {
            (nf >= 3 && nf <= 5) && (nr >= 7 && nr <= 9)
        };
        if in_same {
            mask |= 1 << (nr * 9 + nf);
        }
        i += 1;
    }
    mask
}

/// King moves: one orthogonal step within the palace.
const fn init_king_attacks() -> [Bitboard; Square::COUNT] {
    const DIRS: [(i8, i8); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
    let mut table = [Bitboard::new(); Square::COUNT];
    let mut sq = 0;
    while sq < Square::COUNT {
        let bits = palace_step_attacks(sq, &DIRS);
        table[sq] = unsafe { Bitboard::from_raw(bits) };
        sq += 1;
    }
    table
}

/// Advisor moves: one diagonal step within the palace.
const fn init_advisor_attacks() -> [Bitboard; Square::COUNT] {
    const DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    let mut table = [Bitboard::new(); Square::COUNT];
    let mut sq = 0;
    while sq < Square::COUNT {
        let bits = palace_step_attacks(sq, &DIRS);
        table[sq] = unsafe { Bitboard::from_raw(bits) };
        sq += 1;
    }
    table
}

// ============================================================================
// Pawn helpers
// ============================================================================

/// Builds the pawn attack mask for a single square.
///
/// A pawn always attacks one square in its `forward_dr` direction (+1 = up for
/// Red, -1 = down for Black). After crossing the river (`promoted == true`) it
/// also attacks the two sideways squares on its current rank.
#[inline(always)]
const fn pawn_attacks_from(f: i8, r: i8, forward_dr: i8, promoted: bool) -> u128 {
    let mut mask = 0u128;
    // Forward square (always present if on board).
    let nr = r + forward_dr;
    if nr >= 0 && nr < 10 {
        mask |= 1 << (nr * 9 + f);
    }
    // Sideways squares (only for promoted pawns that have crossed the river).
    if promoted {
        if f > 0 {
            mask |= 1 << (r * 9 + f - 1);
        }
        if f + 1 < 9 {
            mask |= 1 << (r * 9 + f + 1);
        }
    }
    mask
}

/// Forward pawn attacks: squares a pawn *at* `sq` threatens.
/// `table[0]` = Red (moves toward higher ranks), `table[1]` = Black.
const fn init_pawn_attacks() -> [[Bitboard; Square::COUNT]; 2] {
    let mut table = [[Bitboard::new(); Square::COUNT]; 2];
    let mut sq = 0;
    while sq < Square::COUNT {
        let f = (sq % 9) as i8;
        let r = (sq / 9) as i8;
        // Red: forward = +rank, promoted when r >= 5 (has crossed the river).
        let red = pawn_attacks_from(f, r, 1, r >= 5);
        table[0][sq] = unsafe { Bitboard::from_raw(red) };
        // Black: forward = -rank, promoted when r <= 4.
        let black = pawn_attacks_from(f, r, -1, r <= 4);
        table[1][sq] = unsafe { Bitboard::from_raw(black) };
        sq += 1;
    }
    table
}

/// Reverse pawn attacks: squares a pawn of the given colour could occupy to
/// attack target `sq`. Used in check / threat detection.
///
/// The logic is the mirror of [`init_pawn_attacks`]: to reach `sq` the pawn
/// must have been one step *behind* it (opposite `forward_dr`), so we flip the
/// direction and keep the same promotion zone rules.
const fn init_pawn_attacks_to() -> [[Bitboard; Square::COUNT]; 2] {
    let mut table = [[Bitboard::new(); Square::COUNT]; 2];
    let mut sq = 0;
    while sq < Square::COUNT {
        let f = (sq % 9) as i8;
        let r = (sq / 9) as i8;
        // A Red pawn at some square attacks `sq` from one rank *below* it.
        // Promotion rule is still based on `sq`'s rank (r >= 5 = past river).
        let red = pawn_attacks_from(f, r, -1, r >= 5);
        table[0][sq] = unsafe { Bitboard::from_raw(red) };
        // A Black pawn attacks `sq` from one rank *above* it.
        let black = pawn_attacks_from(f, r, 1, r <= 4);
        table[1][sq] = unsafe { Bitboard::from_raw(black) };
        sq += 1;
    }
    table
}

// ============================================================================
// Rook / Cannon 1-D sliding helpers
// ============================================================================
//
// Each helper computes an attack mask for a single piece within a single row
// or column of `len` squares. The piece is at position `pos`, and `occ` is a
// bitmask of occupied squares in that line (bit i = square i).
//
// These are called from `init_rank_table` / `init_file_table`, which iterate
// over every (pos, occ) pair to fill the lookup tables used at runtime.

/// Rook ray: slides outward from `pos` in both directions, stopping *at* the
/// first occupied square (which can be captured, so it is included in the mask).
const fn rook_ray(pos: i32, len: i32, occ: u32) -> u32 {
    let mut mask = 0u32;
    // Slide left (toward square 0).
    let mut i = pos - 1;
    while i >= 0 {
        mask |= 1 << i;
        if (occ & (1 << i)) != 0 {
            break; // blocked — include blocker, then stop
        }
        i -= 1;
    }
    // Slide right (toward square len-1).
    let mut i = pos + 1;
    while i < len {
        mask |= 1 << i;
        if (occ & (1 << i)) != 0 {
            break;
        }
        i += 1;
    }
    mask
}

/// Cannon capture ray: locates the first occupied square on each side (the
/// "screen"), then sets the bit of the *second* occupied square beyond it (the
/// capture target). Empty squares and the screen itself are NOT included.
const fn cannon_ray(pos: i32, len: i32, occ: u32) -> u32 {
    let mut mask = 0u32;

    // Left side.
    let mut i = pos - 1;
    let mut screen = false; // have we passed one occupied square?
    while i >= 0 {
        if (occ & (1 << i)) != 0 {
            if screen {
                // Second piece found — this is the capture target.
                mask |= 1 << i;
                break;
            }
            screen = true; // first piece = the screen
        }
        i -= 1;
    }

    // Right side.
    let mut i = pos + 1;
    let mut screen = false;
    while i < len {
        if (occ & (1 << i)) != 0 {
            if screen {
                mask |= 1 << i;
                break;
            }
            screen = true;
        }
        i += 1;
    }

    mask
}

/// Cannon attack ray: all squares strictly behind the screen piece, including
/// the capture target itself. This covers every square a cannon *threatens* or
/// x-rays once a screen is present.
///
/// Contrast with [`cannon_ray`] which only marks the capture target square.
const fn cannon_attack_ray(pos: i32, len: i32, occ: u32) -> u32 {
    let mut mask = 0u32;

    // Left side.
    let mut i = pos - 1;
    let mut screen = false;
    while i >= 0 {
        if screen {
            // Every square past the screen is part of the attack ray.
            mask |= 1 << i;
        }
        if (occ & (1 << i)) != 0 {
            if screen {
                break; // capture target reached — stop
            }
            screen = true; // just hit the screen piece
        }
        i -= 1;
    }

    // Right side.
    let mut i = pos + 1;
    let mut screen = false;
    while i < len {
        if screen {
            mask |= 1 << i;
        }
        if (occ & (1 << i)) != 0 {
            if screen {
                break;
            }
            screen = true;
        }
        i += 1;
    }

    mask
}

/// Populates `RANK_TABLE[f]` for every file position `f` (0–8) and every
/// 9-bit occupancy (0–511). The occupancy bit layout mirrors rank bit layout
/// in the Bitboard representation.
const fn init_rank_table() -> [RankEntry; 9] {
    let mut table = [RankEntry {
        rook_slides: [0; 1 << File::COUNT],
        cannon_captures: [0; 1 << File::COUNT],
        cannon_attack_ray: [0; 1 << File::COUNT],
    }; 9];
    let mut f = 0i32;
    while f < 9 {
        let mut occ = 0u32;
        while occ < 1 << File::COUNT {
            table[f as usize].rook_slides[occ as usize] = rook_ray(f, 9, occ) as u16;
            table[f as usize].cannon_captures[occ as usize] = cannon_ray(f, 9, occ) as u16;
            table[f as usize].cannon_attack_ray[occ as usize] = cannon_attack_ray(f, 9, occ) as u16;
            occ += 1;
        }
        f += 1;
    }
    table
}

/// Populates `FILE_TABLE[r]` for every rank position `r` (0–9) and every
/// 10-bit occupancy (0–1023).
const fn init_file_table() -> [FileEntry; 10] {
    let mut table = [FileEntry {
        rook_slides: [0; 1 << Rank::COUNT],
        cannon_captures: [0; 1 << Rank::COUNT],
        cannon_attack_ray: [0; 1 << Rank::COUNT],
    }; 10];
    let mut r = 0i32;
    while r < 10 {
        let mut occ = 0u32;
        while occ < 1 << Rank::COUNT {
            table[r as usize].rook_slides[occ as usize] = rook_ray(r, 10, occ) as u16;
            table[r as usize].cannon_captures[occ as usize] = cannon_ray(r, 10, occ) as u16;
            table[r as usize].cannon_attack_ray[occ as usize] =
                cannon_attack_ray(r, 10, occ) as u16;
            occ += 1;
        }
        r += 1;
    }
    table
}

/// Helper table: `FILE_ATTACKS_BY_MASK[f][mask]` converts a 10-bit occupancy
/// bitmask on file `f` back into a full 128-bit [`Bitboard`].
///
/// This is needed because the file tables store results in compact 10-bit form
/// (one bit per rank), but the move generator works with full-board bitboards.
/// Indexing here: bit `r` of `mask` corresponds to rank `r` on file `f`.
const fn init_file_attacks_by_mask() -> [[Bitboard; 1 << Rank::COUNT]; 9] {
    let mut table = [[Bitboard::new(); 1 << Rank::COUNT]; 9];
    let mut f = 0usize;
    while f < 9 {
        let mut mask = 0usize;
        while mask < 1 << Rank::COUNT {
            let mut bits = 0u128;
            let mut r = 0usize;
            while r < 10 {
                if (mask & (1 << r)) != 0 {
                    // Rank-bit `r` is set → include the square at (rank=r, file=f).
                    bits |= 1u128 << (r * 9 + f);
                }
                r += 1;
            }
            table[f][mask] = unsafe { Bitboard::from_raw(bits) };
            mask += 1;
        }
        f += 1;
    }
    table
}

// ============================================================================
// Magic bitboards for leapers (Knight, Bishop)
// ============================================================================
//
// "Magic bitboards" replace per-square attack generation with an O(1) lookup:
//
//   1. AND the full-board occupancy with a per-square `mask` to extract only
//      the squares that can block this piece.
//   2. Multiply by a per-square `magic` constant to hash those bits into the
//      high portion of a u128.
//   3. Shift right to produce a small index into the `attacks` table.
//
// For leapers (Knight, Bishop) the mask covers at most 4 squares (the
// intermediate "leg" or "elbow" squares that can block the move), so SIZE = 16
// (2^4) entries are enough to store all possible attack sets.

/// One entry in the magic lookup table for a leaper piece on a single square.
#[derive(Clone, Copy)]
pub struct Magic<const SIZE: usize> {
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

// ── Compile-time PRNG ────────────────────────────────────────────────────────

/// xorshift128 PRNG — used during magic search to generate random candidates.
/// Returns a new pseudo-random value and updates `state` in place.
const fn xorshift128(state: &mut u128) -> u128 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Returns a sparse (few set bits) random value by ANDing three xorshift
/// outputs together. Sparse values tend to make better magic multipliers
/// because they concentrate the relevant bits more tightly.
const fn sparse_rand(state: &mut u128) -> u128 {
    xorshift128(state) & xorshift128(state) & xorshift128(state)
}

// ── Per-piece attack generators ──────────────────────────────────────────────
//
// These functions compute the exact attack set for one piece, one square, and
// one occupancy. They are only called during the compile-time magic search;
// the results are cached in the Magic tables and never called at runtime.

/// Knight attacks from `sq` given board occupancy `occ`.
///
/// A knight moves in an "L" shape: one step along a *leg* direction, then two
/// steps perpendicular. The leg square must be empty (orthogonally blocked),
/// otherwise that arm of the L is cut off.
///
/// `LEGS` lists the four possible leg directions. For each unblocked leg, the
/// two possible landing squares in `TARGETS` are added to the attack mask.
const fn knight_attacks(sq: usize, occ: u128) -> u128 {
    let r = (sq / 9) as i32;
    let f = (sq % 9) as i32;
    // One step in each cardinal direction — the knight's "leg".
    const LEGS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    // For each leg, the two L-shaped destinations (two steps perpendicular).
    const TARGETS: [[(i32, i32); 2]; 4] = [
        [(2, -1), (2, 1)],
        [(-2, -1), (-2, 1)],
        [(-1, 2), (1, 2)],
        [(-1, -2), (1, -2)],
    ];
    let mut attacks = 0u128;
    let mut i = 0;
    while i < 4 {
        let (ldr, ldf) = LEGS[i];
        let lr = r + ldr;
        let lf = f + ldf;
        // The leg square must be on the board and unoccupied.
        if lr >= 0 && lr < 10 && lf >= 0 && lf < 9 && (occ & (1 << (lr * 9 + lf))) == 0 {
            let mut t = 0;
            while t < 2 {
                let (tdr, tdf) = TARGETS[i][t];
                let tr = r + tdr;
                let tf = f + tdf;
                if tr >= 0 && tr < 10 && tf >= 0 && tf < 9 {
                    attacks |= 1 << (tr * 9 + tf);
                }
                t += 1;
            }
        }
        i += 1;
    }
    attacks
}

/// Bishop attacks from `sq` given board occupancy `occ`.
///
/// A bishop moves exactly two squares diagonally. The intermediate "elbow"
/// square must be empty. Bishops are also confined to their own half of the
/// board (same side of the river as the source square).
const fn bishop_attacks(sq: usize, occ: u128) -> u128 {
    let r = (sq / 9) as i32;
    let f = (sq % 9) as i32;
    const DIRS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    let mut attacks = 0u128;
    let mut i = 0;
    while i < 4 {
        let (dr, df) = DIRS[i];
        let er = r + dr; // elbow rank (one step diagonal)
        let ef = f + df; // elbow file
        let tr = r + dr * 2; // target rank (two steps diagonal)
        let tf = f + df * 2; // target file
        let on_board =
            tr >= 0 && tr < 10 && tf >= 0 && tf < 9 && er >= 0 && er < 10 && ef >= 0 && ef < 9;
        // Bishops cannot cross the river: source and target must be in the same half.
        let same_half = (r < 5) == (tr < 5);
        // Elbow square must be unoccupied (not blocked).
        if on_board && same_half && (occ & (1 << (er * 9 + ef))) == 0 {
            attacks |= 1 << (tr * 9 + tf);
        }
        i += 1;
    }
    attacks
}

/// Squares from which a knight could attack target `sq`, given `occ`.
///
/// This is the *reverse* knight attack: "where could a knight be standing to
/// threaten `sq`?" It is used in check detection (does an enemy knight attack
/// our king?).
///
/// For each of the 8 possible knight origins `(sq + ORIGINS[i])`:
///   - The leg toward `sq` is the one-step portion of the L-shape. Its
///     direction is determined by whether `odr`/`odf` is large (|2|) or
///     small (|1|).
///   - If that leg square is empty, the origin is a valid attacker.
const fn knight_to_attacks(sq: usize, occ: u128) -> u128 {
    let r = (sq / 9) as i32;
    let f = (sq % 9) as i32;
    // All eight squares that could be a knight origin relative to `sq`.
    const ORIGINS: [(i32, i32); 8] = [
        (2, 1),
        (2, -1),
        (-2, 1),
        (-2, -1),
        (1, 2),
        (1, -2),
        (-1, 2),
        (-1, -2),
    ];
    let mut attacks = 0u128;
    let mut i = 0;
    while i < 8 {
        let (odr, odf) = ORIGINS[i];
        let or = r + odr; // candidate origin rank
        let of = f + odf; // candidate origin file
        if or >= 0 && or < 10 && of >= 0 && of < 9 {
            // The leg goes from `or/of` toward `sq`. Whichever axis has the
            // larger offset (|2|) is the two-step axis; the leg steps one
            // square along that axis toward `sq`.
            let leg_r = r + if odr > 0 { 1 } else { -1 };
            let leg_f = f + if odf > 0 { 1 } else { -1 };
            // Origin is only valid if its leg square is clear.
            if (occ & (1 << (leg_r * 9 + leg_f))) == 0 {
                attacks |= 1 << (or * 9 + of);
            }
        }
        i += 1;
    }
    attacks
}

// ── Magic builder ────────────────────────────────────────────────────────────

/// Selects which attack function `build_magics` delegates to.
enum LeaperType {
    Knight,
    Bishop,
    /// Reverse-knight (squares that attack a given square). Shares Bishop's
    /// blocking-square directions because the elbow square is diagonal.
    KnightTo,
}

/// Builds the magic lookup table for every square for a given leaper piece.
///
/// # Type parameters
/// - `SIZE`: number of entries in each per-square attack table (must be a
///   power of two). E.g. `16` for pieces with up to 4 blocking squares.
/// - `SHIFT`: `log2(SIZE)`, i.e. the number of relevant blocking squares.
///   Compile-time assertion enforces `SIZE == 2^SHIFT`.
///
/// # Algorithm
///
/// For each square:
/// 1. **Build the occupancy mask**: OR together all `SHIFT` blocking-square
///    positions (computed from `dirs_dr`/`dirs_df`).
/// 2. **Enumerate all `SIZE` subsets** of the mask and compute the reference
///    attack set for each via the appropriate `*_attacks()` function.
/// 3. **Find a magic multiplier** via trial-and-error with a sparse PRNG:
///    try random candidates until one maps every subset to a unique table slot
///    (or to a slot already holding the same attack set — constructive
///    collision). Store the magic and the filled attack table.
const fn build_magics<const SIZE: usize, const SHIFT: usize>(
    piece: LeaperType,
    dirs_dr: [i32; SHIFT],
    dirs_df: [i32; SHIFT],
) -> [Magic<SIZE>; Square::COUNT] {
    const { assert!(SIZE.trailing_zeros() == SHIFT as u32) }

    let mut magics = [Magic::<SIZE> {
        mask: 0,
        magic: 0,
        attacks: [Bitboard::new(); SIZE],
    }; Square::COUNT];

    let mut sq = 0;
    while sq < Square::COUNT {
        let r = (sq / 9) as i32;
        let f = (sq % 9) as i32;

        // ── Step 1: build the occupancy mask ──────────────────────────────
        // The mask covers all `SHIFT` candidate blocking squares.  Only squares
        // that are on the board are included.
        let mut mask = 0u128;
        let mut i = 0;
        while i < SHIFT {
            let er = r + dirs_dr[i];
            let ef = f + dirs_df[i];
            if er >= 0 && er < 10 && ef >= 0 && ef < 9 {
                mask |= 1 << (er * 9 + ef);
            }
            i += 1;
        }

        // ── Step 2: enumerate all occupancy subsets of `mask` ─────────────
        // We use the bit positions in `bits[]` to convert a subset index `j`
        // (0 .. SIZE) into an actual 128-bit occupancy value.
        let mut bits = [0usize; SHIFT];
        let mut bit_count = 0;
        let mut i = 0;
        while i < 128 && bit_count < SHIFT {
            if (mask & (1 << i)) != 0 {
                bits[bit_count] = i;
                bit_count += 1;
            }
            i += 1;
        }

        let mut occupancies = [0u128; SIZE];
        let mut ref_attacks = [0u128; SIZE];
        let mut j = 0;
        while j < SIZE {
            // Build the occupancy for subset `j` by spreading its bits.
            let mut occ = 0u128;
            let mut k = 0;
            while k < SHIFT {
                if (j & (1 << k)) != 0 {
                    occ |= 1 << bits[k];
                }
                k += 1;
            }
            occupancies[j] = occ;
            ref_attacks[j] = match piece {
                LeaperType::Knight => knight_attacks(sq, occ),
                LeaperType::Bishop => bishop_attacks(sq, occ),
                LeaperType::KnightTo => knight_to_attacks(sq, occ),
            };
            j += 1;
        }

        // ── Step 3: find a collision-free magic multiplier ─────────────────
        // Trial-and-error: generate sparse random candidates and verify that
        // every (occupancy × magic >> shift) maps to a unique slot, or to a
        // slot already containing the identical attack set.
        let shift = 128 - SHIFT;
        let mut rng = 0x9876543210ABCDEF_1234567890ABCDEF_u128 + sq as u128;
        let magic;
        let mut final_attacks = [Bitboard::new(); SIZE];

        loop {
            let candidate = sparse_rand(&mut rng);
            let mut used = [false; SIZE];
            let mut attacks = [0u128; SIZE];
            let mut fail = false;

            let mut k = 0;
            while k < SIZE {
                let idx = (occupancies[k].wrapping_mul(candidate) >> shift) as usize;
                if used[idx] {
                    // Slot already filled — constructive collision only if
                    // both subsets produce the exact same attack set.
                    if attacks[idx] != ref_attacks[k] {
                        fail = true;
                        break;
                    }
                } else {
                    used[idx] = true;
                    attacks[idx] = ref_attacks[k];
                }
                k += 1;
            }

            if !fail {
                magic = candidate;
                let mut idx = 0;
                while idx < SIZE {
                    final_attacks[idx] = unsafe { Bitboard::from_raw(attacks[idx]) };
                    idx += 1;
                }
                break;
            }
        }

        magics[sq] = Magic {
            mask,
            magic,
            attacks: final_attacks,
        };
        sq += 1;
    }
    magics
}

// ============================================================================
// Static lookup tables (all computed at compile time)
// ============================================================================

pub static KING_ATTACKS: [Bitboard; Square::COUNT] = init_king_attacks();
pub static ADVISOR_ATTACKS: [Bitboard; Square::COUNT] = init_advisor_attacks();
pub static PAWN_ATTACKS: [[Bitboard; Square::COUNT]; Side::COUNT] = init_pawn_attacks();
pub static PAWN_ATTACKS_TO: [[Bitboard; Square::COUNT]; 2] = init_pawn_attacks_to();
pub static RANK_TABLE: [RankEntry; File::COUNT] = init_rank_table();
pub static FILE_TABLE: [FileEntry; Rank::COUNT] = init_file_table();
/// Converts a 10-bit file-occupancy mask back to a full [`Bitboard`].
/// Indexed as `FILE_ATTACKS_BY_MASK[file][10-bit mask]`.
pub static FILE_ATTACKS_BY_MASK: [[Bitboard; 1 << Rank::COUNT]; 9] = init_file_attacks_by_mask();

// Knight blocking dirs: the four cardinal leg-squares (rank ±1, file ±1 from origin).
const KNIGHT_DIRS: ([i32; 4], [i32; 4]) = ([1, -1, 0, 0], [0, 0, 1, -1]);
// Bishop blocking dirs: the four diagonal elbow-squares (±1 on both axes).
const BISHOP_DIRS: ([i32; 4], [i32; 4]) = ([1, 1, -1, -1], [1, -1, 1, -1]);

pub static KNIGHT_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::Knight, KNIGHT_DIRS.0, KNIGHT_DIRS.1);
pub static BISHOP_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::Bishop, BISHOP_DIRS.0, BISHOP_DIRS.1);
/// Backward knight attacks: shares Bishop's elbow-square directions because
/// the leg square for a reversed knight move is always diagonal from the target.
pub static KNIGHT_TO_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::KnightTo, BISHOP_DIRS.0, BISHOP_DIRS.1);

// ============================================================================
// Between / ray-pass bitboards
// ============================================================================

/// `BETWEEN_BB[s1][s2]` is the set of squares strictly between `s1` and `s2`
/// on the same rank or file, *plus* `s2` itself.
///
/// For a knight move (|dr|=2,|df|=1 or |dr|=1,|df|=2) the table holds `s2`
/// plus the knight's leg square — used to detect whether the knight's path is
/// obstructed.
///
/// If `s1` and `s2` are not on the same rank, file, or a knight jump apart,
/// only `s2` is stored (the `1 << s2` initialiser).
const fn init_between_bb() -> [[Bitboard; Square::COUNT]; Square::COUNT] {
    let mut table = [[Bitboard::new(); Square::COUNT]; Square::COUNT];
    let mut s1 = 0;
    while s1 < Square::COUNT {
        let mut s2 = 0;
        while s2 < Square::COUNT {
            // Always include s2 (the destination / attacker square).
            let mut bits = 1u128 << s2;

            let f1 = s1 % 9;
            let r1 = s1 / 9;
            let f2 = s2 % 9;
            let r2 = s2 / 9;

            if f1 == f2 {
                // Same file: include all ranks strictly between r1 and r2.
                let min_r = if r1 < r2 { r1 } else { r2 };
                let max_r = if r1 > r2 { r1 } else { r2 };
                let mut r = min_r + 1;
                while r < max_r {
                    bits |= 1 << (r * 9 + f1);
                    r += 1;
                }
            } else if r1 == r2 {
                // Same rank: include all files strictly between f1 and f2.
                let min_f = if f1 < f2 { f1 } else { f2 };
                let max_f = if f1 > f2 { f1 } else { f2 };
                let mut f = min_f + 1;
                while f < max_f {
                    bits |= 1 << (r1 * 9 + f);
                    f += 1;
                }
            } else {
                // Check if this is a valid knight move (|dr|,|df| ∈ {(2,1),(1,2)}).
                let dr = (r2 as i8 - r1 as i8).abs();
                let df = (f2 as i8 - f1 as i8).abs();
                if (dr == 2 && df == 1) || (dr == 1 && df == 2) {
                    // The leg square is the one step along the longer axis.
                    // For dr==2: leg is at the average rank, same file as s2.
                    // For df==2: leg is at the average file, same rank as s2.
                    let leg_r = if dr == 2 {
                        (r1 as i8 + r2 as i8) / 2
                    } else {
                        r2 as i8
                    };
                    let leg_f = if df == 2 {
                        (f1 as i8 + f2 as i8) / 2
                    } else {
                        f2 as i8
                    };
                    bits |= 1 << (leg_r * 9 + leg_f);
                }
            }

            table[s1][s2] = unsafe { Bitboard::from_raw(bits) };
            s2 += 1;
        }
        s1 += 1;
    }
    table
}

/// `RAY_PASS_BB[s1][s2]` is the set of squares from `s2` extending *away*
/// from `s1` to the edge of the board, along the same rank or file.
///
/// Only defined when `s1` and `s2` share a rank or file (otherwise empty).
/// `s2` itself is always included; `s1` is never included.
///
/// Typical use: after a sliding piece at `s1` is blocked by a piece at `s2`,
/// `RAY_PASS_BB[s1][s2]` tells you which squares are still "behind" `s2`
/// from `s1`'s perspective — useful for x-ray / pin detection.
const fn init_ray_pass_bb() -> [[Bitboard; Square::COUNT]; Square::COUNT] {
    let mut table = [[Bitboard::new(); Square::COUNT]; Square::COUNT];
    let mut s1 = 0;
    while s1 < Square::COUNT {
        let mut s2 = 0;
        while s2 < Square::COUNT {
            let f1 = s1 % 9;
            let r1 = s1 / 9;
            let f2 = s2 % 9;
            let r2 = s2 / 9;
            let mut bits = 0u128;

            if f1 == f2 {
                // Same file: extend from s2 away from s1 to the board edge.
                if r2 > r1 {
                    // s2 is above s1 → ray goes further up (increasing rank).
                    let mut r = r2;
                    while r < 10 {
                        bits |= 1 << (r * 9 + f1);
                        r += 1;
                    }
                } else if r2 < r1 {
                    // s2 is below s1 → ray goes further down (decreasing rank).
                    let mut r = r2;
                    loop {
                        bits |= 1 << (r * 9 + f1);
                        if r == 0 {
                            break;
                        }
                        r -= 1;
                    }
                }
            } else if r1 == r2 {
                // Same rank: extend from s2 away from s1 to the board edge.
                if f2 > f1 {
                    // s2 is to the right of s1 → ray extends right.
                    let mut f = f2;
                    while f < 9 {
                        bits |= 1 << (r1 * 9 + f);
                        f += 1;
                    }
                } else if f2 < f1 {
                    // s2 is to the left of s1 → ray extends left.
                    let mut f = f2;
                    loop {
                        bits |= 1 << (r1 * 9 + f);
                        if f == 0 {
                            break;
                        }
                        f -= 1;
                    }
                }
            }

            if bits != 0 {
                table[s1][s2] = unsafe { Bitboard::from_raw(bits) };
            }
            s2 += 1;
        }
        s1 += 1;
    }
    table
}

pub static BETWEEN_BB: [[Bitboard; Square::COUNT]; Square::COUNT] = init_between_bb();
pub static RAY_PASS_BB: [[Bitboard; Square::COUNT]; Square::COUNT] = init_ray_pass_bb();
