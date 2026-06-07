use crate::{
    core::{Move, MoveGenType, MoveList, Score, generate_moves, score},
    search::SearchContext,
};

impl super::Searcher<'_> {
    /// Starts an iterative deepening search up to the specified maximum depth,
    /// with aspiration windows and UCI info updates.
    pub(super) fn iterative_deepening(mut self, max_depth: i8) -> (Score, Move, u64) {
        // Fetch the best move from the transposition table to use as the initial guess
        // for best move, Since we might have seen this position before in
        // earlier searches.
        let mut best_move = self
            .transposition_table
            .probe(self.pos.zobrist_hash(), 0)
            .map_or(Move::NULL, |entry| entry.best_move);

        let mut last_depth_score = -score::INFINITY;

        let mut moves = MoveList::new();
        generate_moves(&self.pos, MoveGenType::Legal, &mut moves);

        if moves.is_empty() {
            return (last_depth_score, Move::NULL, 0);
        }

        // Iterative deepening: This helps to find good moves faster and allows us to
        // send intermediate results back to the main thread after each depth
        // iteration. It also enables aspiration windows based on the previous depth's
        // score.
        //
        // See: https://www.chessprogramming.org/Iterative_Deepening
        for depth in 1..=max_depth {
            if !self.keep_running.get() {
                break;
            }

            // Check if we still have enough time for another iteration to avoid timing out
            // in the middle of a ply. Also known as a Soft-Bound Time Limit.
            //
            // See: https://www.chessprogramming.org/Time_Management#Soft_Bound
            if let Some(limit) = self.allocated_time
                && self.start_time.elapsed() > limit / 2
            {
                break;
            }

            // Sort moves at the root based on the previous depth's best move and heuristics
            // to maximize alpha-beta pruning efficiency in the next iteration.
            self.sort_moves(&mut moves, best_move, 0);

            let mut best_score;
            let mut depth_best_move;

            // Aspiration Windows
            //
            // We set a narrow window around the previous depth's score to try to
            // trigger more beta cutoffs and speed up the search. If the search fails low or
            // high, we widen the window and re-search until we get a stable
            // score within the window.
            //
            // See: https://www.chessprogramming.org/Aspiration_Windows
            let mut alpha = -score::INFINITY;
            let mut beta = score::INFINITY;
            let mut delta: Score = 25; // aspiration window size in centipawns

            if depth >= 5 && !score::is_winning(last_depth_score.abs()) {
                alpha = last_depth_score - delta;
                beta = last_depth_score + delta;
            }

            loop {
                let search_alpha = alpha.max(-score::INFINITY);
                let search_beta = beta.min(score::INFINITY);

                let mut curr_alpha = search_alpha;
                best_score = -score::INFINITY;
                depth_best_move = Move::NULL;

                for m in moves.iter().copied() {
                    if !self.keep_running.get() {
                        break;
                    }
                    self.pos.do_move(m);
                    let score = -self.negamax(
                        depth - 1,
                        1,
                        -search_beta,
                        -curr_alpha,
                        SearchContext::default(),
                    );
                    self.pos.undo_move();

                    if score > best_score {
                        best_score = score;
                        depth_best_move = m;
                    }
                    if score > curr_alpha {
                        curr_alpha = score;
                    }
                }

                if !self.keep_running.get() {
                    break;
                }

                // If window was already full (-INFINITY, INFINITY), we stop, no re-search.
                if search_alpha == -score::INFINITY && search_beta == score::INFINITY {
                    break;
                }

                // Check fail-low / fail-high
                if best_score <= search_alpha {
                    // Fail low: score worse or equal to alpha. Widen alpha.
                    alpha -= delta;
                    beta = best_score + delta;
                    delta *= 2;
                } else if best_score >= search_beta {
                    // Fail high: score better or equal to beta. Widen beta.
                    beta += delta;
                    alpha = best_score - delta;
                    delta *= 2;
                } else {
                    // Stable score inside window!
                    break;
                }
            }

            if self.keep_running.get() && !depth_best_move.is_null() {
                last_depth_score = best_score;
                best_move = depth_best_move;

                self.send_uci_info(depth, best_score, best_move);
            }
        }

        self.keep_running.set(false);

        (last_depth_score, best_move, self.nodes)
    }
}
