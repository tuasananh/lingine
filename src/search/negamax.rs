use crate::{
    core::{Move, MoveGenType, MoveList, Score, generate_moves, score},
    search::{Bound, Entry, MAX_PLY, SearchContext},
    tt_value,
};

impl super::Searcher<'_> {
    /// Performs Fail-Soft Alpha-Beta Negamax Search to a specific depth.
    pub(super) fn negamax<const ROOT: bool, const PV: bool>(
        &mut self,
        depth: i8,
        ply: u8,
        mut alpha: Score,
        beta: Score,
        ctx: SearchContext,
    ) -> Score {
        if self.should_stop_search() {
            return score::ZERO;
        }

        self.pv_table.clear_line(ply);

        if ply >= MAX_PLY as u8 {
            return self.pos.evaluate();
        }

        // Game over / rule evaluations (60-move rule, insufficient material,
        // repetitions, perpetual checks)
        if let Some(rule_score) = self.pos.rule_judge(ply) {
            return rule_score;
        }

        let alpha_orig = alpha;

        let mut best_score = -score::INFINITY;
        let mut best_move = Move::NULL;

        let mut depth = depth;

        // In general, we should do extensions before probing TT, since the depth might
        // increase after extensions, and we want to probe TT with the correct depth.

        // Check Extensions: If the side to move is in check,
        // we extend the search depth by 1 ply to give the engine a better chance to
        // find a defensive resource and avoid missing critical moves that could
        // save the King.
        //
        // See: https://www.chessprogramming.org/Check_Extensions
        if self.pos.is_in_check(self.pos.side_to_move()) {
            depth += 1;
        }

        // Base case: fall back to quiescence search
        if depth == 0 {
            return self.quiescence_search(0, ply, alpha, beta);
        }

        // Update analytics after quiescence to avoid counting same nodes twice
        self.update_analytics(ply);

        let mut is_singular = false;
        let tt_value = self
            .shared
            .transposition_table
            .probe(self.pos.zobrist_hash(), ply);
        if let Some(value) = &tt_value {
            if !PV && value.is_cutoff(alpha, beta, depth) {
                return value.score;
            }
            // Even though the TT entry is not deep enough to be directly used, we can still
            // use the best move for move ordering and singular extensions.
            best_move = value.best_move;

            is_singular = self.singular_extension(depth, ply, &ctx, value);
        };

        let mut moves = MoveList::new();
        generate_moves(&self.pos, MoveGenType::Legal, &mut moves);

        // Stalemate / Checkmate: In Xiangqi, a player with no legal moves loses.
        if moves.is_empty() {
            if ROOT {
                eprintln!(
                    "Warning: Root node has no legal moves, this should not happen in a normal game"
                );
            }
            return score::mated_in(ply);
        }

        // One Reply Extensions: If there is only one legal move available,
        // we extend the search, as it is likely a critical position.
        //
        // See: https://www.chessprogramming.org/One_Reply_Extensions
        if moves.len() == 1 && ctx.excluded_move.is_null() {
            depth += 1;
        }

        // Sort moves: prioritize captures via MVV-LVA Heuristic, with TT move
        // prioritized first, killers, and history
        self.sort_moves(&mut moves, best_move, ply);
        let mut moves_played = 0;

        for m in moves {
            if m == ctx.excluded_move {
                continue;
            }

            self.pos.do_move(m);
            let score = if moves_played == 0 {
                -self.negamax::<false, PV>(
                    depth - 1 + is_singular as i8,
                    ply + 1,
                    -beta,
                    -alpha,
                    SearchContext::default(),
                )
            } else {
                self.pv_search::<PV>(depth, ply, alpha, beta)
            };
            self.pos.undo_move();
            moves_played += 1;

            if !self.shared.keep_running.get() {
                return score::ZERO;
            }

            if score > best_score {
                best_score = score;
                best_move = m;

                if score > alpha {
                    alpha = score;
                    self.pv_table.update_best_move(ply, best_move);
                }
            }

            if alpha >= beta {
                // Update killers and history for quiet moves
                if self.pos.is_empty(m.to()) {
                    self.killer_moves.update(m, ply);
                    self.shared
                        .history_moves
                        .increase(self.pos.side_to_move(), m, depth);
                }
                break; // Beta cutoff
            }
        }

        let bound = Bound::with_score(best_score, alpha_orig, beta);
        self.shared.transposition_table.store(
            self.pos.zobrist_hash(),
            ply,
            tt_value!(best_score, best_move, bound, depth),
        );

        best_score
    }
}
