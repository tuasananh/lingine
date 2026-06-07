use crate::core::MAX_PLY;

pub type Score = i16;

pub const ZERO: i16 = 0;
pub const DRAW: i16 = 0;
pub const MATE: i16 = 32_000;
pub const INFINITY: i16 = 32_001;
pub const MATE_IN_MAX_PLY: i16 = MATE - MAX_PLY as i16;
pub const MATED_IN_MAX_PLY: i16 = -MATE + MAX_PLY as i16;

/// Checks whether the score is winning (mate in some plies)
#[inline]
pub const fn is_winning(value: i16) -> bool {
    value >= MATE_IN_MAX_PLY
}

/// Checks whether the score is losing (mated mate in some plies)
#[inline]
pub const fn is_losing(value: i16) -> bool {
    value <= MATED_IN_MAX_PLY
}

/// Get a score that is ply independent, useful for
/// [`crate::search::TranspositionTable::store`]
#[inline]
pub const fn ply_independent(value: i16, ply: u8) -> i16 {
    if is_winning(value) {
        value + ply as i16
    } else if is_losing(value) {
        value - ply as i16
    } else {
        value
    }
}

/// Get a score that is ply independent, useful for
/// [`crate::search::TranspositionTable::probe`]
#[inline]
pub const fn ply_dependent(value: i16, ply: u8) -> i16 {
    if is_winning(value) {
        value - ply as i16
    } else if is_losing(value) {
        value + ply as i16
    } else {
        value
    }
}

/// Gets the number of ply until we have a mate or get mated
#[inline]
pub const fn ply_to_mate_or_mated(value: i16) -> Option<u8> {
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
pub const fn mate_in(ply: u8) -> i16 {
    MATE - ply as i16
}

/// Value for mated in some ply
#[inline]
pub const fn mated_in(ply: u8) -> i16 {
    -MATE + ply as i16
}
