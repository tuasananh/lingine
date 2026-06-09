use crate::search::MAX_PLY;

pub type Score = i32;

pub const ZERO: Score = 0;
pub const DRAW: Score = 0;
pub const MATE: Score = 32_000;
pub const INFINITY: Score = 32_001;
pub const MATE_IN_MAX_PLY: Score = MATE - MAX_PLY as Score;
pub const MATED_IN_MAX_PLY: Score = -MATE + MAX_PLY as Score;

/// Checks whether the score is winning (mate in some plies)
#[inline]
pub const fn is_winning(value: Score) -> bool {
    value >= MATE_IN_MAX_PLY
}

/// Checks whether the score is losing (mated mate in some plies)
#[inline]
pub const fn is_losing(value: Score) -> bool {
    value <= MATED_IN_MAX_PLY
}

/// Get a score that is ply independent, useful for
/// [`crate::search::TranspositionTable::store`]
#[inline]
pub const fn ply_independent(value: Score, ply: u8) -> Score {
    if is_winning(value) {
        value + ply as Score
    } else if is_losing(value) {
        value - ply as Score
    } else {
        value
    }
}

/// Get a score that is ply independent, useful for
/// [`crate::search::TranspositionTable::probe`]
#[inline]
pub const fn ply_dependent(value: Score, ply: u8) -> Score {
    if is_winning(value) {
        value - ply as Score
    } else if is_losing(value) {
        value + ply as Score
    } else {
        value
    }
}

/// Gets the number of ply until we have a mate or get mated
#[inline]
pub const fn ply_to_mate_or_mated(value: Score) -> Option<u8> {
    if is_winning(value) {
        Some((MATE - value) as u8)
    } else if is_losing(value) {
        Some((value + MATE) as u8)
    } else {
        None
    }
}

/// Value for mate in some ply
#[inline]
pub const fn mate_in(ply: u8) -> Score {
    MATE - ply as Score
}

/// Value for mated in some ply
#[inline]
pub const fn mated_in(ply: u8) -> Score {
    -MATE + ply as Score
}
