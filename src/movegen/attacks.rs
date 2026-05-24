use crate::{bitboard::Bitboard, types::Square};

use super::tables::{FILE_TABLE, RANK_TABLE};

/// Collects (gathers) the vertical file occupancy states into a 10-bit integer.
/// Every 9th bit in our `u128` bitboard represents the same file on successive ranks (R0 to R9).
/// Shifts, masks, and packs these bits dynamically in O(1) time without standard loops.
#[inline(always)]
pub fn gather_file_bits(bits: u128, f: usize) -> usize {
    let mut file_occ = 0;
    let occ = bits >> f;
    file_occ |= (occ & 1) as usize;
    file_occ |= (((occ >> 9) & 1) as usize) << 1;
    file_occ |= (((occ >> 18) & 1) as usize) << 2;
    file_occ |= (((occ >> 27) & 1) as usize) << 3;
    file_occ |= (((occ >> 36) & 1) as usize) << 4;
    file_occ |= (((occ >> 45) & 1) as usize) << 5;
    file_occ |= (((occ >> 54) & 1) as usize) << 6;
    file_occ |= (((occ >> 63) & 1) as usize) << 7;
    file_occ |= (((occ >> 72) & 1) as usize) << 8;
    file_occ |= (((occ >> 81) & 1) as usize) << 9;
    file_occ
}

/// Computes horizontal and vertical attack/move targets for a Rook (Chariot).
/// Combines precomputed `RANK_TABLE` and `FILE_TABLE` lookup masks.
#[inline(always)]
pub fn rook_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    let from_idx = square as usize;
    let f = from_idx % 9;
    let r = from_idx / 9;

    // 1. Rank attacks: Mask the 9 bits of the current rank (offset `r * 9`)
    let rank_occ = ((occupied.0 >> (r * 9)) & 0x1FF) as usize;
    let rank_attack_mask = RANK_TABLE[f].rook[rank_occ];
    let mut attack_bb = Bitboard((rank_attack_mask as u128) << (r * 9));

    // 2. File attacks: Gather the 10 bits of the current file
    let file_occ = gather_file_bits(occupied.0, f);
    let file_attack_mask = FILE_TABLE[r].rook[file_occ];
    let mut file_mask_bb = 0u128;
    let mut temp = file_attack_mask;
    while temp != 0 {
        let r_to = temp.trailing_zeros() as usize;
        temp &= temp - 1;
        file_mask_bb |= 1 << (r_to * 9 + f);
    }
    attack_bb.0 |= file_mask_bb;
    attack_bb
}

/// Computes horizontal and vertical quiet/leap capture masks for a Cannon.
#[inline(always)]
pub fn cannon_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    let from_idx = square as usize;
    let f = from_idx % 9;
    let r = from_idx / 9;

    // 1. Rank attacks: Mask the 9 bits of the rank
    let rank_occ = ((occupied.0 >> (r * 9)) & 0x1FF) as usize;
    let rank_attack_mask = RANK_TABLE[f].cannon[rank_occ];
    let mut attack_bb = Bitboard((rank_attack_mask as u128) << (r * 9));

    // 2. File attacks: Gather the 10 bits of the file
    let file_occ = gather_file_bits(occupied.0, f);
    let file_attack_mask = FILE_TABLE[r].cannon[file_occ];
    let mut file_mask_bb = 0u128;
    let mut temp = file_attack_mask;
    while temp != 0 {
        let r_to = temp.trailing_zeros() as usize;
        temp &= temp - 1;
        file_mask_bb |= 1 << (r_to * 9 + f);
    }
    attack_bb.0 |= file_mask_bb;
    attack_bb
}
