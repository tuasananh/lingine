mod helpers;
mod init;
mod types;

use crate::core::{
    Bitboard, File, Rank, Side, Square,
    movegen::tables::types::{FileEntry, Magic, RankEntry},
};
use strum::EnumCount;

use init::*;
use types::LeaperType;

pub(super) static KING_ATTACKS: [Bitboard; Square::COUNT] = init_king_attacks();
pub(super) static ADVISOR_ATTACKS: [Bitboard; Square::COUNT] = init_advisor_attacks();
pub(super) static PAWN_ATTACKS: [[Bitboard; Square::COUNT]; Side::COUNT] = init_pawn_attacks();
pub(super) static PAWN_ATTACKS_TO: [[Bitboard; Square::COUNT]; 2] = init_pawn_attacks_to();
pub(super) static RANK_TABLE: [RankEntry; File::COUNT] = init_rank_table();
pub(super) static FILE_TABLE: [FileEntry; Rank::COUNT] = init_file_table();
/// Converts a 10-bit file-occupancy mask back to a full [`Bitboard`].
/// Indexed as `FILE_ATTACKS_BY_MASK[file][10-bit mask]`.
pub(super) static FILE_ATTACKS_BY_MASK: [[Bitboard; 1 << Rank::COUNT]; 9] =
    init_file_attacks_by_mask();

// Knight blocking dirs: the four cardinal leg-squares (rank ±1, file ±1 from
// origin).
const KNIGHT_DIRS: ([i8; 4], [i8; 4]) = ([1, -1, 0, 0], [0, 0, 1, -1]);
// Bishop blocking dirs: the four diagonal elbow-squares (±1 on both axes).
const BISHOP_DIRS: ([i8; 4], [i8; 4]) = ([1, 1, -1, -1], [1, -1, 1, -1]);

pub(super) static KNIGHT_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::Knight, KNIGHT_DIRS.0, KNIGHT_DIRS.1);
pub(super) static BISHOP_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::Bishop, BISHOP_DIRS.0, BISHOP_DIRS.1);
/// Backward knight attacks: shares Bishop's elbow-square directions because
/// the leg square for a reversed knight move is always diagonal from the
/// target.
pub(super) static KNIGHT_TO_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::KnightTo, BISHOP_DIRS.0, BISHOP_DIRS.1);

pub(super) static SQUARES_BETWEEN: [[Bitboard; Square::COUNT]; Square::COUNT] =
    init_squares_between();
pub(super) static SQUARES_BEYOND: [[Bitboard; Square::COUNT]; Square::COUNT] =
    init_squares_beyond();
pub(super) static SQUARES_IN_LINE: [[Bitboard; Square::COUNT]; Square::COUNT] =
    init_squares_in_line();
