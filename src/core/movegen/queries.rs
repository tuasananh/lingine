use crate::core::{
    Side,
    bitboard::Bitboard,
    movegen::tables::{
        ADVISOR_ATTACKS, BISHOP_MAGICS, FILE_ATTACKS_BY_MASK, FILE_TABLE, KING_ATTACKS,
        KNIGHT_MAGICS, KNIGHT_TO_MAGICS, PAWN_ATTACKS, PAWN_ATTACKS_TO, RANK_TABLE,
        SQUARES_BETWEEN, SQUARES_BEYOND, SQUARES_IN_LINE,
    },
    types::Square,
};

/// Packers the 10 file bits (every 9th bit starting at `f`) into a contiguous
/// 10-bit integer. Splits across two u64 halves to stay within 64-bit
/// multiply range, using a magic multiplier to compress 5 spaced bits each.
#[inline]
const fn gather_file_bits(bits: u128, f: usize) -> usize {
    let occ = bits >> f;
    const STEP_MASK: u64 = 0x10_0804_0201;
    const MAGIC_MULTIPLIER: u64 = 0x1010101010;
    let low = (occ as u64 & STEP_MASK).wrapping_mul(MAGIC_MULTIPLIER) >> 36;
    let high = ((occ >> 45) as u64 & STEP_MASK).wrapping_mul(MAGIC_MULTIPLIER) >> 36;
    ((low & 0x1F) | ((high & 0x1F) << 5)) as usize
}

/// Returns squares the King can move to from `square` (1-step orthogonal,
/// palace-confined). Does not filter friendly pieces.
#[inline]
pub(crate) fn king_attacks(square: Square) -> Bitboard {
    KING_ATTACKS[square as usize]
}

/// Returns squares the Advisor can move to from `square` (1-step diagonal,
/// palace-confined). Does not filter friendly pieces.
#[inline]
pub(crate) fn advisor_attacks(square: Square) -> Bitboard {
    ADVISOR_ATTACKS[square as usize]
}

/// Returns squares the Bishop can move to from `square` given `occupied`.
/// Blocked if the intervening eye square is occupied.
#[inline]
pub(crate) fn bishop_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    BISHOP_MAGICS[square as usize].attack(occupied)
}

/// Returns squares the Knight can move to from `square` given `occupied`.
/// Blocked if the adjacent leg square in the movement direction is occupied.
#[inline]
pub fn knight_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    KNIGHT_MAGICS[square as usize].attack(occupied)
}

/// Returns squares the Pawn at `square` can move to, given its `side`.
/// Forward-only before crossing the river; adds sideways moves after.
#[inline]
pub(crate) fn pawn_attacks(square: Square, side: Side) -> Bitboard {
    PAWN_ATTACKS[side as usize][square as usize]
}

/// Returns squares from which a Pawn of `side` could attack `square`.
/// The reverse of `pawn_attacks`; used for check detection.
#[inline]
pub(crate) fn pawn_attacks_to(square: Square, side: Side) -> Bitboard {
    PAWN_ATTACKS_TO[side as usize][square as usize]
}

/// Returns squares from which a Knight could attack `square` given `occupied`.
/// The reverse of `knight_attacks`; used for check detection.
#[inline]
pub(crate) fn knight_attacks_to(square: Square, occupied: Bitboard) -> Bitboard {
    KNIGHT_TO_MAGICS[square as usize].attack(occupied)
}

#[inline]
const fn sliding_attacks<const ROOK: bool>(square: Square, occupied: Bitboard) -> Bitboard {
    let idx = square as usize;
    let f = idx % 9;
    let r = idx / 9;
    let rank_occ = ((occupied.raw() >> (r * 9)) & 0x1FF) as usize;
    let file_occ = gather_file_bits(occupied.raw(), f);
    let (rank_mask, file_mask) = if ROOK {
        (
            RANK_TABLE[f].rook_slides[rank_occ],
            FILE_TABLE[r].rook_slides[file_occ],
        )
    } else {
        (
            RANK_TABLE[f].cannon_captures[rank_occ],
            FILE_TABLE[r].cannon_captures[file_occ],
        )
    };
    let rank_bb = unsafe { Bitboard::from_raw((rank_mask as u128) << (r * 9)) };
    rank_bb.const_or(FILE_ATTACKS_BY_MASK[f][file_mask as usize])
}

/// Returns all squares reachable by a Rook from `square` given `occupied`:
/// slides orthogonally in all four directions, stopping on the first blocker
/// (inclusive). Includes both quiet and capture targets; does not filter
/// friendly pieces.
#[inline]
pub const fn rook_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    sliding_attacks::<true>(sq, occ)
}

/// Returns squares the Cannon can capture on from `sq` given `occ`: only
/// squares with exactly one intervening piece (the screen) along a rank or
/// file. Quiet moves are not included — use `rook_attacks` for those.
/// Does not filter friendly pieces.
#[inline]
pub fn cannon_captures(sq: Square, occ: Bitboard) -> Bitboard {
    sliding_attacks::<false>(sq, occ)
}

/// Returns the full cannon attacks ray: all squares behind exactly one screen,
/// up to and including the second piece.
#[inline]
pub(crate) const fn cannon_beyond_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    let idx = square as usize;
    let f = idx % 9;
    let r = idx / 9;
    let rank_occ = ((occupied.raw() >> (r * 9)) & 0x1FF) as usize;
    let file_occ = gather_file_bits(occupied.raw(), f);
    let rank_mask = RANK_TABLE[f].cannon_attack_ray[rank_occ];
    let file_mask = FILE_TABLE[r].cannon_attack_ray[file_occ];
    let rank_bb = unsafe { Bitboard::from_raw((rank_mask as u128) << (r * 9)) };
    rank_bb.const_or(FILE_ATTACKS_BY_MASK[f][file_mask as usize])
}

/// Returns a bitboard containing the squares strictly between `from` and `to`
/// on the same rank or file, plus `to` itself. For a knight move, includes
/// `to` plus the leg square.
#[inline]
pub(crate) fn squares_between(from: Square, to: Square) -> Bitboard {
    SQUARES_BETWEEN[from as usize][to as usize]
}

/// Returns a bitboard containing the squares from `to` extending away from
/// `from` to the edge of the board, along the same rank or file.
#[inline]
pub(crate) fn squares_beyond(from: Square, to: Square) -> Bitboard {
    SQUARES_BEYOND[from as usize][to as usize]
}

/// Returns a bitboard containing the squares that `from` and `to`
/// make a line
#[inline]
pub(crate) fn squares_in_line(from: Square, to: Square) -> Bitboard {
    SQUARES_IN_LINE[from as usize][to as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let empty = Bitboard::new();
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
        let blocked = unsafe { Bitboard::from_raw((1u128 << (2 * 9)) | (1u128 << 1)) };
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
    fn test_cannon_attack_ray() {
        // Cannon quiet attacks are 0 on an empty board (cannon_captures only computes
        // leap captures)
        let empty = Bitboard::new();
        let att_cannon_a0 = cannon_captures(Square::A0, empty);
        assert_eq!(att_cannon_a0.raw(), 0);

        // Cannon at A0, blocker (screen) at A2, and enemy at A5
        let occupied = unsafe { Bitboard::from_raw((1u128 << (2 * 9)) | (1u128 << (5 * 9))) };
        let att_cannon_blocked = cannon_captures(Square::A0, occupied);
        assert_eq!(att_cannon_blocked.raw(), 1u128 << (5 * 9));

        // Test the full cannon attack ray (behind exactly one screen)
        let ray = cannon_beyond_attacks(Square::A0, occupied);
        let mut expected_ray = 0u128;
        expected_ray |= 1u128 << (3 * 9); // A3
        expected_ray |= 1u128 << (4 * 9); // A4
        expected_ray |= 1u128 << (5 * 9); // A5 (up to and including the target)
        assert_eq!(ray.raw(), expected_ray);

        // General can move orthogonally inside Palace: D0, F0, E1
        let mut expected = 0u128;
        expected |= 1u128 << 3; // D0
        expected |= 1u128 << 5; // F0
        expected |= 1u128 << (9 + 4); // E1
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
        let unblocked_attacks = bishop_attacks(Square::C0, Bitboard::new());
        println!("{}", unblocked_attacks);
        let mut expected = 0u128;
        expected |= 1u128 << (2 * 9); // A2
        expected |= 1u128 << (2 * 9 + 4); // E2
        assert_eq!(unblocked_attacks.raw(), expected);

        // When blocked at D1 (which is index 0 in eyes, so occ_idx = 1), moves to E2 is
        // blocked. C0 to A2 jumps over eye B1, which is index 2. So if only D1
        // is occupied (occ_idx = 1):
        let blocked_attacks = bishop_attacks(Square::C0, Square::D1.into());
        println!("{}", blocked_attacks);
        let mut expected_blocked = 0u128;
        expected_blocked |= 1u128 << (2 * 9); // A2 is still valid
        assert_eq!(blocked_attacks.raw(), expected_blocked);
    }

    #[test]
    fn test_squares_between_and_beyond() {
        // Rook/Cannon path between A0 and A3
        let between_a0_a3 = squares_between(Square::A0, Square::A3);
        let mut expected_between = Bitboard::new();
        expected_between.set_bit(Square::A1);
        expected_between.set_bit(Square::A2);
        expected_between.set_bit(Square::A3);
        assert_eq!(between_a0_a3, expected_between);

        // Knight path: King at D1 (sq1) and Knight at F2 (sq2)
        // Leg should be E2
        let between_d1_f2 = squares_between(Square::D1, Square::F2);
        let mut expected_knight_leg = Bitboard::new();
        expected_knight_leg.set_bit(Square::E2);
        expected_knight_leg.set_bit(Square::F2);
        assert_eq!(between_d1_f2, expected_knight_leg);

        // squares_beyond: from A0 to A3, should extend from A3 away to the edge
        // (A4..A9)
        let beyond_a0_a3 = squares_beyond(Square::A0, Square::A3);
        let mut expected_beyond = Bitboard::new();
        expected_beyond.set_bit(Square::A3);
        expected_beyond.set_bit(Square::A4);
        expected_beyond.set_bit(Square::A5);
        expected_beyond.set_bit(Square::A6);
        expected_beyond.set_bit(Square::A7);
        expected_beyond.set_bit(Square::A8);
        expected_beyond.set_bit(Square::A9);
        assert_eq!(beyond_a0_a3, expected_beyond);
    }
}
