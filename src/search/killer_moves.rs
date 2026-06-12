use crate::{core::Move, search::MAX_PLY};

const MOVE_COUNT_PER_PLY: usize = 2;

/// Provides an implementation of the killer heuristic used as a dynamic move
/// ordering technique for quiet moves that caused a beta cutoff.
///
/// See [Killer Heuristic](https://www.chessprogramming.org/Killer_Heuristic)
pub struct KillerMoves {
    table: [[Option<Move>; MOVE_COUNT_PER_PLY]; MAX_PLY],
}

impl KillerMoves {
    pub fn new() -> Self {
        Self {
            table: [[None; MOVE_COUNT_PER_PLY]; MAX_PLY],
        }
    }

    pub fn update(&mut self, mv: Move, ply: u8) {
        let ply_index = ply as usize;

        if self.table[ply_index][0] != Some(mv) {
            self.table[ply_index][1] = self.table[ply_index][0];
            self.table[ply_index][0] = Some(mv);
        }
    }

    pub fn contains(&self, mv: Move, ply: u8) -> bool {
        self.table[ply as usize].contains(&Some(mv))
    }
}

impl Default for KillerMoves {
    fn default() -> Self {
        Self::new()
    }
}
