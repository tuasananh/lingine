use super::helpers::{
    bishop_attacks, cannon_ray, knight_attacks, knight_to_attacks, palace_step_attacks,
    pawn_attacks_from, rook_ray, sparse_rand,
};
use super::types::{FileEntry, LeaperType, Magic, RankEntry};
use crate::core::movegen::tables::helpers::cannon_beyond_attack;
use crate::core::{Bitboard, File, Rank, Square, cannon_beyond_attacks, rook_attacks};
use strum::EnumCount;

const RANK_STRIDE: i8 = File::COUNT as i8;

/// King moves: one orthogonal step within the palace.
pub(super) const fn init_king_attacks() -> [Bitboard; Square::COUNT] {
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
pub(super) const fn init_advisor_attacks() -> [Bitboard; Square::COUNT] {
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
pub(super) const fn init_pawn_attacks() -> [[Bitboard; Square::COUNT]; 2] {
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
pub(super) const fn init_pawn_attacks_to() -> [[Bitboard; Square::COUNT]; 2] {
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
pub(super) const fn init_rank_table() -> [RankEntry; File::COUNT] {
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
                cannon_beyond_attack(f, File::COUNT as i8, occ) as u16;
            occ += 1;
        }
        f += 1;
    }
    table
}

/// Populates `FILE_TABLE[r]` for every rank position `r` (0–9) and every
/// 10-bit occupancy (0–1023).
pub(super) const fn init_file_table() -> [FileEntry; Rank::COUNT] {
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
                cannon_beyond_attack(r, Rank::COUNT as i8, occ) as u16;
            occ += 1;
        }
        r += 1;
    }
    table
}

/// Helper table: `FILE_ATTACKS_BY_MASK[f][mask]` converts a 10-bit occupancy
/// bitmask on file `f` back into a full 128-bit [`Bitboard`].
pub(super) const fn init_file_attacks_by_mask() -> [[Bitboard; 1 << Rank::COUNT]; File::COUNT] {
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
pub(super) const fn build_magics<const SIZE: usize, const SHIFT: usize>(
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

/// `SQUARES_BETWEEN[s1][s2]` is the set of squares strictly between `s1` and
/// `s2` on the same rank or file, *plus* `s2` itself.
pub(super) const fn init_squares_between() -> [[Bitboard; Square::COUNT]; Square::COUNT] {
    let mut table = [[Bitboard::new(); Square::COUNT]; Square::COUNT];
    let mut s1 = 0;
    while s1 < Square::COUNT {
        let mut s2 = 0;
        while s2 < Square::COUNT {
            let sq1 = Square::from_repr(s1 as u8).unwrap();
            let sq2 = Square::from_repr(s2 as u8).unwrap();
            table[s1][s2] = {
                let f1 = sq1.file() as i8;
                let r1 = sq1.rank() as i8;
                let f2 = sq2.file() as i8;
                let r2 = sq2.rank() as i8;

                if f1 == f2 || r1 == r2 {
                    rook_attacks(sq1, Bitboard::from_square(sq2))
                        .const_and(rook_attacks(sq2, Bitboard::from_square(sq1)))
                        .const_or(Bitboard::from_square(sq2))
                } else {
                    let dr = (r2 - r1).abs();
                    let df = (f2 - f1).abs();
                    if (dr == 2 && df == 1) || (dr == 1 && df == 2) {
                        let leg_r = if dr == 2 { (r1 + r2) / 2 } else { r2 };
                        let leg_f = if df == 2 { (f1 + f2) / 2 } else { f2 };
                        let leg_sq = Square::from_repr((leg_r * 9 + leg_f) as u8).unwrap();
                        Bitboard::from_square(leg_sq).const_or(Bitboard::from_square(sq2))
                    } else {
                        Bitboard::new()
                    }
                }
            };
            s2 += 1;
        }
        s1 += 1;
    }
    table
}

/// `SQUARES_BEYOND[s1][s2]` is the set of squares from `s2` extending *away*
/// from `s1` to the edge of the board, along the same rank or file.
pub(super) const fn init_squares_beyond() -> [[Bitboard; Square::COUNT]; Square::COUNT] {
    let mut table = [[Bitboard::new(); Square::COUNT]; Square::COUNT];
    let mut s1 = 0;
    while s1 < Square::COUNT {
        let mut s2 = 0;
        while s2 < Square::COUNT {
            let sq1 = Square::from_repr(s1 as u8).unwrap();
            let sq2 = Square::from_repr(s2 as u8).unwrap();
            table[s1][s2] = {
                let f1 = sq1.file() as i8;
                let r1 = sq1.rank() as i8;
                let f2 = sq2.file() as i8;
                let r2 = sq2.rank() as i8;

                if f1 == f2 || r1 == r2 {
                    cannon_beyond_attacks(sq1, Bitboard::from_square(sq2))
                        .const_or(Bitboard::from_square(sq2))
                } else {
                    Bitboard::new()
                }
            };
            s2 += 1;
        }
        s1 += 1;
    }
    table
}

pub(super) const fn init_squares_in_line() -> [[Bitboard; Square::COUNT]; Square::COUNT] {
    let mut table = [[Bitboard::new(); Square::COUNT]; Square::COUNT];
    let mut s1 = 0;
    while s1 < Square::COUNT {
        let mut s2 = 0;
        while s2 < Square::COUNT {
            let sq1 = Square::from_repr(s1 as u8).unwrap();
            let sq2 = Square::from_repr(s2 as u8).unwrap();
            let f1 = sq1.file() as u8;
            let f2 = sq2.file() as u8;
            let r1 = sq1.rank() as u8;
            let r2 = sq2.rank() as u8;
            table[s1][s2] = if f1 == f2 || r1 == r2 {
                rook_attacks(sq1, Bitboard::new())
                    .const_and(rook_attacks(sq2, Bitboard::new()))
                    .const_or(Bitboard::from_square(sq1))
                    .const_or(Bitboard::from_square(sq2))
            } else {
                Bitboard::new()
            };
            s2 += 1;
        }
        s1 += 1;
    }
    table
}
