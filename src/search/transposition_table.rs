//! Transposition Table (TT) cache system to optimize search space exploration.
//!
//! The Transposition Table acts as an O(1) hash table mapping Zobrist position
//! keys to previously evaluated search states. This allows the search algorithm
//! to prune identical positions reached via different move sequences
//! (transpositions).
//!
//! Key elements:
//! 1. **Table Sizing**: Memory capacity allocated in Megabytes (MB). Elements
//!    are restricted to the nearest smaller power-of-two count to support
//!    ultra-fast bitwise masking modulo.
//! 2. **Replacement Strategy**: Uses a hybrid Depth-Preferred and Age-Preferred
//!    replacement policy to overwrite existing slots only when the new entry is
//!    deeper, a different board position collided, or the existing entry is
//!    stale (belongs to an older search age).
//! 3. **Score Mapping**: Stored scores are converted to ply-independent values
//!    inside the table, and converted back to ply-dependent values on probe
//!    retrieval. This preserves correct mate-distance scores across different
//!    branches of the search tree.

use strum::FromRepr;

use crate::core::{Move, Score, TranspositionTableKey};

/// Identifies the type of bounds for a Transposition Table evaluation score.
#[derive(Clone, Copy, PartialEq, Eq, Debug, FromRepr, Default)]
#[repr(u8)]
pub enum TranspositionTableFlag {
    #[default]
    Empty,
    /// Score is exact/precise.
    Exact,
    /// Score represents an upper bound (score <= alpha).
    Alpha,
    /// Score represents a lower bound (score >= beta).
    Beta,
}

#[derive(Default, Debug, Clone)]
/// Represents an entry inside the Transposition Table.
pub struct TranspositionTableValue {
    /// The evaluation score (may be relative to mate).
    pub score: Score,
    /// The best move found at this position. The upper bits of this Move encode
    /// the TranspositionTableFlag, which denotes the entry's bound type (and
    /// validity; a flag of Empty indicates an invalid/empty entry).
    pub best_move: Move,
    /// The type of bound this entry's score represents.
    pub flag: TranspositionTableFlag,
    /// The search depth this score was evaluated to.
    /// Negative depth is Quiescence search, non-negative is regular search
    /// depth.
    pub depth: i8,
    /// The search sequence generation/age to track relevance.
    pub age: u8,
}

#[derive(Clone, Debug, Default)]
struct TranspositionTableEntry {
    /// Zobrist board hash key.
    pub key: TranspositionTableKey,
    pub value: TranspositionTableValue,
}

#[macro_export]
macro_rules! tt_value {
    ($score:expr, $flag:expr, $best_move:expr, $depth:expr, $age:expr) => {
        TranspositionTableValue {
            score: $score,
            flag: $flag,
            best_move: $best_move,
            depth: $depth,
            age: $age,
        }
    };
}

/// A cache structure storing previously evaluated search nodes to speed up
/// alpha-beta pruning.
pub struct TranspositionTable {
    /// A vector of entries matching the table size.
    table: Vec<TranspositionTableEntry>,
    /// Size mask to perform O(1) fast bitwise modulo logic.
    size_mask: usize,
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
        };
        tt.resize(mb_size);
        tt
    }

    /// Resizes the Transposition Table. Caps element allocation to the largest
    /// power of 2 fitting within the memory limit to allow ultra-fast
    /// bitwise masking.
    pub fn resize(&mut self, mb_size: usize) {
        let entry_size = std::mem::size_of::<TranspositionTableEntry>();
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

        self.table = vec![TranspositionTableEntry::default(); power];
        self.size_mask = power - 1;
    }

    /// Resets all transposition entries back to defaults.
    pub fn clear(&mut self) {
        for entry in self.table.iter_mut() {
            *entry = TranspositionTableEntry::default();
        }
    }

    /// Probes the table for an entry matching the given Zobrist key.
    /// Returns `Some` if a valid entry exists, `None` otherwise.
    #[inline]
    pub fn probe(&self, key: TranspositionTableKey, ply: u8) -> Option<TranspositionTableValue> {
        assert!(
            !self.table.is_empty(),
            "Transposition Table is not initialized. Call resize() with a positive MB size."
        );
        let index = (key as usize) & self.size_mask;
        let entry = &self.table[index];
        if entry.key == key && entry.value.flag != TranspositionTableFlag::Empty {
            Some(TranspositionTableValue {
                score: entry.value.score.ply_dependent(ply),
                ..entry.value
            })
        } else {
            None
        }
    }

    /// Stores search details into the Transposition Table using a
    /// Depth-Preferred and Age-Preferred replacement strategy.
    #[inline]
    pub fn store(&mut self, key: TranspositionTableKey, ply: u8, value: TranspositionTableValue) {
        if self.table.is_empty() {
            return;
        }
        let index = (key as usize) & self.size_mask;
        let existing = &mut self.table[index];

        let is_empty = existing.value.flag == TranspositionTableFlag::Empty;
        let is_collision = existing.key != key;
        let is_deeper = value.depth >= existing.value.depth;
        // Since age is wrapping u8, we can consider an entry "older" if its age is
        // different from the current age.
        let is_older = value.age != existing.value.age;

        // Store if the slot is unused, a different board position collided,
        // this search is deeper/better, or the existing entry is stale (old age).
        if is_empty || is_collision || is_deeper || is_older {
            *existing = TranspositionTableEntry {
                key,
                value: TranspositionTableValue {
                    score: value.score.ply_independent(ply),
                    ..value
                },
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
            if self.table[i].value.flag != TranspositionTableFlag::Empty {
                filled += 1;
            }
        }
        (filled * 1000 / sample_size) as u32
    }
}

#[cfg(test)]
mod tests {
    use crate::score;

    use super::*;

    #[test]
    fn test_transposition_table_resize_and_clearing() {
        let mut tt = TranspositionTable::new(1); // 1 MB
        assert!(tt.size_mask > 0);

        tt.store(
            42,
            0,
            tt_value!(score!(100), TranspositionTableFlag::Exact, Move::NULL, 5, 1),
        );
        let result = tt.probe(42, 0).unwrap();
        assert_eq!(result.depth, 5);
        assert_eq!(result.score, score!(100));
        assert_eq!(result.flag, TranspositionTableFlag::Exact);

        tt.clear();
        assert!(tt.probe(42, 0).is_none());
    }

    #[test]
    fn test_transposition_table_replacement_rules() {
        let mut tt = TranspositionTable::new(1);

        // 1. Store initial entry
        tt.store(
            100,
            0,
            tt_value!(score!(80), TranspositionTableFlag::Exact, Move::NULL, 4, 1),
        );
        assert_eq!(tt.probe(100, 0).unwrap().score.raw(), 80);

        // 2. Reject shallower entry
        tt.store(
            100,
            0,
            tt_value!(score!(90), TranspositionTableFlag::Exact, Move::NULL, 2, 1),
        );
        assert_eq!(tt.probe(100, 0).unwrap().depth, 4); // Kept depth 4
        assert_eq!(tt.probe(100, 0).unwrap().score.raw(), 80);

        // 3. Overwrite with deeper entry
        tt.store(
            100,
            0,
            tt_value!(score!(120), TranspositionTableFlag::Exact, Move::NULL, 6, 1),
        );
        assert_eq!(tt.probe(100, 0).unwrap().depth, 6);
        assert_eq!(tt.probe(100, 0).unwrap().score.raw(), 120);

        // 4. Overwrite same depth if older age
        tt.store(
            100,
            0,
            tt_value!(score!(200), TranspositionTableFlag::Alpha, Move::NULL, 6, 2),
        );
        let entry = tt.probe(100, 0).unwrap();
        assert_eq!(entry.score.raw(), 200);
        assert_eq!(entry.flag, TranspositionTableFlag::Alpha);
        assert_eq!(entry.age, 2);
    }

    #[test]
    fn test_transposition_table_mate_score_mapping() {
        let mate_score = Score::mate_in(10); // Mate in 10 plies
        let ply = 5;

        let mut tt = TranspositionTable::new(1);
        tt.store(
            100,
            ply,
            tt_value!(mate_score, TranspositionTableFlag::Exact, Move::NULL, 6, 1),
        );
        let result = tt.probe(100, ply).unwrap();
        // The stored mate score should be correctly adjusted for the ply when probed.
        assert_eq!(result.score, mate_score);
    }

    #[test]
    fn test_transposition_table_hashfull() {
        let mut tt = TranspositionTable::new(1);
        assert_eq!(tt.hashfull(), 0);

        // Store one entry
        tt.store(
            42,
            0,
            tt_value!(score!(100), TranspositionTableFlag::Exact, Move::NULL, 5, 1),
        );

        let h = tt.hashfull();
        assert!(h > 0);
        assert!(h <= 1000);
    }
}
