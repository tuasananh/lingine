use crate::{
    core::{Move, MoveGenType, MoveList, Score, generate_moves, score},
    search::{SearchContext, TranspositionTableFlag, TranspositionTableValue},
    tt_value,
};

/// Calculates the margin threshold required for singular extensions.
#[inline]
fn singular_margin(depth: i8) -> Score {
    2 * depth as i16
}

impl super::Searcher<'_> {
    /// Performs Fail-Soft Alpha-Beta Negamax Search to a specific depth.
    pub(super) fn negamax(
        &mut self,
        depth: i8,
        ply: u8,
        mut alpha: Score,
        beta: Score,
        mut ctx: SearchContext,
    ) -> Score {
        self.update_analytics(ply);

        if self.should_stop_search() {
            return score::ZERO;
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
        if self.pos.is_in_check(self.pos.side_to_move()) && ctx.extensions < 6 {
            depth += 1;
            ctx.extensions += 1;
        }

        let mut is_singular = false;
        let tt_value = self
            .shared
            .transposition_table
            .probe(self.pos.zobrist_hash(), ply);
        if let Some(value) = &tt_value {
            if value.depth >= depth {
                match value.flag {
                    TranspositionTableFlag::Exact => return value.score,
                    TranspositionTableFlag::Alpha => {
                        if value.score <= alpha {
                            return value.score;
                        }
                    }
                    TranspositionTableFlag::Beta => {
                        if value.score >= beta {
                            return value.score;
                        }
                    }
                    TranspositionTableFlag::Empty => {
                        unreachable!("Empty flag should not be returned by probe")
                    }
                }
            } else {
                // Even though the TT entry is not deep enough to be directly used, we can still
                // use the best move for move ordering and singular extensions.
                best_move = value.best_move;
            }

            // Singular Extensions: We want to check if TT move is a critical move, and that
            // removing it would cause us to be at a really bad position (score
            // drops significantly below beta). If so, we want to extend the
            // search to give the engine a better change to evaluate this move.
            //
            // See: https://www.chessprogramming.org/Singular_Extensions
            if depth >= 8
                && ctx.excluded_move.is_null()
                && ctx.extensions < 6
                && value.depth >= depth - 3
                && value.flag != TranspositionTableFlag::Alpha
                && !score::is_winning(value.score.abs())
            {
                let rdepth = depth - 3;
                let rbeta = value.score - singular_margin(depth);
                let score = self.negamax(
                    rdepth,
                    ply,
                    rbeta - 1,
                    rbeta,
                    SearchContext {
                        extensions: ctx.extensions,
                        excluded_move: value.best_move,
                    },
                );

                // Score fails low, that means the TT move is really good, and that the
                // alternatives are much worse. We should extend this node to find the best move
                // after this critical move.
                if score < rbeta {
                    is_singular = true;
                }
            }
        };

        // Base case: fall back to quiescence search
        if depth == 0 {
            return self.quiescence_search(0, ply, alpha, beta);
        }

        let mut moves = MoveList::new();
        generate_moves(&self.pos, MoveGenType::Legal, &mut moves);

        // Stalemate / Checkmate: In Xiangqi, a player with no legal moves loses.
        if moves.is_empty() {
            return score::mated_in(ply);
        }

        // One Reply Extensions: If there is only one legal move available,
        // we extend the search, as it is likely a critical position.
        //
        // See: https://www.chessprogramming.org/One_Reply_Extensions
        if moves.len() == 1 && ctx.excluded_move.is_null() && ctx.extensions < 6 {
            depth += 1;
            ctx.extensions += 1;
        }

        // Sort moves: prioritize captures via MVV-LVA Heuristic, with TT move
        // prioritized first, killers, and history
        self.sort_moves(&mut moves, best_move, ply);

        for m in moves {
            if m == ctx.excluded_move {
                continue;
            }

            let mut ext = 0;

            if let Some(value) = &tt_value
                && m == value.best_move
                && is_singular
            {
                ext = 1;
            }

            self.pos.do_move(m);
            let score = -self.negamax(
                depth - 1 + ext as i8,
                ply + 1,
                -beta,
                -alpha,
                SearchContext {
                    extensions: (ctx.extensions + ext),
                    ..Default::default()
                },
            );
            self.pos.undo_move();

            if !self.shared.keep_running.get() {
                return score::ZERO;
            }

            if score > best_score {
                best_score = score;
                best_move = m;
            }
            if best_score > alpha {
                alpha = best_score;
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

        // Cache search results to Transposition Table
        if self.shared.keep_running.get() {
            let flag = if best_score >= beta {
                TranspositionTableFlag::Beta
            } else if best_score <= alpha_orig {
                TranspositionTableFlag::Alpha
            } else {
                TranspositionTableFlag::Exact
            };
            self.shared.transposition_table.store(
                self.pos.zobrist_hash(),
                ply,
                tt_value!(best_score, flag, best_move, depth, self.shared.age),
            );
        }

        best_score
    }
}
