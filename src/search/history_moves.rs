use strum::EnumCount;

use crate::core::{Color, Move, Square};

const DECAY_RATE: i32 = 8;

pub struct HistoryMoves {
    table: [[[i32; Square::COUNT]; Square::COUNT]; Color::COUNT],
}

impl HistoryMoves {
    pub fn get(&self, color: Color, mv: Move) -> i32 {
        self.table[color as usize][mv.from() as usize][mv.to() as usize]
    }

    pub fn increase(&mut self, color: Color, mv: Move, depth: i8) {
        let depth = depth as i32;
        let bonus = depth * depth;
        self.table[color as usize][mv.from() as usize][mv.to() as usize] += bonus;
    }

    pub fn decrease(&mut self, color: Color, mv: Move, depth: i8) {
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
        Self {
            table: [[[0; Square::COUNT]; Square::COUNT]; Color::COUNT],
        }
    }
}
