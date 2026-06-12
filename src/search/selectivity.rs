use crate::{
    core::{Move, Score, score},
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

    #[inline]
    pub(super) fn calculate_reductions<const PV: bool>(
        &self,
        mv: Move,
        moves_played: u8,
        depth: i8,
        ply: u8,
    ) -> i8 {
        // The idea is that for later moves, they will probably be less promising,
        // thus we reduce the depth that we search for this move.
        const LMR_MOVES_PLAYED_THRESHOLD: u8 = 2;
        const LMR_DEPTH_THRESHOLD: i8 = 3;
        const LMR_BASE: f32 = 0.75;
        const LMR_DIVISOR: f32 = 2.3;
        const LMR_HISTORY_DIVISOR: i32 = 20000;
        if moves_played >= LMR_MOVES_PLAYED_THRESHOLD
            && depth >= LMR_DEPTH_THRESHOLD
            && self.pos.is_quiet(mv)
            && !self.pos.is_in_check(self.pos.side_to_move())
            && !self.killer_moves.contains(mv, ply)
            && !self.pos.gives_check(mv)
        {
            let mut reductions = (LMR_BASE
                + f32::from(moves_played).ln() * f32::from(depth).ln() / LMR_DIVISOR)
                as i8;
            // We reduce reductions for promising quiet moves from history
            reductions -= (self.shared.history_moves.get(self.pos.side_to_move(), mv)
                / LMR_HISTORY_DIVISOR) as i8;

            // We reduce PV nodes less
            reductions -= i8::from(PV);

            reductions.clamp(0, depth - 1)
        } else {
            0
        }
    }
}
