mod helpers;
mod init;
mod types;

use crate::core::{Bitboard, File, Rank, Side, Square};
use strum::EnumCount;

pub use types::{FileEntry, Magic, RankEntry};

use init::*;
use types::LeaperType;

pub static KING_ATTACKS: [Bitboard; Square::COUNT] = init_king_attacks();
pub static ADVISOR_ATTACKS: [Bitboard; Square::COUNT] = init_advisor_attacks();
pub static PAWN_ATTACKS: [[Bitboard; Square::COUNT]; Side::COUNT] = init_pawn_attacks();
pub static PAWN_ATTACKS_TO: [[Bitboard; Square::COUNT]; 2] = init_pawn_attacks_to();
pub static RANK_TABLE: [RankEntry; File::COUNT] = init_rank_table();
pub static FILE_TABLE: [FileEntry; Rank::COUNT] = init_file_table();
/// Converts a 10-bit file-occupancy mask back to a full [`Bitboard`].
/// Indexed as `FILE_ATTACKS_BY_MASK[file][10-bit mask]`.
pub static FILE_ATTACKS_BY_MASK: [[Bitboard; 1 << Rank::COUNT]; 9] = init_file_attacks_by_mask();

// Knight blocking dirs: the four cardinal leg-squares (rank ±1, file ±1 from
// origin).
const KNIGHT_DIRS: ([i8; 4], [i8; 4]) = ([1, -1, 0, 0], [0, 0, 1, -1]);
// Bishop blocking dirs: the four diagonal elbow-squares (±1 on both axes).
const BISHOP_DIRS: ([i8; 4], [i8; 4]) = ([1, 1, -1, -1], [1, -1, 1, -1]);

pub static KNIGHT_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::Knight, KNIGHT_DIRS.0, KNIGHT_DIRS.1);
pub static BISHOP_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::Bishop, BISHOP_DIRS.0, BISHOP_DIRS.1);
/// Backward knight attacks: shares Bishop's elbow-square directions because
/// the leg square for a reversed knight move is always diagonal from the
/// target.
pub static KNIGHT_TO_MAGICS: [Magic<16>; Square::COUNT] =
    build_magics::<16, 4>(LeaperType::KnightTo, BISHOP_DIRS.0, BISHOP_DIRS.1);

pub static BETWEEN_BB: [[Bitboard; Square::COUNT]; Square::COUNT] = init_between_bb();
pub static RAY_PASS_BB: [[Bitboard; Square::COUNT]; Square::COUNT] = init_ray_pass_bb();
