use strum::EnumCount;

use crate::core::{Move, Side, Square};

const DECAY_RATE: i32 = 8;

/// Provides an implementation of the history heuristic used as a dynamic move
/// ordering technique for quiet moves that caused a beta cutoff.
///
/// See [History Heuristic](https://www.chessprogramming.org/History_Heuristic)
pub struct HistoryMoves {
    table: [[[i32; Square::COUNT]; Square::COUNT]; Side::COUNT],
}

impl HistoryMoves {
    pub fn new() -> Self {
        Self {
            table: [[[0; Square::COUNT]; Square::COUNT]; Side::COUNT],
        }
    }

    #[inline]
    pub fn get(&self, color: Side, mv: Move) -> i32 {
        self.table[color as usize][mv.from() as usize][mv.to() as usize]
    }

    #[inline]
    pub fn increase(&mut self, color: Side, mv: Move, depth: i8) {
        let depth = depth as i32;
        let bonus = depth * depth;
        self.table[color as usize][mv.from() as usize][mv.to() as usize] += bonus;
    }

    #[inline]
    pub fn decrease(&mut self, color: Side, mv: Move, depth: i8) {
        let depth = depth as i32;
        let bonus = depth * depth;
        self.table[color as usize][mv.from() as usize][mv.to() as usize] -= bonus;
    }

    pub fn decay(&mut self) {
        for side in self.table.iter_mut() {
            for from in side.iter_mut() {
                for to in from.iter_mut() {
                    *to /= DECAY_RATE;
                }
            }
        }
    }
}

impl Default for HistoryMoves {
    fn default() -> Self {
        Self::new()
    }
}
