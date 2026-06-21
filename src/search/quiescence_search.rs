use crate::{
    core::{MoveGenType, MoveList, Score, generate_moves, score},
    search::{Bound, Entry, MAX_PLY},
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
        if self.should_stop_search() {
            return score::ZERO;
        }

        self.update_analytics(ply);

        // Base case: to avoid infinite recursion and stack overflow from perpetual
        // checks
        if ply >= MAX_PLY as u8 {
            return self.pos.evaluate();
        }

        if let Some(rule_score) = self.pos.rule_judge(ply) {
            return rule_score;
        }

        let in_check = self.pos.is_in_check();

        let alpha_orig = alpha;
        let mut best_score = -score::INFINITY;

        // Standing pat: static evaluation provides the lower bound for non-check nodes.
        if !in_check {
            best_score = self.pos.evaluate();

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

        let mut best_move = None;

        let tt_value = self.shared.transposition_table.probe(self.pos.hash(), ply);
        if let Some(value) = &tt_value {
            if value.is_cutoff(alpha, beta, depth) {
                return value.score;
            }
            // Even though the TT entry is not deep enough to be directly used, we can still
            // use the best move for move ordering and singular extensions.
            best_move = value.best_move;
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
            self.pos.undo_move(m);

            if !self.shared.keep_running.get() {
                return score::ZERO;
            }

            if score > best_score {
                best_score = score;
                best_move = Some(m);

                if score > alpha {
                    alpha = score;
                }
            }
            if alpha >= beta {
                // beta cutoff
                break;
            }
        }

        let flag = Bound::with_score(best_score, alpha_orig, beta);
        self.shared.transposition_table.store(
            self.pos.hash(),
            ply,
            tt_value!(best_score, best_move, flag, depth),
        );

        best_score
    }
}
