use strum::EnumCount;
use crate::core::{Bitboard, File, Rank, Square};
use super::types::{FileEntry, LeaperType, Magic, RankEntry};
use super::helpers::{
    bishop_attacks, cannon_attack_ray, cannon_ray, knight_attacks, knight_to_attacks,
    palace_step_attacks, pawn_attacks_from, rook_ray, sparse_rand,
};

const RANK_STRIDE: i8 = File::COUNT as i8;

/// King moves: one orthogonal step within the palace.
pub(crate) const fn init_king_attacks() -> [Bitboard; Square::COUNT] {
    const DIRS: [(i8, i8); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
    let mut table = [Bitboard::new(); Square::COUNT];
    let mut sq = 0;
    while sq < Square::COUNT {
        let from_sq = Square::from_repr(sq as u8).unwrap();
        let bits = palace_step_attacks(from_sq, &DIRS);
        table[sq] = unsafe { Bitboard::from_raw(bits) };
        sq += 1;
    }
    table
}

/// Advisor moves: one diagonal step within the palace.
pub(crate) const fn init_advisor_attacks() -> [Bitboard; Square::COUNT] {
    const DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    let mut table = [Bitboard::new(); Square::COUNT];
    let mut sq = 0;
    while sq < Square::COUNT {
        let from_sq = Square::from_repr(sq as u8).unwrap();
        let bits = palace_step_attacks(from_sq, &DIRS);
        table[sq] = unsafe { Bitboard::from_raw(bits) };
        sq += 1;
    }
    table
}

/// Forward pawn attacks: squares a pawn *at* `sq` threatens.
/// `table[0]` = Red (moves toward higher ranks), `table[1]` = Black.
pub(crate) const fn init_pawn_attacks() -> [[Bitboard; Square::COUNT]; 2] {
    let mut table = [[Bitboard::new(); Square::COUNT]; 2];
    let mut sq_idx = 0;
    while sq_idx < Square::COUNT {
        let sq = Square::from_repr(sq_idx as u8).unwrap();
        let f = sq.file() as i8;
        let r = sq.rank() as i8;
        // Red: forward = +rank, promoted when r >= 5 (has crossed the river).
        let red = pawn_attacks_from(f, r, 1, r >= 5);
        table[0][sq_idx] = unsafe { Bitboard::from_raw(red) };
        // Black: forward = -rank, promoted when r <= 4.
        let black = pawn_attacks_from(f, r, -1, r <= 4);
        table[1][sq_idx] = unsafe { Bitboard::from_raw(black) };
        sq_idx += 1;
    }
    table
}

/// Reverse pawn attacks: squares a pawn of the given colour could occupy to
/// attack target `sq`. Used in check / threat detection.
pub(crate) const fn init_pawn_attacks_to() -> [[Bitboard; Square::COUNT]; 2] {
    let mut table = [[Bitboard::new(); Square::COUNT]; 2];
    let mut sq_idx = 0;
    while sq_idx < Square::COUNT {
        let sq = Square::from_repr(sq_idx as u8).unwrap();
        let f = sq.file() as i8;
        let r = sq.rank() as i8;
        // A Red pawn at some square attacks `sq` from one rank *below* it.
        // Promotion rule is still based on `sq`'s rank (r >= 5 = past river).
        let red = pawn_attacks_from(f, r, -1, r >= 5);
        table[0][sq_idx] = unsafe { Bitboard::from_raw(red) };
        // A Black pawn attacks `sq` from one rank *above* it.
        let black = pawn_attacks_from(f, r, 1, r <= 4);
        table[1][sq_idx] = unsafe { Bitboard::from_raw(black) };
        sq_idx += 1;
    }
    table
}

/// Populates `RANK_TABLE[f]` for every file position `f` (0–8) and every
/// 9-bit occupancy (0–511).
pub(crate) const fn init_rank_table() -> [RankEntry; File::COUNT] {
    let mut table = [RankEntry {
        rook_slides: [0; 1 << File::COUNT],
        cannon_captures: [0; 1 << File::COUNT],
        cannon_attack_ray: [0; 1 << File::COUNT],
    }; File::COUNT];
    let mut f = 0i8;
    while f < File::COUNT as i8 {
        let mut occ = 0u32;
        while occ < (1 << File::COUNT) {
            table[f as usize].rook_slides[occ as usize] =
                rook_ray(f, File::COUNT as i8, occ) as u16;
            table[f as usize].cannon_captures[occ as usize] =
                cannon_ray(f, File::COUNT as i8, occ) as u16;
            table[f as usize].cannon_attack_ray[occ as usize] =
                cannon_attack_ray(f, File::COUNT as i8, occ) as u16;
            occ += 1;
        }
        f += 1;
    }
    table
}

/// Populates `FILE_TABLE[r]` for every rank position `r` (0–9) and every
/// 10-bit occupancy (0–1023).
pub(crate) const fn init_file_table() -> [FileEntry; Rank::COUNT] {
    let mut table = [FileEntry {
        rook_slides: [0; 1 << Rank::COUNT],
        cannon_captures: [0; 1 << Rank::COUNT],
        cannon_attack_ray: [0; 1 << Rank::COUNT],
    }; Rank::COUNT];
    let mut r = 0i8;
    while r < Rank::COUNT as i8 {
        let mut occ = 0u32;
        while occ < (1 << Rank::COUNT) {
            table[r as usize].rook_slides[occ as usize] =
                rook_ray(r, Rank::COUNT as i8, occ) as u16;
            table[r as usize].cannon_captures[occ as usize] =
                cannon_ray(r, Rank::COUNT as i8, occ) as u16;
            table[r as usize].cannon_attack_ray[occ as usize] =
                cannon_attack_ray(r, Rank::COUNT as i8, occ) as u16;
            occ += 1;
        }
        r += 1;
    }
    table
}

/// Helper table: `FILE_ATTACKS_BY_MASK[f][mask]` converts a 10-bit occupancy
/// bitmask on file `f` back into a full 128-bit [`Bitboard`].
pub(crate) const fn init_file_attacks_by_mask() -> [[Bitboard; 1 << Rank::COUNT]; File::COUNT] {
    let mut table = [[Bitboard::new(); 1 << Rank::COUNT]; File::COUNT];
    let mut f = 0usize;
    while f < File::COUNT {
        let mut mask = 0usize;
        while mask < (1 << Rank::COUNT) {
            let mut bits = 0u128;
            let mut r = 0usize;
            while r < Rank::COUNT {
                if (mask & (1 << r)) != 0 {
                    bits |= 1u128 << (r * File::COUNT + f);
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

/// Builds the magic lookup table for every square for a given leaper piece.
pub(crate) const fn build_magics<const SIZE: usize, const SHIFT: usize>(
    piece: LeaperType,
    dirs_dr: [i8; SHIFT],
    dirs_df: [i8; SHIFT],
) -> [Magic<SIZE>; Square::COUNT] {
    const { assert!(SIZE.trailing_zeros() == SHIFT as u32) }

    let mut magics = [Magic::<SIZE> {
        mask: 0,
        magic: 0,
        attacks: [Bitboard::new(); SIZE],
    }; Square::COUNT];

    let mut sq_idx = 0;
    while sq_idx < Square::COUNT {
        let sq = Square::from_repr(sq_idx as u8).unwrap();
        let r = sq.rank() as i8;
        let f = sq.file() as i8;

        // ── Step 1: build the occupancy mask ──────────────────────────────
        let mut mask = 0u128;
        let mut i = 0;
        while i < SHIFT {
            let er = r + dirs_dr[i];
            let ef = f + dirs_df[i];
            if er >= 0 && er < Rank::COUNT as i8 && ef >= 0 && ef < File::COUNT as i8 {
                mask |= 1 << (er * RANK_STRIDE + ef);
            }
            i += 1;
        }

        // ── Step 2: enumerate all occupancy subsets of `mask` ─────────────
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
        let shift = 128 - SHIFT;
        let mut rng = 0x9876543210ABCDEF_1234567890ABCDEF_u128 + sq_idx as u128;
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

        magics[sq_idx] = Magic {
            mask,
            magic,
            attacks: final_attacks,
        };
        sq_idx += 1;
    }
    magics
}

/// `BETWEEN_BB[s1][s2]` is the set of squares strictly between `s1` and `s2`
/// on the same rank or file, *plus* `s2` itself.
pub(crate) const fn init_between_bb() -> [[Bitboard; Square::COUNT]; Square::COUNT] {
    let mut table = [[Bitboard::new(); Square::COUNT]; Square::COUNT];
    let mut s1 = 0;
    while s1 < Square::COUNT {
        let mut s2 = 0;
        while s2 < Square::COUNT {
            let mut bits = 1u128 << s2;

            let sq1 = Square::from_repr(s1 as u8).unwrap();
            let sq2 = Square::from_repr(s2 as u8).unwrap();
            let f1 = sq1.file() as usize;
            let r1 = sq1.rank() as usize;
            let f2 = sq2.file() as usize;
            let r2 = sq2.rank() as usize;

            if f1 == f2 {
                let min_r = if r1 < r2 { r1 } else { r2 };
                let max_r = if r1 > r2 { r1 } else { r2 };
                let mut r = min_r + 1;
                while r < max_r {
                    bits |= 1 << (r * File::COUNT + f1);
                    r += 1;
                }
            } else if r1 == r2 {
                let min_f = if f1 < f2 { f1 } else { f2 };
                let max_f = if f1 > f2 { f1 } else { f2 };
                let mut f = min_f + 1;
                while f < max_f {
                    bits |= 1 << (r1 * File::COUNT + f);
                    f += 1;
                }
            } else {
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
                    bits |= 1 << (leg_r as usize * File::COUNT + leg_f as usize);
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
pub(crate) const fn init_ray_pass_bb() -> [[Bitboard; Square::COUNT]; Square::COUNT] {
    let mut table = [[Bitboard::new(); Square::COUNT]; Square::COUNT];
    let mut s1 = 0;
    while s1 < Square::COUNT {
        let mut s2 = 0;
        while s2 < Square::COUNT {
            let sq1 = Square::from_repr(s1 as u8).unwrap();
            let sq2 = Square::from_repr(s2 as u8).unwrap();
            let f1 = sq1.file() as usize;
            let r1 = sq1.rank() as usize;
            let f2 = sq2.file() as usize;
            let r2 = sq2.rank() as usize;
            let mut bits = 0u128;

            if f1 == f2 {
                if r2 > r1 {
                    let mut r = r2;
                    while r < Rank::COUNT {
                        bits |= 1 << (r * File::COUNT + f1);
                        r += 1;
                    }
                } else if r2 < r1 {
                    let mut r = r2;
                    loop {
                        bits |= 1 << (r * File::COUNT + f1);
                        if r == 0 {
                            break;
                        }
                        r -= 1;
                    }
                }
            } else if r1 == r2 {
                if f2 > f1 {
                    let mut f = f2;
                    while f < File::COUNT {
                        bits |= 1 << (r1 * File::COUNT + f);
                        f += 1;
                    }
                } else if f2 < f1 {
                    let mut f = f2;
                    loop {
                        bits |= 1 << (r1 * File::COUNT + f);
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
