use std::cmp::Reverse;

use crate::{
    core::{Move, MoveScore},
    search::get_piece_value_rank,
};

impl super::Searcher<'_> {
    /// Sorts the moves based on their heuristic scores, prioritizing the
    /// transposition table best move, killers, and history.
    ///
    /// See: https://www.chessprogramming.org/Move_Ordering
    #[inline]
    pub(super) fn sort_moves(&self, moves: &mut [Move], tt_move: Option<Move>, ply: u8) {
        moves.sort_unstable_by_key(|&m| -> Reverse<MoveScore> {
            // Returns a heuristic move-ordering score. Captures are scored highly
            // based on MVV-LVA. Quiet moves get score 0. The transposition table
            // best move is prioritized at the very top.
            let move_score = if let Some(tt_move) = tt_move
                && m == tt_move
            {
                2_000_000_000 // Prioritize TT best move above all else
            } else if let Some(to_piece) = self.pos.piece_at(m.to()) {
                // Capture: 10000 + victim_rank * 100 - attacker_rank
                let victim = get_piece_value_rank(to_piece);
                let attacker = get_piece_value_rank(
                    self.pos
                        .piece_at(m.from())
                        .expect("Move from square should always have a piece"),
                );
                1_000_000_000 + victim * 10_000_000 - attacker * 100_000
            } else if self.killer_moves.contains(m, ply) {
                900_000_000
            } else {
                self.shared.history_moves.get(self.pos.side_to_move(), m)
            };

            Reverse(move_score)
        });
    }
}
