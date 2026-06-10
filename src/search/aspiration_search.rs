use crate::{
    core::{Score, score},
    search::SearchContext,
};

impl super::Searcher<'_> {
    pub(super) fn aspiration_search(&mut self, score: Score, depth: i8) -> Score {
        const ASPIRATION_WINDOW_SIZE: Score = 25; // aspiration window size in centipawns
        // Aspiration Windows
        //
        // We set a narrow window around the previous depth's score to try to
        // trigger more beta cutoffs and speed up the search. If the search fails low or
        // high, we widen the window and re-search until we get a stable
        // score within the window.
        //
        // See: https://www.chessprogramming.org/Aspiration_Windows
        // We assume that the more depth the better the score is
        let mut delta: Score = (ASPIRATION_WINDOW_SIZE - depth as Score).max(10);
        let mut alpha = (score - delta).max(-score::INFINITY);
        let mut beta = (score + delta).min(score::INFINITY);

        loop {
            let score = self.negamax::<true, true>(depth, 0, alpha, beta, SearchContext::default());

            if !self.shared.keep_running.get() {
                return score::ZERO;
            }

            if score <= alpha {
                // Fail low: score worse or equal to alpha. Widen alpha.
                alpha = (alpha - delta).max(-score::INFINITY);
                beta = score + delta;
            } else if score >= beta {
                // Fail high: score better or equal to beta. Widen beta.
                beta = (beta + delta).min(score::INFINITY);
                alpha = score - delta;
            } else {
                return score;
            }

            delta *= 2;
        }
    }
}
