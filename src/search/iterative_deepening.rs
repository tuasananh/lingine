use crate::{
    core::{Move, score},
    search::SearchContext,
};

impl super::Searcher<'_> {
    /// Starts an iterative deepening search up to the specified maximum depth,
    /// with aspiration windows and UCI info updates.
    ///
    /// This technique helps to find good moves faster and allows us to
    /// send intermediate results back to the main thread after each depth
    /// iteration. It also enables aspiration windows based on the previous
    /// depth's score.
    ///
    /// See: https://www.chessprogramming.org/Iterative_Deepening
    pub(super) fn iterative_deepening(mut self) -> Move {
        let mut last_best_score = -score::INFINITY;
        let mut last_best_move = Move::NULL;

        const ASPIRATION_WINDOW_THRESHOLD: i8 = 6;
        let max_depth = self.time_manager.max_depth();
        while self.current_root_depth < max_depth {
            self.current_root_depth += 1;
            let depth = self.current_root_depth;

            let score = if depth >= ASPIRATION_WINDOW_THRESHOLD {
                self.aspiration_search(last_best_score, depth)
            } else {
                self.negamax::<true, true>(
                    depth,
                    0,
                    -score::INFINITY,
                    score::INFINITY,
                    SearchContext::default(),
                )
            };

            if !self.shared.keep_running.get() {
                break;
            }

            last_best_score = score;
            last_best_move = *self.pv_table.get_line(0).first().unwrap_or_else(|| {
                eprintln!(
                    "Warning: No best move found for depth {}. This should not happen.",
                    depth
                );
                &Move::NULL
            });

            self.send_uci_info(depth, last_best_score);

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
