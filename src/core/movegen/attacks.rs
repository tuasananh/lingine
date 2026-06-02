use crate::core::{bitboard::Bitboard, types::Square};

use super::tables::{FILE_ATTACKS_BY_MASK, FILE_TABLE, RANK_TABLE};

/// Collects (gathers) the vertical file occupancy states into a 10-bit integer.
/// Every 9th bit in our `u128` bitboard represents the same file on successive
/// ranks (R0 to R9). Shifts, masks, and packs these bits dynamically in O(1)
/// time without standard loops.
#[inline]
pub fn gather_file_bits(bits: u128, f: usize) -> usize {
    let occ = bits >> f;
    let low = occ as u64;
    let high = (occ >> 45) as u64;

    // Mask the 5 bits at positions 0, 9, 18, 27, 36 for low and high.
    let val_low = low & 0x10_0804_0201;
    let val_high = high & 0x10_0804_0201;

    // Multiply by a magic factor that packs the 5 spaced bits into contiguous bits
    // starting at bit 36, then shift down by 36 and mask the lowest 5 bits.
    let key_low = (val_low.wrapping_mul(0x1010101010) >> 36) & 0x1F;
    let key_high = (val_high.wrapping_mul(0x1010101010) >> 36) & 0x1F;

    (key_low | (key_high << 5)) as usize
}

/// Computes horizontal and vertical attack/move targets for a Rook (Chariot).
/// Combines precomputed `RANK_TABLE` and `FILE_TABLE` lookup masks.
#[inline]
pub fn rook_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    let from_idx = square as usize;
    let f = from_idx % 9;
    let r = from_idx / 9;

    // 1. Rank attacks: Mask the 9 bits of the current rank (offset `r * 9`)
    let rank_occ = ((occupied.raw() >> (r * 9)) & 0x1FF) as usize;
    let rank_attack_mask = RANK_TABLE[f].rook[rank_occ];
    let mut attack_bb = Bitboard::from_raw((rank_attack_mask as u128) << (r * 9));

    // 2. File attacks: Gather the 10 bits of the current file
    let file_occ = gather_file_bits(occupied.raw(), f);
    let file_attack_mask = FILE_TABLE[r].rook[file_occ];

    let file_mask_bb = FILE_ATTACKS_BY_MASK[f][file_attack_mask as usize];

    attack_bb |= file_mask_bb;
    attack_bb
}

/// Computes horizontal and vertical quiet/leap capture masks for a Cannon.
#[inline]
pub fn cannon_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    let from_idx = square as usize;
    let f = from_idx % 9;
    let r = from_idx / 9;

    // 1. Rank attacks: Mask the 9 bits of the rank
    let rank_occ = ((occupied.raw() >> (r * 9)) & 0x1FF) as usize;
    let rank_attack_mask = RANK_TABLE[f].cannon[rank_occ];
    let mut attack_bb = Bitboard::from_raw((rank_attack_mask as u128) << (r * 9));

    // 2. File attacks: Gather the 10 bits of the file
    let file_occ = gather_file_bits(occupied.raw(), f);
    let file_attack_mask = FILE_TABLE[r].cannon[file_occ];

    let file_mask_bb = FILE_ATTACKS_BY_MASK[f][file_attack_mask as usize];

    attack_bb |= file_mask_bb;
    attack_bb
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::movegen::tables::*;

    #[test]
    fn test_gather_file_bits() {
        // If the bitboard has no pieces in file 0, file bits should be 0
        assert_eq!(gather_file_bits(0, 0), 0);

        // A single piece at A0 (rank 0, file 0)
        let bits_a0 = 1u128 << 0;
        assert_eq!(gather_file_bits(bits_a0, 0), 1 << 0);

        // A single piece at A9 (rank 9, file 0)
        let bits_a9 = 1u128 << (9 * 9);
        assert_eq!(gather_file_bits(bits_a9, 0), 1 << 9);

        // Two pieces at A3 and A7
        let bits_a3_a7 = (1u128 << (3 * 9)) | (1u128 << (7 * 9));
        assert_eq!(gather_file_bits(bits_a3_a7, 0), (1 << 3) | (1 << 7));

        // A piece on File 4 (File E), Rank 5
        let bits_e5 = 1u128 << (5 * 9 + 4);
        assert_eq!(gather_file_bits(bits_e5, 4), 1 << 5);
    }

    #[test]
    fn test_rook_attacks() {
        // Rook at A0 on empty board
        let empty = Bitboard::from_raw(0);
        let att_a0 = rook_attacks(Square::A0, empty);
        // Should attack B0..I0 (files 1..8 on rank 0) and A1..A9 (file 0 on ranks 1..9)
        let mut expected_mask = 0u128;
        for f in 1..9 {
            expected_mask |= 1u128 << f;
        }
        for r in 1..10 {
            expected_mask |= 1u128 << (r * 9);
        }
        assert_eq!(att_a0.raw(), expected_mask);

        // Rook at A0, blocked by a piece at A2 and B0
        let blocked = Bitboard::from_raw((1u128 << (2 * 9)) | (1u128 << 1));
        let att_a0_blocked = rook_attacks(Square::A0, blocked);
        // Should attack B0 (the blocker on rank 0) and A1, A2 (the blocker on rank 2).
        // No further.
        let mut expected_blocked_mask = 0u128;
        expected_blocked_mask |= 1u128 << 1; // B0
        expected_blocked_mask |= 1u128 << 9; // A1
        expected_blocked_mask |= 1u128 << (2 * 9); // A2
        assert_eq!(att_a0_blocked.raw(), expected_blocked_mask);
    }

    #[test]
    fn test_cannon_attacks() {
        // Cannon quiet attacks are 0 on an empty board (cannon_attacks only computes
        // leap captures)
        let empty = Bitboard::from_raw(0);
        let att_cannon_a0 = cannon_attacks(Square::A0, empty);
        assert_eq!(att_cannon_a0.raw(), 0);

        // Cannon at A0, blocker (screen) at A2, and enemy at A5
        let occupied = Bitboard::from_raw((1u128 << (2 * 9)) | (1u128 << (5 * 9)));
        let att_cannon_blocked = cannon_attacks(Square::A0, occupied);
        // Upwards file leap capture target behind screen A2: A5.

        // General can move orthogonally inside Palace: D0, F0, E1
        let mut expected = 0u128;
        expected |= 1u128 << 3; // D0
        expected |= 1u128 << 5; // F0
        expected |= 1u128 << (9 + 4); // E1
        assert_eq!(att_cannon_blocked.raw(), 1u128 << (5 * 9));
        assert_eq!(KING_ATTACKS[Square::E0 as usize].raw(), expected);

        // Advisor from center of Palace (E1)
        let advisor_e1 = ADVISOR_ATTACKS[Square::E1 as usize];
        // Advisor can move diagonally inside Palace: D0, F0, D2, F2
        let mut expected_adv = 0u128;
        expected_adv |= 1u128 << 3; // D0
        expected_adv |= 1u128 << 5; // F0
        expected_adv |= 1u128 << (2 * 9 + 3); // D2
        expected_adv |= 1u128 << (2 * 9 + 5); // F2
        assert_eq!(advisor_e1.raw(), expected_adv);
    }

    #[test]
    fn test_bishop_blocking() {
        // Bishop at C0 (Elephant eye at D1)
        let bishop_c0 = BISHOP_TABLE[Square::C0 as usize];
        assert_eq!(bishop_c0.eyes[0], Some(Square::D1));

        // When unblocked (occ_idx = 0), can go to A2 and E2
        let unblocked_attacks = bishop_c0.attacks[0];
        let mut expected = 0u128;
        expected |= 1u128 << (2 * 9); // A2
        expected |= 1u128 << (2 * 9 + 4); // E2
        assert_eq!(unblocked_attacks.raw(), expected);

        // When blocked at D1 (which is index 0 in eyes, so occ_idx = 1), moves to E2 is
        // blocked. C0 to A2 jumps over eye B1, which is index 2. So if only D1
        // is occupied (occ_idx = 1):
        let blocked_attacks = bishop_c0.attacks[1];
        let mut expected_blocked = 0u128;
        expected_blocked |= 1u128 << (2 * 9); // A2 is still valid
        assert_eq!(blocked_attacks.raw(), expected_blocked);
    }
}
