use crate::core::{MAX_PLY, Move};

const MOVE_COUNT_PER_PLY: usize = 2;

/// Provides an implementation of the killer heuristic used as a dynamic move
/// ordering technique for quiet moves that caused a beta cutoff.
///
/// See [Killer Heuristic](https://www.chessprogramming.org/Killer_Heuristic)
pub struct KillerMoves {
    table: [[Move; MOVE_COUNT_PER_PLY]; MAX_PLY],
}

impl KillerMoves {
    pub fn new() -> Self {
        Self {
            table: [[Move::NULL; MOVE_COUNT_PER_PLY]; MAX_PLY],
        }
    }

    pub fn update(&mut self, mv: Move, ply: u8) {
        let ply_index = ply as usize;

        // We only store different moves
        if mv != self.table[ply_index][0] {
            self.table[ply_index][1] = self.table[ply_index][0];
            self.table[ply_index][0] = mv;
        }
    }

    pub fn contains(&self, mv: Move, ply: u8) -> bool {
        self.table[ply as usize].contains(&mv)
    }
}

impl Default for KillerMoves {
    fn default() -> Self {
        Self::new()
    }
}
