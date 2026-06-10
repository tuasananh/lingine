use crate::{
    core::{Score, score},
    search::{Bound, Entry, SearchContext},
};

impl super::Searcher<'_> {
    /// Calculates the margin threshold required for singular extensions.
    #[inline]
    fn singular_margin(depth: i8) -> Score {
        2 * depth as Score
    }

    #[inline]
    pub(super) fn singular_extension(
        &mut self,
        depth: i8,
        ply: u8,
        ctx: &SearchContext,
        value: &Entry,
    ) -> bool {
        const SINGULAR_EXTENSION_DEPTH_THRESHOLD: i8 = 8;
        const SINGULAR_EXTENSION_DEPTH_REDUCTION: i8 = 3;
        // Singular Extensions: We want to check if TT move is a critical move, and that
        // removing it would cause us to be at a really bad position (score
        // drops significantly below beta). If so, we want to extend the
        // search to give the engine a better change to evaluate this move.
        //
        // See: https://www.chessprogramming.org/Singular_Extensions
        if depth >= SINGULAR_EXTENSION_DEPTH_THRESHOLD
            && ctx.excluded_move.is_none()
            && value.depth >= depth - SINGULAR_EXTENSION_DEPTH_REDUCTION
            && value.bound != Bound::Alpha
            && !score::is_winning(value.score.abs())
        {
            assert!(
                value.best_move.is_some(),
                "Not alpha flag means there is a best move, hopefully"
            );
            let rdepth = depth - SINGULAR_EXTENSION_DEPTH_REDUCTION;
            let rbeta = value.score - Self::singular_margin(depth);
            let score = self.negamax::<false, false>(
                rdepth,
                ply,
                rbeta - 1,
                rbeta,
                SearchContext {
                    excluded_move: value.best_move,
                },
            );

            // Score fails low, that means the TT move is really good, and that the
            // alternatives are much worse. We should extend this node to find the best move
            // after this critical move.
            score < rbeta
        } else {
            false
        }
    }
}
