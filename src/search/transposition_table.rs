use strum::FromRepr;

use crate::core::{Move, Score, score};

/// Identifies the type of bounds for a Transposition Table evaluation score.
#[derive(Clone, Copy, PartialEq, Eq, Debug, FromRepr, Default)]
#[repr(u8)]
pub enum Bound {
    #[default]
    Empty,
    /// Score is exact/precise.
    Exact,
    /// Score represents an upper bound (score <= alpha).
    Alpha,
    /// Score represents a lower bound (score >= beta).
    Beta,
}

impl Bound {
    pub fn with_score(score: Score, alpha: Score, beta: Score) -> Self {
        if score >= beta {
            Bound::Beta
        } else if score <= alpha {
            Bound::Alpha
        } else {
            Bound::Exact
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Flags {
    data: u8,
}

impl Flags {
    pub fn bound(&self) -> Bound {
        match self.data & 0b11 {
            0 => Bound::Empty,
            1 => Bound::Exact,
            2 => Bound::Alpha,
            3 => Bound::Beta,
            _ => unreachable!(),
        }
    }

    pub fn age(&self) -> u8 {
        self.data >> 2
    }
}

/// Represents the hash key for Zobrist position hashing.
pub type Key = u64;

#[derive(Default, Debug, Clone, Copy)]
/// Represents an entry inside the Transposition Table.
pub struct Entry {
    /// The evaluation score (may be relative to mate).
    pub score: Score, // 4 bytes
    /// The best move found at this position. The upper bits of this Move encode
    /// the TranspositionTableFlag, which denotes the entry's bound type (and
    /// validity; a flag of Empty indicates an invalid/empty entry).
    pub best_move: Move, // 2 bytes
    /// The type of bound this entry's score represents.
    pub bound: Bound, // 1 byte
    /// The search depth this score was evaluated to.
    /// Negative depth is Quiescence search, non-negative is regular search
    /// depth.
    pub depth: i8, // 1 byte
}

#[derive(Clone, Debug, Default)]
struct Storage {
    /// Zobrist board hash key.
    pub key: Key,
    /// The evaluation score (may be relative to mate).
    pub score: Score, // 4 bytes
    /// The best move found at this position. The upper bits of this Move encode
    /// the TranspositionTableFlag, which denotes the entry's bound type (and
    /// validity; a flag of Empty indicates an invalid/empty entry).
    pub best_move: Move, // 2 bytes
    /// The type of bound and age this entry represents. The lower 2 bits encode the bound type.
    pub flags: Flags, // 1 byte
    /// The search depth this score was evaluated to.
    /// Negative depth is Quiescence search, non-negative is regular search
    /// depth.
    pub depth: i8, // 1 byte
}

#[macro_export]
macro_rules! tt_value {
    ($score:expr, $best_move:expr, $bound:expr, $depth:expr) => {
        Entry {
            score: $score,
            bound: $bound,
            best_move: $best_move,
            depth: $depth,
        }
    };
}

/// A cache structure storing previously evaluated search nodes to speed up
/// alpha-beta pruning.
pub struct TranspositionTable {
    /// A vector of entries matching the table size.
    table: Vec<Storage>,
    /// Size mask to perform O(1) fast bitwise modulo logic.
    size_mask: usize,
    /// The age of the table, incremented each search iteration.
    age: u8,
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new(16) // Default to 16 MB
    }
}

impl TranspositionTable {
    /// Creates a new Transposition Table instance allocated with a target MB
    /// capacity.
    pub fn new(mb_size: usize) -> Self {
        let mut tt = Self {
            table: Vec::new(),
            size_mask: 0,
            age: 0,
        };
        tt.resize(mb_size);
        tt
    }
    const AGE_MASK: u8 = 0b111111; // 6 bits for age, allowing up to 64 iterations before wrap-around
    const AGE_CYCLE: u8 = Self::AGE_MASK + 1; // 64 iterations before age wraps around

    pub fn incremente_age(&mut self) {
        self.age = (self.age + 1) & Self::AGE_MASK;
    }

    pub fn age(&self) -> u8 {
        self.age
    }

    pub fn relative_age(&self, age: u8) -> u8 {
        (Self::AGE_CYCLE + age - self.age) & Self::AGE_MASK
    }

    /// Resizes the Transposition Table. Caps element allocation to the largest
    /// power of 2 fitting within the memory limit to allow ultra-fast
    /// bitwise masking.
    pub fn resize(&mut self, mb_size: usize) {
        let entry_size = std::mem::size_of::<Storage>();
        let target_bytes = mb_size * 1024 * 1024;
        let count = target_bytes / entry_size;

        if count == 0 {
            self.table = Vec::new();
            self.size_mask = 0;
            return;
        }

        // Find the largest power of 2 <= count
        let mut power = 1;
        while power * 2 <= count {
            power *= 2;
        }

        self.table = vec![Storage::default(); power];
        self.size_mask = power - 1;
        self.age = 0;
    }

    /// Resets all transposition entries back to defaults.
    pub fn clear(&mut self) {
        for entry in self.table.iter_mut() {
            *entry = Storage::default();
        }
    }

    /// Probes the table for an entry matching the given Zobrist key.
    /// Returns `Some` if a valid entry exists, `None` otherwise.
    #[inline]
    pub fn probe(&self, key: Key, ply: u8) -> Option<Entry> {
        assert!(
            !self.table.is_empty(),
            "Transposition Table is not initialized. Call resize() with a positive MB size."
        );
        let index = (key as usize) & self.size_mask;
        let entry = &self.table[index];
        if entry.key == key && entry.flags.bound() != Bound::Empty {
            Some(Entry {
                score: score::ply_dependent(entry.score, ply),
                best_move: entry.best_move,
                bound: entry.flags.bound(),
                depth: entry.depth,
            })
        } else {
            None
        }
    }

    /// Stores search details into the Transposition Table using a
    /// Depth-Preferred and Age-Preferred replacement strategy.
    #[inline]
    pub fn store(&mut self, key: Key, ply: u8, value: Entry) {
        if self.table.is_empty() {
            return;
        }
        let index = (key as usize) & self.size_mask;
        let Storage {
            flags: existing_flags,
            depth: existing_depth,
            ..
        } = self.table[index];

        let is_empty = existing_flags.bound() == Bound::Empty;
        let is_better =
            value.depth >= existing_depth - self.relative_age(existing_flags.age()) as i8;

        // Store if the slot is unused, a different board position collided,
        // this search is deeper/better, or the existing entry is stale (old age).
        if is_empty || is_better {
            self.table[index] = Storage {
                key,
                score: score::ply_independent(value.score, ply),
                best_move: value.best_move,
                flags: Flags {
                    data: (value.bound as u8) | (self.age << 2),
                },
                depth: value.depth,
            };
        }
    }

    /// Calculates the table's fullness in per-mille (0–1000).
    pub fn hashfull(&self) -> u32 {
        if self.table.is_empty() {
            return 0;
        }
        let sample_size = self.table.len().min(1000);
        let mut filled = 0;
        for i in 0..sample_size {
            if self.table[i].flags.bound() != Bound::Empty {
                filled += 1;
            }
        }
        (filled * 1000 / sample_size) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transposition_table_resize_and_clearing() {
        let mut tt = TranspositionTable::new(1); // 1 MB
        assert!(tt.size_mask > 0);

        tt.store(42, 0, tt_value!(100, Move::NULL, Bound::Exact, 5));
        let result = tt.probe(42, 0).unwrap();
        assert_eq!(result.depth, 5);
        assert_eq!(result.score, 100);
        assert_eq!(result.bound, Bound::Exact);

        tt.clear();
        assert!(tt.probe(42, 0).is_none());
    }

    #[test]
    fn test_transposition_table_replacement_rules() {
        let mut tt = TranspositionTable::new(1);

        // 1. Store initial entry
        tt.store(100, 0, tt_value!(80, Move::NULL, Bound::Exact, 4));
        assert_eq!(tt.probe(100, 0).unwrap().score, 80);

        // 2. Reject shallower entry
        tt.store(100, 0, tt_value!(90, Move::NULL, Bound::Exact, 2));
        assert_eq!(tt.probe(100, 0).unwrap().depth, 4); // Kept depth 4
        assert_eq!(tt.probe(100, 0).unwrap().score, 80);

        // 3. Overwrite with deeper entry
        tt.store(100, 0, tt_value!(120, Move::NULL, Bound::Exact, 6));
        assert_eq!(tt.probe(100, 0).unwrap().depth, 6);
        assert_eq!(tt.probe(100, 0).unwrap().score, 120);
    }

    #[test]
    fn test_transposition_table_mate_score_mapping() {
        let mate_score = score::mate_in(10); // Mate in 10 plies
        let ply = 5;

        let mut tt = TranspositionTable::new(1);
        tt.store(100, ply, tt_value!(mate_score, Move::NULL, Bound::Exact, 6));
        let result = tt.probe(100, ply).unwrap();
        // The stored mate score should be correctly adjusted for the ply when probed.
        assert_eq!(result.score, mate_score);
    }

    #[test]
    fn test_transposition_table_hashfull() {
        let mut tt = TranspositionTable::new(1);
        assert_eq!(tt.hashfull(), 0);

        // Store one entry
        tt.store(42, 0, tt_value!(100, Move::NULL, Bound::Exact, 5));

        let h = tt.hashfull();
        assert!(h > 0);
        assert!(h <= 1000);
    }
}
