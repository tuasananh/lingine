use crate::core::types::{Key, Move, Value};
use crate::search::MATE_VALUE;

/// Identifies the type of bounds for a Transposition Table evaluation score.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum TranspositionTableFlag {
    /// Score is exact/precise.
    Exact,
    /// Score represents an upper bound (score <= alpha).
    Alpha,
    /// Score represents a lower bound (score >= beta).
    Beta,
}

/// Represents an entry inside the Transposition Table.
#[derive(Clone, Copy, Debug)]
pub struct TranspositionTableEntry {
    /// Zobrist board hash key.
    pub key: Key,
    /// The search depth this score was evaluated to (-1 represents empty/invalid).
    pub depth: i16,
    /// The evaluation score (may be relative to mate).
    pub score: Value,
    /// The evaluation boundary flag.
    pub flag: TranspositionTableFlag,
    /// The best move found at this position.
    pub best_move: Move,
    /// The search sequence generation/age to track relevance.
    pub age: u8,
}

impl Default for TranspositionTableEntry {
    fn default() -> Self {
        Self {
            key: 0,
            depth: -1, // Invalid depth to denote empty/unused entry
            score: 0,
            flag: TranspositionTableFlag::Exact,
            best_move: Move::none(),
            age: 0,
        }
    }
}

/// A cache structure storing previously evaluated search nodes to speed up alpha-beta pruning.
pub struct TranspositionTable {
    /// A vector of entries matching the table size.
    table: Box<[TranspositionTableEntry]>,
    /// Size mask to perform O(1) fast bitwise modulo logic.
    size_mask: usize,
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new(16) // Default to 16 MB
    }
}

impl TranspositionTable {
    /// Creates a new Transposition Table instance allocated with a target MB capacity.
    pub fn new(mb_size: usize) -> Self {
        let mut tt = Self {
            table: Box::new([]),
            size_mask: 0,
        };
        tt.resize(mb_size);
        tt
    }

    /// Resizes the Transposition Table. Caps element allocation to the largest power of 2
    /// fitting within the memory limit to allow ultra-fast bitwise masking.
    pub fn resize(&mut self, mb_size: usize) {
        let entry_size = std::mem::size_of::<TranspositionTableEntry>();
        let target_bytes = mb_size * 1024 * 1024;
        let count = target_bytes / entry_size;

        if count == 0 {
            self.table = Box::new([]);
            self.size_mask = 0;
            return;
        }

        // Find the largest power of 2 <= count
        let mut power = 1;
        while power * 2 <= count {
            power *= 2;
        }

        self.table = vec![TranspositionTableEntry::default(); power].into_boxed_slice();
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
    pub fn probe(&self, key: Key) -> Option<TranspositionTableEntry> {
        if self.table.is_empty() {
            return None;
        }
        let index = (key as usize) & self.size_mask;
        let entry = self.table[index];
        if entry.key == key && entry.depth >= 0 {
            Some(entry)
        } else {
            None
        }
    }

    /// Stores search details into the Transposition Table using a Depth-Preferred and Age-Preferred replacement strategy.
    pub fn store(
        &mut self,
        key: Key,
        depth: i16,
        score: Value,
        flag: TranspositionTableFlag,
        best_move: Move,
        age: u8,
    ) {
        if self.table.is_empty() {
            return;
        }
        let index = (key as usize) & self.size_mask;
        let existing = &mut self.table[index];

        let is_empty = existing.depth < 0;
        let is_collision = existing.key != key;
        let is_deeper = depth >= existing.depth;
        let is_older = age != existing.age;

        // Store if the slot is unused, a different board position collided,
        // this search is deeper/better, or the existing entry is stale (old age).
        if is_empty || is_collision || is_deeper || is_older {
            *existing = TranspositionTableEntry {
                key,
                depth,
                score,
                flag,
                best_move,
                age,
            };
        }
    }

    /// Converts a search score to an absolute score independent of search depth (ply) for storing.
    #[inline(always)]
    pub fn score_to_transposition(score: Value, ply: i32) -> Value {
        if score > MATE_VALUE - 1000 {
            score + ply
        } else if score < -MATE_VALUE + 1000 {
            score - ply
        } else {
            score
        }
    }

    /// Restores a stored absolute score back to a ply-dependent search evaluation score.
    #[inline(always)]
    pub fn score_from_transposition(score: Value, ply: i32) -> Value {
        if score > MATE_VALUE - 1000 {
            score - ply
        } else if score < -MATE_VALUE + 1000 {
            score + ply
        } else {
            score
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transposition_table_resize_and_clearing() {
        let mut tt = TranspositionTable::new(1); // 1 MB
        assert!(tt.size_mask > 0);

        tt.store(42, 5, 100, TranspositionTableFlag::Exact, Move::none(), 1);
        let entry = tt.probe(42).unwrap();
        assert_eq!(entry.key, 42);
        assert_eq!(entry.depth, 5);
        assert_eq!(entry.score, 100);
        assert_eq!(entry.flag, TranspositionTableFlag::Exact);

        tt.clear();
        assert!(tt.probe(42).is_none());
    }

    #[test]
    fn test_transposition_table_replacement_rules() {
        let mut tt = TranspositionTable::new(1);

        // 1. Store initial entry
        tt.store(100, 4, 80, TranspositionTableFlag::Exact, Move::none(), 1);
        assert_eq!(tt.probe(100).unwrap().score, 80);

        // 2. Reject shallower entry
        tt.store(100, 2, 90, TranspositionTableFlag::Exact, Move::none(), 1);
        assert_eq!(tt.probe(100).unwrap().depth, 4); // Kept depth 4
        assert_eq!(tt.probe(100).unwrap().score, 80);

        // 3. Overwrite with deeper entry
        tt.store(100, 6, 120, TranspositionTableFlag::Exact, Move::none(), 1);
        assert_eq!(tt.probe(100).unwrap().depth, 6);
        assert_eq!(tt.probe(100).unwrap().score, 120);

        // 4. Overwrite same depth if older age
        tt.store(100, 6, 200, TranspositionTableFlag::Alpha, Move::none(), 2);
        let entry = tt.probe(100).unwrap();
        assert_eq!(entry.score, 200);
        assert_eq!(entry.flag, TranspositionTableFlag::Alpha);
        assert_eq!(entry.age, 2);
    }

    #[test]
    fn test_transposition_table_mate_score_mapping() {
        let mate_score = MATE_VALUE - 10; // Mate in 10 plies
        let ply = 5;

        // Score stored in transposition table should be independent of current search depth
        let stored = TranspositionTable::score_to_transposition(mate_score, ply);
        assert_eq!(stored, mate_score + ply); // MATE_VALUE - 10 + 5 = MATE_VALUE - 5

        // Restored score should adapt to current ply
        let restored = TranspositionTable::score_from_transposition(stored, ply);
        assert_eq!(restored, mate_score);
    }
}
