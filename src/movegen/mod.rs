mod attacks;
mod tables;

// Re-export public API so external callers (position.rs, etc.) keep their existing `use crate::movegen::*` paths.
#[allow(unused_imports)]
pub use attacks::{cannon_attacks, gather_file_bits, rook_attacks};
#[allow(unused_imports)]
pub use tables::{
    FILE_TABLE, FileEntry, KNIGHT_TO_TABLE, KnightToEntry, PAWN_ATTACKS, PAWN_ATTACKS_TO,
    RANK_TABLE, RankEntry,
};

use crate::{
    bitboard::Bitboard,
    position::Position,
    types::{Color, MAX_MOVES, Move, MoveGenType, PieceType, Square},
};

use tables::{ADVISOR_ATTACKS, BISHOP_TABLE, KING_ATTACKS, KNIGHT_TABLE};

/// Generates valid orthogonal moves for the General (King) inside the Palace.
fn generate_king_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    if let Some(from_sq) = pos.king_square(us) {
        let us_pieces = pos.bitboard_by_color(us);
        let mut target_bb = Bitboard(KING_ATTACKS[from_sq as usize].0 & !us_pieces.0);
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates diagonal moves for Advisors inside the Palace.
fn generate_advisor_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let mut advisors = pos.bitboard_by_type(PieceType::Advisor) & us_pieces;
    while let Some(from_sq) = advisors.pop_lsb() {
        let mut target_bb = Bitboard(ADVISOR_ATTACKS[from_sq as usize].0 & !us_pieces.0);
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates diagonal moves for Elephants (Bishops), checking diagonal blocker intermediate eyes.
fn generate_bishop_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let occupied = pos.bitboard_by_color(Color::White) | pos.bitboard_by_color(Color::Black);
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

        let mut target_bb = Bitboard(entry.attacks[occ_idx].0 & !us_pieces.0);
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates L-shaped moves for Horses (Knights), checking intermediate orthogonal blocker leg squares.
fn generate_knight_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let occupied = pos.bitboard_by_color(Color::White) | pos.bitboard_by_color(Color::Black);
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

        let mut target_bb = Bitboard(entry.attacks[occ_idx].0 & !us_pieces.0);
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates moves for Soldiers (Pawns) based on whether they have crossed the river or not.
fn generate_pawn_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let mut pawns = pos.bitboard_by_type(PieceType::Pawn) & us_pieces;
    let color_idx = if IS_WHITE { 0 } else { 1 };

    while let Some(from_sq) = pawns.pop_lsb() {
        let mut target_bb = Bitboard(PAWN_ATTACKS[color_idx][from_sq as usize].0 & !us_pieces.0);
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates horizontal and vertical sliding moves for Chariots (Rooks) in O(1) lookups.
fn generate_rook_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let occupied = pos.bitboard_by_color(Color::White) | pos.bitboard_by_color(Color::Black);
    let mut rooks = pos.bitboard_by_type(PieceType::Rook) & us_pieces;

    while let Some(from_sq) = rooks.pop_lsb() {
        let from_idx = from_sq as usize;
        let f = from_idx % 9;
        let r = from_idx / 9;

        // 1. Rank attacks
        let rank_occ = ((occupied.0 >> (r * 9)) & 0x1FF) as usize;
        let us_rank_mask = ((us_pieces.0 >> (r * 9)) & 0x1FF) as u16;
        let mut rank_attack_mask = RANK_TABLE[f].rook[rank_occ] & !us_rank_mask;

        while rank_attack_mask != 0 {
            let f_to = rank_attack_mask.trailing_zeros() as usize;
            rank_attack_mask &= rank_attack_mask - 1;
            let to_sq = Square::from_repr((r * 9 + f_to) as u8).unwrap();
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }

        // 2. File attacks
        let file_occ = gather_file_bits(occupied.0, f);
        let us_file_mask = gather_file_bits(us_pieces.0, f) as u16;
        let mut file_attack_mask = FILE_TABLE[r].rook[file_occ] & !us_file_mask;

        while file_attack_mask != 0 {
            let r_to = file_attack_mask.trailing_zeros() as usize;
            file_attack_mask &= file_attack_mask - 1;
            let to_sq = Square::from_repr((r_to * 9 + f) as u8).unwrap();
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates horizontal and vertical moves/leap captures for Cannons in O(1) lookups.
fn generate_cannon_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let them = us.opposite();
    let us_pieces = pos.bitboard_by_color(us);
    let them_pieces = pos.bitboard_by_color(them);
    let occupied = pos.bitboard_by_color(Color::White) | pos.bitboard_by_color(Color::Black);
    let mut cannons = pos.bitboard_by_type(PieceType::Cannon) & us_pieces;

    while let Some(from_sq) = cannons.pop_lsb() {
        let from_idx = from_sq as usize;
        let f = from_idx % 9;
        let r = from_idx / 9;

        // 1. Rank moves (horizontal quiet + leap captures)
        let rank_occ = ((occupied.0 >> (r * 9)) & 0x1FF) as usize;
        let them_rank_mask = ((them_pieces.0 >> (r * 9)) & 0x1FF) as u16;

        let rank_quiet_mask =
            RANK_TABLE[f].rook[rank_occ] & !((occupied.0 >> (r * 9)) & 0x1FF) as u16;
        let rank_capture_mask = RANK_TABLE[f].cannon[rank_occ] & them_rank_mask;

        let mut rank_attack_mask = rank_quiet_mask | rank_capture_mask;
        while rank_attack_mask != 0 {
            let f_to = rank_attack_mask.trailing_zeros() as usize;
            rank_attack_mask &= rank_attack_mask - 1;
            let to_sq = Square::from_repr((r * 9 + f_to) as u8).unwrap();
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }

        // 2. File moves (vertical quiet + leap captures)
        let file_occ = gather_file_bits(occupied.0, f);
        let them_file_mask = gather_file_bits(them_pieces.0, f) as u16;
        let occ_file_mask = gather_file_bits(occupied.0, f) as u16;

        let file_quiet_mask = FILE_TABLE[r].rook[file_occ] & !occ_file_mask;
        let file_capture_mask = FILE_TABLE[r].cannon[file_occ] & them_file_mask;

        let mut file_attack_mask = file_quiet_mask | file_capture_mask;
        while file_attack_mask != 0 {
            let r_to = file_attack_mask.trailing_zeros() as usize;
            file_attack_mask &= file_attack_mask - 1;
            let to_sq = Square::from_repr((r_to * 9 + f) as u8).unwrap();
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Orchestrates move generators for all piece types, returning the total pseudo-legal move count.
fn generate_pseudo_legal<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
) -> usize {
    let mut count = 0;
    generate_king_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_advisor_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_bishop_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_knight_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_pawn_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_rook_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_cannon_moves::<IS_WHITE>(pos, moves, &mut count);
    count
}

/// The main entry point for move generation.
/// Filters pseudo-legal moves into legal moves (e.g. by ensuring the King is not left in check)
/// and respects the target `MoveGenType` request (Legal, PseudoLegal, Quiets, Captures, Evasions).
pub fn generate_moves(
    pos: &Position,
    gen_type: MoveGenType,
    moves: &mut [Move; MAX_MOVES],
) -> usize {
    let color = pos.side_to_move();
    let mut count = match color {
        Color::White => generate_pseudo_legal::<true>(pos, moves),
        Color::Black => generate_pseudo_legal::<false>(pos, moves),
    };

    if gen_type == MoveGenType::PseudoLegal {
        return count;
    }

    let mut cur = 0;
    while cur < count {
        let m = moves[cur];
        let is_legal = pos.legal(m);
        let keep = if is_legal {
            match gen_type {
                MoveGenType::Legal | MoveGenType::Evasions => true,
                MoveGenType::Captures => !pos.is_empty(m.square_to()),
                MoveGenType::Quiets => pos.is_empty(m.square_to()),
                _ => false,
            }
        } else {
            false
        };

        if !keep {
            count -= 1;
            moves[cur] = moves[count]; // Swap with the last move in the list
        } else {
            cur += 1;
        }
    }

    count
}
