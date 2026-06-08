use crate::{
    core::{Move, score},
    search::SearchContext,
};

impl super::Searcher<'_> {
    /// Starts an iterative deepening search up to the specified maximum depth,
    /// with aspiration windows and UCI info updates.
    pub(super) fn iterative_deepening(mut self) -> Move {
        let mut last_best_score = -score::INFINITY;
        let mut last_best_move = Move::NULL;

        const ASPIRATION_WINDOW_THRESHOLD: i8 = 6;
        // Iterative deepening: This helps to find good moves faster and allows us to
        // send intermediate results back to the main thread after each depth
        // iteration. It also enables aspiration windows based on the previous depth's
        // score.
        //
        // See: https://www.chessprogramming.org/Iterative_Deepening
        for depth in 1..=self.time_manager.max_depth() {
            let score = if depth >= ASPIRATION_WINDOW_THRESHOLD {
                self.aspiration_search(last_best_score, depth)
            } else {
                self.negamax::<true>(
                    depth,
                    1,
                    -score::INFINITY,
                    score::INFINITY,
                    SearchContext::default(),
                )
            };

            last_best_score = score;
            last_best_move = self
                .shared
                .transposition_table
                .probe(self.pos.zobrist_hash(), 0)
                .unwrap()
                .best_move;

            self.send_uci_info(depth, last_best_score, last_best_move);

            if !self.shared.keep_running.get() {
                break;
            }

            // Check if we still have enough time for another iteration to avoid timing out
            // in the middle of a ply. Also known as a Soft-Bound Time Limit.
            //
            // See: https://www.chessprogramming.org/Time_Management#Soft_Bound
            if self.time_manager.is_soft_bound_reached() {
                break;
            }
        }

        last_best_move
    }
}
