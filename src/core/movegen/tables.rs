use strum::EnumCount;

use crate::core::{Bitboard, File, Rank, Side, Square};

// ============================================================================
// Table entry types
// ============================================================================

/// Horizontal rank attack masks for Rooks and Cannons, indexed by 9-bit
/// rank occupancy (0–511).
#[derive(Clone, Copy)]
pub struct RankEntry {
    /// Slides until hitting a blocker (inclusive).
    pub rook: [u16; 512],
    /// Quiet moves skip to the first screen; captures land on the piece behind
    /// it.
    pub cannon: [u16; 512],
    /// Full cannon attack paths including quiet/screened squares.
    pub cannon_attack_ray: [u16; 512],
}

/// Vertical file attack masks for Rooks and Cannons, indexed by 10-bit
/// file occupancy (0–1023).
#[derive(Clone, Copy)]
pub struct FileEntry {
    pub rook: [u16; 1024],
    pub cannon: [u16; 1024],
    /// Full cannon attack paths including quiet/screened squares.
    pub cannon_attack_ray: [u16; 1024],
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

const fn init_king_attacks() -> [Bitboard; Square::COUNT] {
    const DIRS: [(i8, i8); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
    let mut table = [Bitboard::new(); Square::COUNT];
    let mut sq = 0;
    while sq < Square::COUNT {
        let bits = palace_step_attacks(sq, &DIRS);
        if bits != 0 {
            table[sq] = unsafe { Bitboard::from_raw(bits) };
        }
        sq += 1;
    }
    table
}

const fn init_advisor_attacks() -> [Bitboard; Square::COUNT] {
    const DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    let mut table = [Bitboard::new(); Square::COUNT];
    let mut sq = 0;
    while sq < Square::COUNT {
        let bits = palace_step_attacks(sq, &DIRS);
        if bits != 0 {
            table[sq] = unsafe { Bitboard::from_raw(bits) };
        }
        sq += 1;
    }
    table
}

// ============================================================================
// Pawn helpers
// ============================================================================

/// Builds the pawn attack mask for one square and one side.
/// `forward_dr`: +1 for Red (moves up), -1 for Black (moves down).
/// `river_start`: rank at which the pawn is considered promoted (5 for Red, ≤4
/// for Black). `promoted`: true when the pawn has crossed the river.
#[inline(always)]
const fn pawn_attacks_from(f: i8, r: i8, forward_dr: i8, promoted: bool) -> u128 {
    let mut mask = 0u128;
    let nr = r + forward_dr;
    if nr >= 0 && nr < 10 {
        mask |= 1 << (nr * 9 + f);
    }
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

const fn init_pawn_attacks() -> [[Bitboard; Square::COUNT]; 2] {
    let mut table = [[Bitboard::new(); Square::COUNT]; 2];
    let mut sq = 0;
    while sq < Square::COUNT {
        let f = (sq % 9) as i8;
        let r = (sq / 9) as i8;
        // Red: forward = +rank, promoted when r >= 5
        let red = pawn_attacks_from(f, r, 1, r >= 5);
        table[0][sq] = unsafe { Bitboard::from_raw(red) };
        // Black: forward = -rank, promoted when r <= 4
        let black = pawn_attacks_from(f, r, -1, r <= 4);
        table[1][sq] = unsafe { Bitboard::from_raw(black) };
        sq += 1;
    }
    table
}

/// Reverse pawn attacks: squares a pawn of the given color could have come
/// from to attack `sq`. Used in check detection.
const fn init_pawn_attacks_to() -> [[Bitboard; Square::COUNT]; 2] {
    let mut table = [[Bitboard::new(); Square::COUNT]; 2];
    let mut sq = 0;
    while sq < Square::COUNT {
        let f = (sq % 9) as i8;
        let r = (sq / 9) as i8;
        // A Red pawn attacks `sq` from one rank below (or sideways if promoted).
        let red = pawn_attacks_from(f, r, -1, r >= 5);
        table[0][sq] = unsafe { Bitboard::from_raw(red) };
        // A Black pawn attacks `sq` from one rank above (or sideways if promoted).
        let black = pawn_attacks_from(f, r, 1, r <= 4);
        table[1][sq] = unsafe { Bitboard::from_raw(black) };
        sq += 1;
    }
    table
}

// ============================================================================
// Rook / Cannon sliding tables
// ============================================================================

/// Builds a 1-D rook sliding mask for position `pos` in a line of `len` squares
/// given a `len`-bit occupancy. Returns a bitmask over the line.
const fn rook_ray(pos: i32, len: i32, occ: u32) -> u32 {
    let mut mask = 0u32;
    let mut i = pos - 1;
    while i >= 0 {
        mask |= 1 << i;
        if (occ & (1 << i)) != 0 {
            break;
        }
        i -= 1;
    }
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

/// Builds a 1-D cannon capture mask: jumps over exactly one screen and
/// targets the next occupied square.
const fn cannon_ray(pos: i32, len: i32, occ: u32) -> u32 {
    let mut mask = 0u32;

    let mut i = pos - 1;
    let mut screen = false;
    while i >= 0 {
        if (occ & (1 << i)) != 0 {
            if screen {
                mask |= 1 << i;
                break;
            }
            screen = true;
        }
        i -= 1;
    }

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

/// Builds a 1-D cannon attack mask: all squares behind exactly one screen,
/// up to and including the second piece.
const fn cannon_attack_ray(pos: i32, len: i32, occ: u32) -> u32 {
    let mut mask = 0u32;

    let mut i = pos - 1;
    let mut screen = false;
    while i >= 0 {
        if screen {
            mask |= 1 << i;
        }
        if (occ & (1 << i)) != 0 {
            if screen {
                break;
            }
            screen = true;
        }
        i -= 1;
    }

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

const fn init_rank_table() -> [RankEntry; 9] {
    let mut table = [RankEntry {
        rook: [0; 512],
        cannon: [0; 512],
        cannon_attack_ray: [0; 512],
    }; 9];
    let mut f = 0i32;
    while f < 9 {
        let mut occ = 0u32;
        while occ < 512 {
            table[f as usize].rook[occ as usize] = rook_ray(f, 9, occ) as u16;
            table[f as usize].cannon[occ as usize] = cannon_ray(f, 9, occ) as u16;
            table[f as usize].cannon_attack_ray[occ as usize] = cannon_attack_ray(f, 9, occ) as u16;
            occ += 1;
        }
        f += 1;
    }
    table
}

const fn init_file_table() -> [FileEntry; 10] {
    let mut table = [FileEntry {
        rook: [0; 1024],
        cannon: [0; 1024],
        cannon_attack_ray: [0; 1024],
    }; 10];
    let mut r = 0i32;
    while r < 10 {
        let mut occ = 0u32;
        while occ < 1024 {
            table[r as usize].rook[occ as usize] = rook_ray(r, 10, occ) as u16;
            table[r as usize].cannon[occ as usize] = cannon_ray(r, 10, occ) as u16;
            table[r as usize].cannon_attack_ray[occ as usize] =
                cannon_attack_ray(r, 10, occ) as u16;
            occ += 1;
        }
        r += 1;
    }
    table
}

const fn init_file_attacks_by_mask() -> [[Bitboard; 1024]; 9] {
    let mut table = [[Bitboard::new(); 1024]; 9];
    let mut f = 0usize;
    while f < 9 {
        let mut mask = 0usize;
        while mask < 1024 {
            let mut bits = 0u128;
            let mut r = 0usize;
            while r < 10 {
                if (mask & (1 << r)) != 0 {
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

#[derive(Clone, Copy)]
pub struct Magic<const SIZE: usize> {
    pub mask: u128,
    pub magic: u128,
    pub attacks: [Bitboard; SIZE],
}

impl<const SIZE: usize> Magic<SIZE> {
    const SHIFT: u32 = 128 - SIZE.trailing_zeros();

    #[inline]
    pub const fn attack(&self, occupied: Bitboard) -> Bitboard {
        let idx = (occupied.raw() & self.mask).wrapping_mul(self.magic) >> Self::SHIFT;
        self.attacks[idx as usize]
    }
}

// ── PRNG ────────────────────────────────────────────────────────────────────

const fn xorshift128(state: &mut u128) -> u128 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

const fn sparse_rand(state: &mut u128) -> u128 {
    xorshift128(state) & xorshift128(state) & xorshift128(state)
}

// ── Per-piece attack generators ──────────────────────────────────────────────

const fn knight_attacks(sq: usize, occ: u128) -> u128 {
    let r = (sq / 9) as i32;
    let f = (sq % 9) as i32;
    const LEGS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
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

const fn bishop_attacks(sq: usize, occ: u128) -> u128 {
    let r = (sq / 9) as i32;
    let f = (sq % 9) as i32;
    const DIRS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    let mut attacks = 0u128;
    let mut i = 0;
    while i < 4 {
        let (dr, df) = DIRS[i];
        let er = r + dr;
        let ef = f + df;
        let tr = r + dr * 2;
        let tf = f + df * 2;
        let on_board =
            tr >= 0 && tr < 10 && tf >= 0 && tf < 9 && er >= 0 && er < 10 && ef >= 0 && ef < 9;
        let same_half = (r < 5) == (tr < 5);
        if on_board && same_half && (occ & (1 << (er * 9 + ef))) == 0 {
            attacks |= 1 << (tr * 9 + tf);
        }
        i += 1;
    }
    attacks
}

/// Squares a knight could occupy to attack `sq` given `occ` (used in check
/// detection).
const fn knight_to_attacks(sq: usize, occ: u128) -> u128 {
    let r = (sq / 9) as i32;
    let f = (sq % 9) as i32;
    // Each entry: (origin offset dr/df, leg direction dr/df toward destination)
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
        let or = r + odr;
        let of = f + odf;
        if or >= 0 && or < 10 && of >= 0 && of < 9 {
            let leg_r = r + if odr > 0 { 1 } else { -1 };
            let leg_f = f + if odf > 0 { 1 } else { -1 };
            if (occ & (1 << (leg_r * 9 + leg_f))) == 0 {
                attacks |= 1 << (or * 9 + of);
            }
        }
        i += 1;
    }
    attacks
}

// ── Magic builder ────────────────────────────────────────────────────────────

/// Leaper type selector for `build_magics`. A plain `u8` constant is used
/// instead of an enum so the value is usable inside `const fn` match arms
/// without stable `const` trait support.
enum LeaperType {
    Knight,
    Bishop,
    KnightTo,
}

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

        // Build the occupancy mask from neighbour squares in `dirs`.
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

        // Collect the bit positions of the mask so we can enumerate subsets.
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

        // Enumerate all occupancy subsets and their reference attacks.
        let mut occupancies = [0u128; SIZE];
        let mut ref_attacks = [0u128; SIZE];
        let mut j = 0;
        while j < SIZE {
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

        // Find a collision-free magic multiplier.
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
pub static FILE_ATTACKS_BY_MASK: [[Bitboard; 1024]; 9] = init_file_attacks_by_mask();

const KNIGHT_DIRS: ([i32; 4], [i32; 4]) = ([1, -1, 0, 0], [0, 0, 1, -1]);
const BISHOP_DIRS: ([i32; 4], [i32; 4]) = ([1, 1, -1, -1], [1, -1, 1, -1]);

pub static KNIGHT_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::Knight, KNIGHT_DIRS.0, KNIGHT_DIRS.1);
pub static BISHOP_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::Bishop, BISHOP_DIRS.0, BISHOP_DIRS.1);
/// Backward knight attacks share the bishop's blocking-square offsets.
pub static KNIGHT_TO_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::KnightTo, BISHOP_DIRS.0, BISHOP_DIRS.1);

const fn init_between_bb() -> [[Bitboard; Square::COUNT]; Square::COUNT] {
    let mut table = [[Bitboard::new(); Square::COUNT]; Square::COUNT];
    let mut s1 = 0;
    while s1 < Square::COUNT {
        let mut s2 = 0;
        while s2 < Square::COUNT {
            let mut bits = 1u128 << s2; // always include s2

            let f1 = s1 % 9;
            let r1 = s1 / 9;
            let f2 = s2 % 9;
            let r2 = s2 / 9;

            if f1 == f2 {
                // same file
                let min_r = if r1 < r2 { r1 } else { r2 };
                let max_r = if r1 > r2 { r1 } else { r2 };
                let mut r = min_r + 1;
                while r < max_r {
                    bits |= 1 << (r * 9 + f1);
                    r += 1;
                }
            } else if r1 == r2 {
                // same rank
                let min_f = if f1 < f2 { f1 } else { f2 };
                let max_f = if f1 > f2 { f1 } else { f2 };
                let mut f = min_f + 1;
                while f < max_f {
                    bits |= 1 << (r1 * 9 + f);
                    f += 1;
                }
            } else {
                // Check if knight move
                let dr = (r2 as i8 - r1 as i8).abs();
                let df = (f2 as i8 - f1 as i8).abs();
                if (dr == 2 && df == 1) || (dr == 1 && df == 2) {
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
                // same file
                if r2 > r1 {
                    let mut r = r2;
                    while r < 10 {
                        bits |= 1 << (r * 9 + f1);
                        r += 1;
                    }
                } else if r2 < r1 {
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
                // same rank
                if f2 > f1 {
                    let mut f = f2;
                    while f < 9 {
                        bits |= 1 << (r1 * 9 + f);
                        f += 1;
                    }
                } else if f2 < f1 {
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
