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

        let mut consecutive_fail_high = 0;

        loop {
            // For consecutive fail highs, we do not need to search the full depth
            // since we know the position is quite good already, so we do a reduced
            // search to find out the score much quicker. But in fail lows, we need
            // to search full depth since we might be in a critical position.
            let search_depth = (depth - consecutive_fail_high).max(1);
            let score =
                self.negamax::<true, true>(search_depth, 0, alpha, beta, SearchContext::default());

            if !self.shared.keep_running.get() {
                return score::ZERO;
            }

            if score <= alpha {
                // Fail low: score worse or equal to alpha. Widen alpha.
                alpha = (alpha - delta).max(-score::INFINITY);
                beta = score + delta;
                consecutive_fail_high = 0;
            } else if score >= beta {
                // Fail high: score better or equal to beta. Widen beta.
                beta = (beta + delta).min(score::INFINITY);
                alpha = score - delta;
                consecutive_fail_high += 1;
            } else {
                return score;
            }

            delta *= 2;
        }
    }
}
