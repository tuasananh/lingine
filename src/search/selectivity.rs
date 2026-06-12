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
                    ..Default::default()
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

    /// Performs Null Move Pruning (NMP).
    ///
    /// NMP passes the turn to the opponent (a "null move") and performs a search at
    /// a reduced depth. If the resulting score is still >= beta, we can prune the
    /// current node under the assumption that the position is so strong that the
    /// opponent cannot prevent a beta cutoff even if we do nothing.
    ///
    /// At higher depths (>= 12 plies), NMP performs a "Verification Search" to prevent
    /// pruning deep mating threats or tactical surprises. NMP is disabled recursively
    /// during verification by setting `nmp_min_ply`.
    #[inline]
    pub(super) fn null_move_pruning(
        &mut self,
        depth: i8,
        ply: u8,
        beta: Score,
        eval: Score,
        ctx: SearchContext,
    ) -> Option<Score> {
        // Minimum depth required to attempt NMP.
        const NMP_DEPTH_THRESHOLD: i8 = 3;
        // Base depth reduction plies.
        const NMP_REDUCTIONS: i8 = 3;
        // Divisor for dynamic depth reduction scale: R = NMP_REDUCTIONS + depth / NMP_DIVISOR
        const NMP_DIVISOR: i8 = 6;
        // Depth threshold where verification search is triggered.
        const NMP_VERIFICATION_DEPTH: i8 = 12;

        // Check if NMP preconditions are met:
        // 1. Depth is high enough.
        // 2. We are not already inside a null move search (to prevent infinite null move chains).
        // 3. No move is excluded (to avoid interfering with singular extensions).
        // 4. Static evaluation is high enough relative to beta (scaled down from Pikafish).
        // 5. The side to move has attacking pieces (Rook, Knight, Cannon) to avoid zugzwang.
        // 6. Current ply is above nmp_min_ply (not currently in verification search bounds).
        if depth >= NMP_DEPTH_THRESHOLD
            && !ctx.is_null_move_search
            && ctx.excluded_move.is_none()
            && eval >= beta - 4 * depth as i32 + 100
            && self.pos.has_attacking_pieces(self.pos.side_to_move())
            && ply >= self.nmp_min_ply
        {
            let r = NMP_REDUCTIONS + depth / NMP_DIVISOR;

            self.pos.do_null_move();

            let null_ctx = SearchContext {
                is_null_move_search: true,
                excluded_move: None,
            };

            let null_score =
                -self.negamax::<false, false>(depth - 1 - r, ply + 1, -beta, -beta + 1, null_ctx);

            // Undo the null move
            self.pos.undo_null_move();

            if null_score >= beta {
                // Avoid returning unproven mate scores from NMP
                let pruned_score = if score::is_winning(null_score) {
                    beta
                } else {
                    null_score
                };

                // At high depths (>= 12), perform verification search with NMP disabled
                // for the next few plies.
                if self.nmp_min_ply == 0
                    && depth >= NMP_VERIFICATION_DEPTH
                    && !score::is_losing(beta)
                {
                    // Disable NMP recursively up to nmp_min_ply:
                    self.nmp_min_ply = ply + (3 * (depth - 1 - r).max(0) / 4) as u8;

                    // Verify the cutoff using a normal search window [beta - 1, beta]
                    let v = self.negamax::<false, false>(depth - 1 - r, ply, beta - 1, beta, ctx);

                    // Restore nmp_min_ply to 0
                    self.nmp_min_ply = 0;

                    // If verification search also fails high, we successfully prune!
                    if v >= beta {
                        return Some(pruned_score);
                    }
                } else {
                    // For lower depths, prune immediately.
                    return Some(pruned_score);
                }
            }
        }

        None
    }
}
