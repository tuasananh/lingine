//! High-performance move generator for all 7 Xiangqi piece types.
//!
//! This module orchestrates move generation for:
//! 1. **General (King)**: Moves 1 step orthogonally within the Palace.
//! 2. **Advisor (Shi)**: Moves 1 step diagonally within the Palace.
//! 3. **Elephant (Bishop)**: Moves 2 steps diagonally, blocked by intermediate
//!    diagonal Elephant Eyes. Cannot cross the river.
//! 4. **Horse (Knight)**: Moves L-shape (1 orthogonal, 1 diagonal), blocked by
//!    intermediate orthogonal Horse Legs.
//! 5. **Soldier (Pawn)**: Moves 1 step forward. After crossing the river, can
//!    also move sideways.
//! 6. **Chariot (Rook)**: Slides orthogonally, blocked by the first piece.
//! 7. **Cannon**: Slides orthogonally, must jump exactly one piece (hurdle) to
//!    capture.
//!
//! Features dynamic filtering of pseudo-legal moves into fully legal moves by
//! verifying that execution does not leave the friendly King in check
//! (incorporating the Flying General rule).

mod attacks;
mod tables;

use arrayvec::ArrayVec;
// Re-export public API so external callers (position.rs, etc.) keep their
// existing `use crate::movegen::*` paths.
pub use attacks::{cannon_attacks, gather_file_bits, rook_attacks};
pub use tables::{
    ADVISOR_ATTACKS, BISHOP_TABLE, BishopEntry, FILE_TABLE, FileEntry, KING_ATTACKS, KNIGHT_TABLE,
    KNIGHT_TO_TABLE, KnightEntry, KnightToEntry, PAWN_ATTACKS, PAWN_ATTACKS_TO, RANK_TABLE,
    RankEntry,
};

use crate::core::{Move, PieceType, Position, Side, Square};

/// The maximum number of pseudo-legal moves in any given Xiangqi position
/// (typically <= 120).
pub const MAX_MOVES: usize = 128;

/// A stack-allocated array vector that holds up to `MAX_MOVES` without heap
/// allocation.
pub type MoveList = ArrayVec<Move, MAX_MOVES>;

/// Types of moves that can be requested during move generation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveGenType {
    Captures,
    Quiets,
    Evasions,
    PseudoLegal,
    Legal,
}

/// Generates valid orthogonal moves for the General (King) inside the Palace.
fn generate_king_moves<const IS_WHITE: bool>(pos: &Position, moves: &mut MoveList) {
    let us = if IS_WHITE { Side::Red } else { Side::Black };
    let from_sq = pos.king_square(us);
    let us_pieces = pos.bitboard_by_color(us);
    let mut target_bb = KING_ATTACKS[from_sq as usize] & !us_pieces;
    while let Some(to_sq) = target_bb.pop_lsb() {
        moves.push(Move::new(from_sq, to_sq));
    }
}

/// Generates diagonal moves for Advisors inside the Palace.
fn generate_advisor_moves<const IS_WHITE: bool>(pos: &Position, moves: &mut MoveList) {
    let us = if IS_WHITE { Side::Red } else { Side::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let mut advisors = pos.bitboard_by_type(PieceType::Advisor) & us_pieces;
    while let Some(from_sq) = advisors.pop_lsb() {
        let mut target_bb = ADVISOR_ATTACKS[from_sq as usize] & !us_pieces;
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves.push(Move::new(from_sq, to_sq));
        }
    }
}

/// Generates diagonal moves for Elephants (Bishops), checking diagonal blocker
/// intermediate eyes.
fn generate_bishop_moves<const IS_WHITE: bool>(pos: &Position, moves: &mut MoveList) {
    let us = if IS_WHITE { Side::Red } else { Side::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let occupied = pos.bitboard_by_color(Side::Red) | pos.bitboard_by_color(Side::Black);
    let mut bishops = pos.bitboard_by_type(PieceType::Bishop) & us_pieces;

    while let Some(from_sq) = bishops.pop_lsb() {
        let entry = &BISHOP_TABLE[from_sq as usize];
        let mut occ_idx = 0;

        let mut i = 0;
        while i < 4 {
            if let Some(eye_sq) = entry.eyes[i]
                && occupied.is_occupied(eye_sq)
            {
                occ_idx |= 1 << i;
            }
            i += 1;
        }

        let mut target_bb = entry.attacks[occ_idx] & !us_pieces;
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves.push(Move::new(from_sq, to_sq));
        }
    }
}

/// Generates L-shaped moves for Horses (Knights), checking intermediate
/// orthogonal blocker leg squares.
fn generate_knight_moves<const IS_WHITE: bool>(pos: &Position, moves: &mut MoveList) {
    let us = if IS_WHITE { Side::Red } else { Side::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let occupied = pos.bitboard_by_color(Side::Red) | pos.bitboard_by_color(Side::Black);
    let mut knights = pos.bitboard_by_type(PieceType::Knight) & us_pieces;

    while let Some(from_sq) = knights.pop_lsb() {
        let entry = &KNIGHT_TABLE[from_sq as usize];
        let mut occ_idx = 0;

        let mut i = 0;
        while i < 4 {
            if let Some(eye_sq) = entry.eyes[i]
                && occupied.is_occupied(eye_sq)
            {
                occ_idx |= 1 << i;
            }
            i += 1;
        }

        let mut target_bb = entry.attacks[occ_idx] & !us_pieces;
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves.push(Move::new(from_sq, to_sq));
        }
    }
}

/// Generates moves for Soldiers (Pawns) based on whether they have crossed the
/// river or not.
fn generate_pawn_moves<const IS_WHITE: bool>(pos: &Position, moves: &mut MoveList) {
    let us = if IS_WHITE { Side::Red } else { Side::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let mut pawns = pos.bitboard_by_type(PieceType::Pawn) & us_pieces;
    let color_idx = if IS_WHITE { 0 } else { 1 };

    while let Some(from_sq) = pawns.pop_lsb() {
        let mut target_bb = PAWN_ATTACKS[color_idx][from_sq as usize] & !us_pieces;
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves.push(Move::new(from_sq, to_sq));
        }
    }
}

/// Generates horizontal and vertical sliding moves for Chariots (Rooks) in O(1)
/// lookups.
fn generate_rook_moves<const IS_WHITE: bool>(pos: &Position, moves: &mut MoveList) {
    let us = if IS_WHITE { Side::Red } else { Side::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let occupied = pos.bitboard_by_color(Side::Red) | pos.bitboard_by_color(Side::Black);
    let mut rooks = pos.bitboard_by_type(PieceType::Rook) & us_pieces;

    while let Some(from_sq) = rooks.pop_lsb() {
        let from_idx = from_sq as usize;
        let f = from_idx % 9;
        let r = from_idx / 9;

        // 1. Rank attacks
        let rank_occ = ((occupied.raw() >> (r * 9)) & 0x1FF) as usize;
        let us_rank_mask = ((us_pieces.raw() >> (r * 9)) & 0x1FF) as u16;
        let mut rank_attack_mask = RANK_TABLE[f].rook[rank_occ] & !us_rank_mask;

        while rank_attack_mask != 0 {
            let f_to = rank_attack_mask.trailing_zeros() as usize;
            rank_attack_mask &= rank_attack_mask - 1;
            let to_sq = Square::from_repr((r * 9 + f_to) as u8).unwrap();
            moves.push(Move::new(from_sq, to_sq));
        }

        // 2. File attacks
        let file_occ = gather_file_bits(occupied.raw(), f);
        let us_file_mask = gather_file_bits(us_pieces.raw(), f) as u16;
        let mut file_attack_mask = FILE_TABLE[r].rook[file_occ] & !us_file_mask;

        while file_attack_mask != 0 {
            let r_to = file_attack_mask.trailing_zeros() as usize;
            file_attack_mask &= file_attack_mask - 1;
            let to_sq = Square::from_repr((r_to * 9 + f) as u8).unwrap();
            moves.push(Move::new(from_sq, to_sq));
        }
    }
}

/// Generates horizontal and vertical moves/leap captures for Cannons in O(1)
/// lookups.
fn generate_cannon_moves<const IS_WHITE: bool>(pos: &Position, moves: &mut MoveList) {
    let us = if IS_WHITE { Side::Red } else { Side::Black };
    let them = us.opposite();
    let us_pieces = pos.bitboard_by_color(us);
    let them_pieces = pos.bitboard_by_color(them);
    let occupied = pos.bitboard_by_color(Side::Red) | pos.bitboard_by_color(Side::Black);
    let mut cannons = pos.bitboard_by_type(PieceType::Cannon) & us_pieces;

    while let Some(from_sq) = cannons.pop_lsb() {
        let from_idx = from_sq as usize;
        let f = from_idx % 9;
        let r = from_idx / 9;

        // 1. Rank moves (horizontal quiet + leap captures)
        let rank_occ = ((occupied.raw() >> (r * 9)) & 0x1FF) as usize;
        let them_rank_mask = ((them_pieces.raw() >> (r * 9)) & 0x1FF) as u16;

        let rank_quiet_mask =
            RANK_TABLE[f].rook[rank_occ] & !((occupied.raw() >> (r * 9)) & 0x1FF) as u16;
        let rank_capture_mask = RANK_TABLE[f].cannon[rank_occ] & them_rank_mask;

        let mut rank_attack_mask = rank_quiet_mask | rank_capture_mask;
        while rank_attack_mask != 0 {
            let f_to = rank_attack_mask.trailing_zeros() as usize;
            rank_attack_mask &= rank_attack_mask - 1;
            let to_sq = Square::from_repr((r * 9 + f_to) as u8).unwrap();
            moves.push(Move::new(from_sq, to_sq));
        }

        // 2. File moves (vertical quiet + leap captures)
        let file_occ = gather_file_bits(occupied.raw(), f);
        let them_file_mask = gather_file_bits(them_pieces.raw(), f) as u16;
        let occ_file_mask = gather_file_bits(occupied.raw(), f) as u16;

        let file_quiet_mask = FILE_TABLE[r].rook[file_occ] & !occ_file_mask;
        let file_capture_mask = FILE_TABLE[r].cannon[file_occ] & them_file_mask;

        let mut file_attack_mask = file_quiet_mask | file_capture_mask;
        while file_attack_mask != 0 {
            let r_to = file_attack_mask.trailing_zeros() as usize;
            file_attack_mask &= file_attack_mask - 1;
            let to_sq = Square::from_repr((r_to * 9 + f) as u8).unwrap();
            moves.push(Move::new(from_sq, to_sq));
        }
    }
}

/// Orchestrates move generators for all piece types, returning the total
/// pseudo-legal move count.
fn generate_pseudo_legal<const IS_WHITE: bool>(pos: &Position, moves: &mut MoveList) {
    generate_king_moves::<IS_WHITE>(pos, moves);
    generate_advisor_moves::<IS_WHITE>(pos, moves);
    generate_bishop_moves::<IS_WHITE>(pos, moves);
    generate_knight_moves::<IS_WHITE>(pos, moves);
    generate_pawn_moves::<IS_WHITE>(pos, moves);
    generate_rook_moves::<IS_WHITE>(pos, moves);
    generate_cannon_moves::<IS_WHITE>(pos, moves);
}

/// The main entry point for move generation.
/// Filters pseudo-legal moves into legal moves (e.g. by ensuring the King is
/// not left in check) and respects the target `MoveGenType` request (Legal,
/// PseudoLegal, Quiets, Captures, Evasions).
pub fn generate_moves(pos: &Position, gen_type: MoveGenType, moves: &mut MoveList) {
    let color = pos.side_to_move();
    match color {
        Side::Red => generate_pseudo_legal::<true>(pos, moves),
        Side::Black => generate_pseudo_legal::<false>(pos, moves),
    };

    if gen_type == MoveGenType::PseudoLegal {
        return;
    }

    // Filter legal moves depending on the generation type
    let mut write_idx = 0;
    for read_idx in 0..moves.len() {
        let m = moves[read_idx];
        let is_legal = pos.legal(m);
        let keep = if is_legal {
            match gen_type {
                MoveGenType::Legal | MoveGenType::Evasions => true,
                MoveGenType::Captures => !pos.is_empty(m.to()),
                MoveGenType::Quiets => pos.is_empty(m.to()),
                _ => false,
            }
        } else {
            false
        };

        if keep {
            moves[write_idx] = m;
            write_idx += 1
        }
    }

    moves.truncate(write_idx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::position::Position;
    use crate::core::types::{Move, Square};

    fn count_moves(pos: &Position, gen_type: MoveGenType) -> usize {
        let mut moves = MoveList::new();
        generate_moves(pos, gen_type, &mut moves);
        moves.len()
    }

    fn has_move(pos: &Position, gen_type: MoveGenType, m: Move) -> bool {
        let mut moves = MoveList::new();
        generate_moves(pos, gen_type, &mut moves);
        moves.contains(&m)
    }

    #[test]
    fn test_king_moves() {
        let mut pos = Position::new();
        // Setup King in the middle of palace (E0)
        pos.set("9/9/9/9/9/9/9/9/9/4K4 w - - 0 1").unwrap();
        let count = count_moves(&pos, MoveGenType::Legal);
        // E0 has 4 directions: D0, F0, E1 (E-1 is offboard, so 3 legal moves)
        assert_eq!(count, 3);
        assert!(has_move(
            &pos,
            MoveGenType::Legal,
            Move::new(Square::E0, Square::D0)
        ));
        assert!(has_move(
            &pos,
            MoveGenType::Legal,
            Move::new(Square::E0, Square::F0)
        ));
        assert!(has_move(
            &pos,
            MoveGenType::Legal,
            Move::new(Square::E0, Square::E1)
        ));

        // Block with a friendly piece at E1
        pos.set("9/9/9/9/9/9/9/9/4A4/4K4 w - - 0 1").unwrap();
        // Advisor is on E1. Advisor moves: FD0, FF0.
        // Let's generate moves for E0
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let king_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::E0)
            .copied()
            .collect();
        assert_eq!(king_moves.len(), 2);
        assert!(king_moves.contains(&Move::new(Square::E0, Square::D0)));
        assert!(king_moves.contains(&Move::new(Square::E0, Square::F0)));
    }

    #[test]
    fn test_advisor_moves() {
        let mut pos = Position::new();
        // Setup Advisor at center of Palace (E1)
        pos.set("9/9/9/9/9/9/9/9/4A4/4K4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let adv_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::E1)
            .copied()
            .collect();
        // E1 has 4 corners: D0, F0, D2, F2
        assert_eq!(adv_moves.len(), 4);
        assert!(adv_moves.contains(&Move::new(Square::E1, Square::D0)));
        assert!(adv_moves.contains(&Move::new(Square::E1, Square::F0)));
        assert!(adv_moves.contains(&Move::new(Square::E1, Square::D2)));
        assert!(adv_moves.contains(&Move::new(Square::E1, Square::F2)));

        // Corner Advisor (D0)
        pos.set("9/9/9/9/9/9/9/9/9/3AK4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let adv_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::D0)
            .copied()
            .collect();
        // D0 only has 1 diagonal square: E1
        assert_eq!(adv_moves.len(), 1);
        assert!(adv_moves.contains(&Move::new(Square::D0, Square::E1)));
    }

    #[test]
    fn test_bishop_moves() {
        let mut pos = Position::new();
        // Bishop (Elephant) at C0
        pos.set("9/9/9/9/9/9/9/9/9/2B1K4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let bish_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::C0)
            .copied()
            .collect();
        // C0 can go to A2, E2. (Can't cross river or go off board)
        assert_eq!(bish_moves.len(), 2);
        assert!(bish_moves.contains(&Move::new(Square::C0, Square::A2)));
        assert!(bish_moves.contains(&Move::new(Square::C0, Square::E2)));

        // Block with a piece on the Elephant eye (D1)
        pos.set("9/9/9/9/9/9/9/9/3p5/2B1K4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let bish_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::C0)
            .copied()
            .collect();
        // D1 eye blocked, can only go to A2
        assert_eq!(bish_moves.len(), 1);
        assert!(bish_moves.contains(&Move::new(Square::C0, Square::A2)));
    }

    #[test]
    fn test_knight_moves() {
        let mut pos = Position::new();
        // Knight at E2
        pos.set("9/9/9/9/9/9/9/4H4/9/4K4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let kn_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::E2)
            .copied()
            .collect();
        // Knight at E2 should have 8 moves on empty board
        assert_eq!(kn_moves.len(), 8);

        // Block with a piece on the Horse Leg (E3)
        pos.set("9/9/9/9/9/9/4p4/4H4/9/4K4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let kn_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::E2)
            .copied()
            .collect();
        // E3 leg blocked, so moves to D4 and F4 are blocked. 8 - 2 = 6 moves left.
        assert_eq!(kn_moves.len(), 6);
        assert!(!kn_moves.contains(&Move::new(Square::E2, Square::D4)));
        assert!(!kn_moves.contains(&Move::new(Square::E2, Square::F4)));
    }

    #[test]
    fn test_pawn_moves() {
        let mut pos = Position::new();
        // Unpromoted Pawn (C3)
        pos.set("9/9/9/9/9/9/2P6/9/9/4K4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let pawn_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::C3)
            .copied()
            .collect();
        // Only 1 step forward (C4)
        assert_eq!(pawn_moves.len(), 1);
        assert!(pawn_moves.contains(&Move::new(Square::C3, Square::C4)));

        // Promoted Pawn (C6 - crossed river)
        pos.set("9/9/9/2P6/9/9/9/9/9/4K4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let pawn_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::C6)
            .copied()
            .collect();
        // Forward (C7), Left (B6), Right (D6)
        assert_eq!(pawn_moves.len(), 3);
        assert!(pawn_moves.contains(&Move::new(Square::C6, Square::C7)));
        assert!(pawn_moves.contains(&Move::new(Square::C6, Square::B6)));
        assert!(pawn_moves.contains(&Move::new(Square::C6, Square::D6)));
    }

    #[test]
    fn test_rook_moves() {
        let mut pos = Position::new();
        // Rook at E5
        pos.set("9/9/9/9/4R4/9/9/9/9/4K4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let r_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::E5)
            .copied()
            .collect();
        // Rank 5 has 8 other squares, File E has 9 other squares. Minus King. Total 16 moves.
        assert_eq!(r_moves.len(), 16);

        // Friendly blocker at E6, opponent at E3
        pos.set("9/9/9/4A4/4R4/9/4p4/9/9/4K4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let r_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::E5)
            .copied()
            .collect();
        // Upwards blocked at E6 (so no E6, E7, E8, E9).
        // Downwards blocked by opponent at E3 (so E4 is quiet, E3 is capture, but no
        // E2, E1, E0). Plus 8 horizontal moves.
        // File moves allowed: E4, E3. (2 moves)
        // Rank moves: A5, B5, C5, D5, F5, G5, H5, I5 (8 moves)
        // Total: 10 moves.
        assert_eq!(r_moves.len(), 10);
        assert!(r_moves.contains(&Move::new(Square::E5, Square::E3))); // capture
        assert!(!r_moves.contains(&Move::new(Square::E5, Square::E6))); // friendly block
    }

    #[test]
    fn test_cannon_moves() {
        let mut pos = Position::new();
        // Cannon at E5, empty board.
        pos.set("9/9/9/9/4C4/9/9/9/9/4K4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let c_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::E5)
            .copied()
            .collect();
        // Quiet moves only (behaves like Rook) = 17 - 1 (King) = 16 moves.
        assert_eq!(c_moves.len(), 16);

        // Friendly screen at E6, opponent behind at E8
        pos.set("9/4r4/9/4A4/4C4/9/9/9/9/4K4 w - - 0 1").unwrap();
        pos.print_board();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let c_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::E5)
            .copied()
            .collect();
        // Upwards quiet moves: none (E6 blocked).
        // Leap capture: E8 (opponent) using E6 (friendly) as hurdle!
        // Downwards: E4, E3, E2, E1, E0. (5 moves)
        // Rank: 8 moves.
        // Total: 5 - 1 (King) (file quiet) + 1 (file capture) + 8 (rank quiet) = 14 moves.
        assert_eq!(c_moves.len(), 13);
        assert!(c_moves.contains(&Move::new(Square::E5, Square::E8))); // leap capture
    }

    #[test]
    fn test_facing_kings() {
        let mut pos = Position::new();
        // Kings facing each other on file E with only 1 friendly Rook in between (E3)
        pos.set("4k4/9/9/9/9/4R4/9/9/9/4K4 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        generate_moves(&pos, MoveGenType::Legal, &mut moves);
        let r_moves: Vec<Move> = moves
            .iter()
            .filter(|m| m.from() == Square::E3)
            .copied()
            .collect();
        // Moving the Rook horizontally (off file E) is illegal because it exposes the
        // Kings to face each other! So the Rook can only move vertically along
        // file E (D3, F3, etc. are illegal).
        for m in r_moves {
            assert_eq!(m.to() as u8 % 9, 4); // must stay on file E (index 4)
        }
    }

    fn assert_positions_equal(a: &Position, b: &Position) {
        use crate::core::types::{Piece, PieceType, Side};
        use strum::IntoEnumIterator;
        assert_eq!(a.side_to_move(), b.side_to_move());
        assert_eq!(a.zobrist_hash(), b.zobrist_hash());
        for sq_val in 0..90 {
            let sq = Square::from_repr(sq_val).unwrap();
            assert_eq!(a.piece_at(sq), b.piece_at(sq));
            assert_eq!(a.is_empty(sq), b.is_empty(sq));
        }
        for color in [Side::Red, Side::Black] {
            assert_eq!(a.king_square(color), b.king_square(color));
            assert_eq!(a.is_in_check(color), b.is_in_check(color));
            assert_eq!(a.bitboard_by_color(color), b.bitboard_by_color(color));
        }
        for pt in PieceType::iter() {
            assert_eq!(a.bitboard_by_type(pt), b.bitboard_by_type(pt));
        }
        for piece in Piece::iter() {
            assert_eq!(a.piece_count(piece), b.piece_count(piece));
        }
        assert_eq!(a.rule_judge(0), b.rule_judge(0));
    }

    #[test]
    fn test_position_restored_after_move_generation() {
        // Test with a few different FEN configurations:
        let fens = [
            // 1. Initial starting position
            "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1",
            // 2. Middle game position with complex checks, pins, and attacks
            "3akab2/9/2n1b4/2p1p1p1C/2R3r2/6P2/P1P1P3P/1C2c1N11/9/RN2KAB1r b - - 0 12",
            // 3. Simple position with facing kings and threat of check
            "4k4/9/9/9/9/4R4/9/9/9/4K4 w - - 0 1",
            // 4. Position with multiple legal and illegal moves
            "4k4/9/9/9/9/9/9/5h3/5A3/4K4 w - - 0 1",
        ];

        let gen_types = [
            MoveGenType::Legal,
            MoveGenType::PseudoLegal,
            MoveGenType::Quiets,
            MoveGenType::Captures,
            MoveGenType::Evasions,
        ];

        for fen in fens {
            let mut pos = Position::new();
            pos.set(fen).unwrap();

            for gen_type in gen_types {
                let pos_before = pos.clone();

                let mut moves = MoveList::new();
                generate_moves(&pos, gen_type, &mut moves);

                // Ensure calling generate_moves did not modify/corrupt the position in any way
                assert_positions_equal(&pos, &pos_before);
            }
        }
    }
}
