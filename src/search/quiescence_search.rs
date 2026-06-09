use crate::{
    core::{Move, MoveGenType, MoveList, Score, Side, generate_moves, score},
    search::{Bound, Entry},
    tt_value,
};

impl super::Searcher<'_> {
    /// Implements Quiescence Search to evaluate tactical capture sequences.
    ///
    /// Prevents the horizon effect by searching captures only until a quiet
    /// position is reached.
    pub(super) fn quiescence_search(
        &mut self,
        depth: i8,
        ply: u8,
        mut alpha: Score,
        beta: Score,
    ) -> Score {
        self.update_analytics(ply);

        if self.should_stop_search() {
            return score::ZERO;
        }

        // Base case: to avoid infinite recursion and stack overflow from perpetual
        // checks
        if depth <= -12 {
            let stand_pat = self.pos.evaluate();
            return if self.pos.side_to_move() == Side::Red {
                stand_pat
            } else {
                -stand_pat
            };
        }

        if let Some(rule_score) = self.pos.rule_judge(ply) {
            return rule_score;
        }

        let in_check = self.pos.is_in_check(self.pos.side_to_move());

        let alpha_orig = alpha;
        let mut best_score = -score::INFINITY;

        // Standing pat: static evaluation provides the lower bound for non-check nodes.
        if !in_check {
            let stand_pat = self.pos.evaluate();
            best_score = if self.pos.side_to_move() == Side::Red {
                stand_pat
            } else {
                -stand_pat
            };

            // If the static evaluation is already good enough to cause a beta cutoff, we
            // can prune this node without searching captures. This is the
            // essence of quiescence search: we only search captures if the
            // position is "noisy" (i.e. in check or has potential captures that
            // could change the evaluation significantly).
            if best_score >= beta {
                return best_score;
            }
            if best_score > alpha {
                alpha = best_score;
            }
        }

        let mut best_move = Move::NULL;

        let tt_value = self
            .shared
            .transposition_table
            .probe(self.pos.zobrist_hash(), ply);
        if let Some(value) = &tt_value {
            if value.depth >= depth {
                match value.bound {
                    Bound::Exact => return value.score,
                    Bound::Alpha => {
                        if value.score <= alpha {
                            return value.score;
                        }
                    }
                    Bound::Beta => {
                        if value.score >= beta {
                            return value.score;
                        }
                    }
                    Bound::Empty => {
                        unreachable!("Empty flag should not be returned by probe")
                    }
                }
            } else {
                // Even though the TT entry is not deep enough to be directly used, we can still
                // use the best move for move ordering and singular extensions.
                best_move = value.best_move;
            }
        };

        let mut moves = MoveList::new();
        generate_moves(
            &self.pos,
            // Generate moves: if in check, we must search all legal evasions to save the
            // King. Otherwise, we only search capture moves.
            if in_check {
                MoveGenType::Legal
            } else {
                MoveGenType::Captures
            },
            &mut moves,
        );

        // Checkmate detection: in check with no legal evasions = checkmate
        if in_check && moves.is_empty() {
            return score::mated_in(ply);
        }

        // Sort captures using MVV-LVA
        self.sort_moves(&mut moves, best_move, ply);

        for m in moves {
            self.pos.do_move(m);
            let score = -self.quiescence_search(depth - 1, ply + 1, -beta, -alpha);
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
                // beta cutoff
                break;
            }
        }

        if self.shared.keep_running.get() {
            let flag = if best_score >= beta {
                Bound::Beta
            } else if best_score <= alpha_orig {
                Bound::Alpha
            } else {
                Bound::Exact
            };
            self.shared.transposition_table.store(
                self.pos.zobrist_hash(),
                ply,
                tt_value!(best_score, best_move, flag, depth),
            );
        }

        best_score
    }
}
