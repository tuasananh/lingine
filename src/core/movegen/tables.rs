use strum::EnumCount;

use crate::core::{Bitboard, File, Rank, Side, Square};

/// Holds precomputed horizontal rank attack/move masks for Rooks and Cannons.
/// Indexed by the 9-bit rank occupancy state (0 to 511).
#[derive(Clone, Copy)]
pub struct RankEntry {
    /// Rook sliding quiet and capture targets. Stops sliding immediately upon
    /// hitting a blocker.
    pub rook: [u16; 512],
    /// Cannon quiet and leap capture targets. Skips quiet squares, jumps over
    /// exactly 1 screen, and targets the first piece behind it.
    pub cannon: [u16; 512],
}

/// Holds precomputed vertical file attack/move masks for Rooks and Cannons.
/// Indexed by the 10-bit file occupancy state (0 to 1023).
#[derive(Clone, Copy)]
pub struct FileEntry {
    /// Rook vertical sliding targets.
    pub rook: [u16; 1024],
    /// Cannon vertical sliding and leap capture targets.
    pub cannon: [u16; 1024],
}

/// Precomputes valid orthogonal moves for the General (King) inside the Palace.
/// Generals can only move 1 step orthogonally (up/down/left/right) and are
/// strictly confined to the 3x3 Palace.
const fn init_king_attacks() -> [Bitboard; Square::COUNT] {
    let mut table = [Bitboard::new(); Square::COUNT];
    let mut from_idx = 0;
    while from_idx < Square::COUNT {
        let f = (from_idx % 9) as i8;
        let r = (from_idx / 9) as i8;
        let is_white_palace = (f >= 3 && f <= 5) && (r >= 0 && r <= 2);
        let is_black_palace = (f >= 3 && f <= 5) && (r >= 7 && r <= 9);
        if is_white_palace || is_black_palace {
            let king_dirs = [(0, 1), (0, -1), (1, 0), (-1, 0)];
            let mut i = 0;
            let mut mask = 0u128;
            while i < 4 {
                let (df, dr) = king_dirs[i];
                let nf = f + df;
                let nr = r + dr;
                let is_in_same_palace = if is_white_palace {
                    (nf >= 3 && nf <= 5) && (nr >= 0 && nr <= 2)
                } else {
                    (nf >= 3 && nf <= 5) && (nr >= 7 && nr <= 9)
                };
                if is_in_same_palace {
                    let to_idx = nr * 9 + nf;
                    mask |= 1 << to_idx;
                }
                i += 1;
            }
            table[from_idx] = unsafe { Bitboard::from_raw(mask) };
        }
        from_idx += 1;
    }
    table
}

/// Precomputes valid diagonal moves for Advisors inside the Palace.
/// Advisors move exactly 1 step diagonally and are strictly confined to the
/// Palace. There are exactly 5 valid Palace squares for an Advisor.
const fn init_advisor_attacks() -> [Bitboard; Square::COUNT] {
    let mut table = [Bitboard::new(); Square::COUNT];
    let mut from_idx = 0;
    while from_idx < Square::COUNT {
        let f = (from_idx % 9) as i8;
        let r = (from_idx / 9) as i8;
        let is_white_palace = (f >= 3 && f <= 5) && (r >= 0 && r <= 2);
        let is_black_palace = (f >= 3 && f <= 5) && (r >= 7 && r <= 9);
        if is_white_palace || is_black_palace {
            let advisor_dirs = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
            let mut i = 0;
            let mut mask = 0u128;
            while i < 4 {
                let (df, dr) = advisor_dirs[i];
                let nf = f + df;
                let nr = r + dr;
                let is_in_same_palace = if is_white_palace {
                    (nf >= 3 && nf <= 5) && (nr >= 0 && nr <= 2)
                } else {
                    (nf >= 3 && nf <= 5) && (nr >= 7 && nr <= 9)
                };
                if is_in_same_palace {
                    let to_idx = nr * 9 + nf;
                    mask |= 1 << to_idx;
                }
                i += 1;
            }
            table[from_idx] = unsafe { Bitboard::from_raw(mask) };
        }
        from_idx += 1;
    }
    table
}

/// Precomputes Pawn attack masks for both White and Black Pawns.
///
/// * **Unpromoted (own side)**: Can only move exactly 1 step straight forward.
/// * **Promoted (crossed river)**: Can move 1 step straight forward OR 1 step
///   horizontally (left/right).
const fn init_pawn_attacks() -> [[Bitboard; Square::COUNT]; 2] {
    let mut table = [[Bitboard::new(); Square::COUNT]; 2];

    // Index 0: White Pawn
    let mut from_idx = 0;
    while from_idx < Square::COUNT {
        let f = (from_idx % 9) as i8;
        let r = (from_idx / 9) as i8;
        let mut mask = 0u128;
        if r + 1 < 10 {
            mask |= 1 << ((r + 1) * 9 + f); // Forward move
        }
        if r >= 5 {
            // Crossed river (R5 to R9 are opponent's ranks)
            if f > 0 {
                mask |= 1 << (r * 9 + f - 1); // Left sideways move
            }
            if f + 1 < 9 {
                mask |= 1 << (r * 9 + f + 1); // Right sideways move
            }
        }
        table[0][from_idx] = unsafe { Bitboard::from_raw(mask) };
        from_idx += 1;
    }

    // Index 1: Black Pawn
    let mut from_idx = 0;
    while from_idx < Square::COUNT {
        let f = (from_idx % 9) as i8;
        let r = (from_idx / 9) as i8;
        let mut mask = 0u128;
        if r > 0 {
            mask |= 1 << ((r - 1) * 9 + f); // Forward move
        }
        if r <= 4 {
            // Crossed river (R0 to R4 are opponent's ranks)
            if f > 0 {
                mask |= 1 << (r * 9 + f - 1); // Left sideways move
            }
            if f + 1 < 9 {
                mask |= 1 << (r * 9 + f + 1); // Right sideways move
            }
        }
        table[1][from_idx] = unsafe { Bitboard::from_raw(mask) };
        from_idx += 1;
    }

    table
}

/// Precomputes "reverse" Pawn attack lists.
/// Represents which source squares a Pawn of the given color could have come
/// from to attack the target square. Used in `Position::checkers_to` to check
/// Pawn checks.
const fn init_pawn_attacks_to() -> [[Bitboard; Square::COUNT]; 2] {
    let mut table = [[Bitboard::new(); Square::COUNT]; 2];

    // Index 0: White pawn attacking a square
    let mut square_idx = 0;
    while square_idx < Square::COUNT {
        let f = (square_idx % 9) as i8;
        let r = (square_idx / 9) as i8;
        let mut mask = 0u128;
        if r > 0 {
            mask |= 1 << ((r - 1) * 9 + f); // Came from behind
        }
        if r >= 5 {
            if f > 0 {
                mask |= 1 << (r * 9 + f - 1); // Came from left
            }
            if f + 1 < 9 {
                mask |= 1 << (r * 9 + f + 1); // Came from right
            }
        }
        table[0][square_idx] = unsafe { Bitboard::from_raw(mask) };
        square_idx += 1;
    }

    // Index 1: Black pawn attacking a square
    let mut square_idx = 0;
    while square_idx < Square::COUNT {
        let f = (square_idx % 9) as i8;
        let r = (square_idx / 9) as i8;
        let mut mask = 0u128;
        if r + 1 < 10 {
            mask |= 1 << ((r + 1) * 9 + f); // Came from in front
        }
        if r <= 4 {
            if f > 0 {
                mask |= 1 << (r * 9 + f - 1); // Came from left
            }
            if f + 1 < 9 {
                mask |= 1 << (r * 9 + f + 1); // Came from right
            }
        }
        table[1][square_idx] = unsafe { Bitboard::from_raw(mask) };
        square_idx += 1;
    }

    table
}

/// Precomputes sliding Rank occupancy attack masks.
/// A rank has 9 files, so the occupancy is a 9-bit number (0..511).
///
/// * **Rook**: Slides orthogonally, stopping on the first blocking piece (quiet
///   on empty, capture on opponent).
/// * **Cannon**: Quiet moves are identical to Rook, but captures require
///   jumping over exactly 1 blocking screen, landing on the next piece.
const fn init_rank_table() -> [RankEntry; 9] {
    let mut table = [RankEntry {
        rook: [0; 512],
        cannon: [0; 512],
    }; 9];
    let mut f = 0;
    while f < 9 {
        let mut occ = 0;
        while occ < 512 {
            // 1. Rook horizontal attack generation
            let mut r_mask = 0u16;
            let mut temp_f = f - 1;
            while temp_f >= 0 {
                r_mask |= 1 << temp_f;
                if (occ & (1 << temp_f)) != 0 {
                    break; // Hit a blocking piece
                }
                temp_f -= 1;
            }
            let mut temp_f = f + 1;
            while temp_f < 9 {
                r_mask |= 1 << temp_f;
                if (occ & (1 << temp_f)) != 0 {
                    break; // Hit a blocking piece
                }
                temp_f += 1;
            }
            table[f as usize].rook[occ as usize] = r_mask;

            // 2. Cannon horizontal leap capture generation
            let mut c_mask = 0u16;
            let mut temp_f = f - 1;
            let mut screen = false; // Tracks if we have crossed exactly 1 screen
            while temp_f >= 0 {
                let occupied = (occ & (1 << temp_f)) != 0;
                if !screen {
                    if occupied {
                        screen = true; // Found the screen
                    }
                } else {
                    if occupied {
                        c_mask |= 1 << temp_f; // Found the target behind the screen!
                        break;
                    }
                }
                temp_f -= 1;
            }
            let mut temp_f = f + 1;
            let mut screen = false;
            while temp_f < 9 {
                let occupied = (occ & (1 << temp_f)) != 0;
                if !screen {
                    if occupied {
                        screen = true; // Found the screen
                    }
                } else {
                    if occupied {
                        c_mask |= 1 << temp_f; // Found the target behind the screen!
                        break;
                    }
                }
                temp_f += 1;
            }
            table[f as usize].cannon[occ as usize] = c_mask;

            occ += 1;
        }
        f += 1;
    }
    table
}

/// Precomputes sliding File occupancy attack masks.
/// A file has 10 ranks, so the occupancy is a 10-bit number (0..1023).
/// Generates vertical Rook attacks and Cannon leap capture targets.
const fn init_file_table() -> [FileEntry; 10] {
    let mut table = [FileEntry {
        rook: [0; 1024],
        cannon: [0; 1024],
    }; 10];
    let mut r = 0;
    while r < 10 {
        let mut occ = 0;
        while occ < 1024 {
            // 1. Rook vertical attack generation
            let mut r_mask = 0u16;
            let mut temp_r = r - 1;
            while temp_r >= 0 {
                r_mask |= 1 << temp_r;
                if (occ & (1 << temp_r)) != 0 {
                    break;
                }
                temp_r -= 1;
            }
            let mut temp_r = r + 1;
            while temp_r < 10 {
                r_mask |= 1 << temp_r;
                if (occ & (1 << temp_r)) != 0 {
                    break;
                }
                temp_r += 1;
            }
            table[r as usize].rook[occ as usize] = r_mask;

            // 2. Cannon vertical leap capture generation
            let mut c_mask = 0u16;
            let mut temp_r = r - 1;
            let mut screen = false;
            while temp_r >= 0 {
                let occupied = (occ & (1 << temp_r)) != 0;
                if !screen {
                    if occupied {
                        screen = true;
                    }
                } else {
                    if occupied {
                        c_mask |= 1 << temp_r;
                        break;
                    }
                }
                temp_r -= 1;
            }
            let mut temp_r = r + 1;
            let mut screen = false;
            while temp_r < 10 {
                let occupied = (occ & (1 << temp_r)) != 0;
                if !screen {
                    if occupied {
                        screen = true;
                    }
                } else {
                    if occupied {
                        c_mask |= 1 << temp_r;
                        break;
                    }
                }
                temp_r += 1;
            }
            table[r as usize].cannon[occ as usize] = c_mask;

            occ += 1;
        }
        r += 1;
    }
    table
}

// Precomputed static lookup tables dissolved at compile-time to eliminate
// thread checks, lock contention, and atomic operations during perft search
// loops.
pub static KING_ATTACKS: [Bitboard; Square::COUNT] = init_king_attacks();
pub static ADVISOR_ATTACKS: [Bitboard; Square::COUNT] = init_advisor_attacks();
pub static PAWN_ATTACKS: [[Bitboard; Square::COUNT]; Side::COUNT] = init_pawn_attacks();
pub static PAWN_ATTACKS_TO: [[Bitboard; Square::COUNT]; 2] = init_pawn_attacks_to();
pub static RANK_TABLE: [RankEntry; File::COUNT] = init_rank_table();
pub static FILE_TABLE: [FileEntry; Rank::COUNT] = init_file_table();

const fn init_file_attacks_by_mask() -> [[Bitboard; 1024]; 9] {
    let mut table = [[Bitboard::new(); 1024]; 9];
    let mut f = 0;
    while f < 9 {
        let mut mask = 0;
        while mask < 1024 {
            let mut bits = 0u128;
            let mut r = 0;
            while r < 10 {
                if (mask & (1 << r)) != 0 {
                    bits |= 1u128 << (r * 9 + f);
                }
                r += 1;
            }
            table[f as usize][mask as usize] = unsafe { Bitboard::from_raw(bits) };
            mask += 1;
        }
        f += 1;
    }
    table
}

pub static FILE_ATTACKS_BY_MASK: [[Bitboard; 1024]; 9] = init_file_attacks_by_mask();

// ============================================================================
// COMPILE-TIME MAGIC BITBOARD GENERATOR FOR LEAPERS
// ============================================================================

#[derive(Clone, Copy)]
pub struct Magic<const SIZE: usize> {
    pub mask: u128,
    pub magic: u128,
    pub attacks: [Bitboard; SIZE],
}

impl<const SIZE: usize> Magic<SIZE> {
    #[inline]
    pub const fn attack(&self, occupied: Bitboard) -> Bitboard {
        let occ_idx =
            (occupied.raw() & self.mask).wrapping_mul(self.magic) >> (128 - SIZE.trailing_zeros());
        self.attacks[occ_idx as usize]
    }
}

// 1. A Deterministic PRNG allowed in const fn
const fn next_random(state: &mut u128) -> u128 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

const fn sparse_random(state: &mut u128) -> u128 {
    next_random(state) & next_random(state) & next_random(state)
}

// 2. Attack Generators
const fn compute_knight_attacks(sq: usize, occ: u128) -> u128 {
    let mut attacks = 0u128;
    let r = (sq / 9) as i32;
    let f = (sq % 9) as i32;
    let legs_dr = [1, -1, 0, 0];
    let legs_df = [0, 0, 1, -1];
    let targets_dr = [[2, 2], [-2, -2], [-1, 1], [-1, 1]];
    let targets_df = [[-1, 1], [-1, 1], [2, 2], [-2, -2]];

    let mut i = 0;
    while i < 4 {
        let lr = r + legs_dr[i];
        let lf = f + legs_df[i];
        if lr >= 0 && lr < 10 && lf >= 0 && lf < 9 {
            let leg_sq = (lr * 9 + lf) as usize;
            if (occ & (1 << leg_sq)) == 0 {
                let mut t = 0;
                while t < 2 {
                    let tr = r + targets_dr[i][t];
                    let tf = f + targets_df[i][t];
                    if tr >= 0 && tr < 10 && tf >= 0 && tf < 9 {
                        attacks |= 1 << (tr * 9 + tf);
                    }
                    t += 1;
                }
            }
        }
        i += 1;
    }
    attacks
}

const fn compute_bishop_attacks(sq: usize, occ: u128) -> u128 {
    let mut attacks = 0u128;
    let r = (sq / 9) as i32;
    let f = (sq % 9) as i32;
    let dirs_dr = [1, 1, -1, -1];
    let dirs_df = [1, -1, 1, -1];

    let mut i = 0;
    while i < 4 {
        let er = r + dirs_dr[i];
        let ef = f + dirs_df[i];
        let tr = r + dirs_dr[i] * 2;
        let tf = f + dirs_df[i] * 2;

        if tr >= 0
            && tr < 10
            && tf >= 0
            && tf < 9
            && er >= 0
            && er < 10
            && ef >= 0
            && ef < 9
            && ((r < 5 && tr < 5) || (r >= 5 && tr >= 5))
        {
            let eye_sq = (er * 9 + ef) as usize;
            if (occ & (1 << eye_sq)) == 0 {
                attacks |= 1 << (tr * 9 + tf);
            }
        }
        i += 1;
    }
    attacks
}

const fn compute_knight_to_attacks(sq: usize, occ: u128) -> u128 {
    let mut attacks = 0u128;
    let r = (sq / 9) as i32;
    let f = (sq % 9) as i32;

    // The 8 possible originating squares for a knight targeting `sq`
    let origins_dr = [2, 2, -2, -2, 1, 1, -1, -1];
    let origins_df = [1, -1, 1, -1, 2, -2, 2, -2];

    let mut i = 0;
    while i < 8 {
        let or = r + origins_dr[i];
        let of = f + origins_df[i];

        // If the origin square is valid on the 9x10 board...
        if or >= 0 && or < 10 && of >= 0 && of < 9 {
            // The blocking leg is always diagonally adjacent to the destination
            // square in the direction of the origin square.
            let leg_r = r + if origins_dr[i] > 0 { 1 } else { -1 };
            let leg_f = f + if origins_df[i] > 0 { 1 } else { -1 };

            let leg_sq = (leg_r * 9 + leg_f) as usize;

            // If the leg is empty, the incoming attack is valid
            if (occ & (1 << leg_sq)) == 0 {
                attacks |= 1 << (or * 9 + of);
            }
        }
        i += 1;
    }
    attacks
}

pub enum LeaperType {
    Knight,
    Bishop,
    KnightTo,
}

const fn build_magics<const SIZE: usize, const SHIFT: usize>(
    piece: LeaperType,
    dirs_dr: [i32; SHIFT],
    dirs_df: [i32; SHIFT],
) -> [Magic<SIZE>; Square::COUNT] {
    assert!(
        SIZE.trailing_zeros() == SHIFT as u32,
        "Shift must match trailing zeros of SIZE"
    );

    let mut magics = [Magic::<SIZE> {
        mask: 0,
        magic: 0,
        attacks: [Bitboard::new(); SIZE],
    }; Square::COUNT];

    let mut sq = 0;
    while sq < Square::COUNT {
        let r = (sq / 9) as i32;
        let f = (sq % 9) as i32;
        let mut mask = 0u128;

        let mut i = 0;
        while i < SHIFT {
            let er = r + dirs_dr[i];
            let ef = f + dirs_df[i];
            if er >= 0 && er < 10 && ef >= 0 && ef < 9 {
                mask |= 1 << ((er * 9 + ef) as usize);
            }
            i += 1;
        }

        let mut bits = [0; SHIFT];
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
            let mut occ = 0u128;
            let mut k = 0;
            while k < SHIFT {
                if (j & (1 << k)) != 0 {
                    occ |= 1 << bits[k];
                }
                k += 1;
            }

            occupancies[j] = occ;

            // Dispatch to the correct attack generator
            ref_attacks[j] = match piece {
                LeaperType::Knight => compute_knight_attacks(sq, occ),
                LeaperType::Bishop => compute_bishop_attacks(sq, occ),
                LeaperType::KnightTo => compute_knight_to_attacks(sq, occ),
            };
            j += 1;
        }

        let shift = 128 - SHIFT;
        let mut prng_state = 0x9876543210ABCDEF_1234567890ABCDEF + sq as u128;
        let mut found_magic = 0;
        let mut final_attacks = [Bitboard::new(); SIZE];
        let mut searching = true;

        while searching {
            let candidate = sparse_random(&mut prng_state);
            let mut used = [false; SIZE];
            let mut attacks = [0u128; SIZE];
            let mut fail = false;

            let mut k = 0;
            while k < SIZE {
                let hash_idx = (occupancies[k].wrapping_mul(candidate) >> shift) as usize;
                if used[hash_idx] {
                    if attacks[hash_idx] != ref_attacks[k] {
                        fail = true;
                        break;
                    }
                } else {
                    used[hash_idx] = true;
                    attacks[hash_idx] = ref_attacks[k];
                }
                k += 1;
            }

            if !fail {
                found_magic = candidate;
                let mut idx = 0;
                while idx < SIZE {
                    final_attacks[idx] = unsafe { Bitboard::from_raw(attacks[idx]) };
                    idx += 1;
                }
                searching = false;
            }
        }

        magics[sq] = Magic {
            mask,
            magic: found_magic,
            attacks: final_attacks,
        };
        sq += 1;
    }
    magics
}

// These run at compile time!
pub static KNIGHT_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::Knight, [1, -1, 0, 0], [0, 0, 1, -1]);

pub static BISHOP_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::Bishop, [1, 1, -1, -1], [1, -1, 1, -1]);

// Backward Knights share the Bishop's blocking square offsets!
pub static KNIGHT_TO_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::KnightTo, [1, 1, -1, -1], [1, -1, 1, -1]);
