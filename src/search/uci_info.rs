use crate::{
    core::{Move, MoveGenType, MoveList, Score, generate_moves, score},
    uci::{UciInfo, UciScore, UciScoreBound},
};

impl super::Searcher<'_> {
    /// Traverses the transposition table to extract the predicted line of moves
    /// (Principal Variation).
    fn extract_pv(&self, depth: i8, best_move: Move) -> Option<Vec<Move>> {
        if best_move.is_null() {
            return None;
        }
        let mut pv = Vec::new();
        pv.push(best_move);

        let mut current_pos = self.pos.clone();
        current_pos.do_move(best_move);

        let mut visited = std::collections::HashSet::new();
        visited.insert(current_pos.zobrist_hash());

        for ply in 1..depth as u8 {
            if let Some(entry) = self
                .shared
                .transposition_table
                .probe(current_pos.zobrist_hash(), ply)
            {
                let m = entry.best_move;
                // Here we do not need to check for null move
                // since we know that a non-empty TT entry must have a valid move.
                let mut moves = MoveList::new();
                generate_moves(&current_pos, MoveGenType::Legal, &mut moves);
                // The TT move might be illegal
                if !moves.contains(&m) {
                    break;
                }
                pv.push(m);
                current_pos.do_move(m);

                let hash = current_pos.zobrist_hash();
                if visited.contains(&hash) {
                    break;
                }
                visited.insert(hash);
            } else {
                break;
            }
        }

        Some(pv)
    }

    /// Sends UCI info updates back to the main thread after each completed
    /// depth iteration, including the best move, score, principal
    /// variation, nodes searched, time taken, and NPS.
    pub(super) fn send_uci_info(&self, depth: i8, best_score: Score, best_move: Move) {
        let pv_vec = self.extract_pv(depth, best_move);
        let time_elapsed = self.time_manager.executed_time();
        let nps = if time_elapsed.as_secs_f64() > 0.001 {
            Some((self.nodes as f64 / time_elapsed.as_secs_f64()) as u64)
        } else {
            None
        };

        let uci_score = if let Some(mate_plies) = score::ply_to_mate_or_mated(best_score) {
            let mate_moves = mate_plies.div_ceil(2);
            let sign: i32 = if best_score > 0 { 1 } else { -1 };
            UciScoreBound {
                score: UciScore::Mate(sign * mate_moves as i32),
                bound: None,
            }
        } else {
            UciScoreBound {
                score: UciScore::Centipawns(best_score),
                bound: None,
            }
        };

        let info = UciInfo {
            depth: Some(depth as u32),
            seldepth: Some(self.max_ply as u32),
            nodes: Some(self.nodes),
            time: Some(time_elapsed),
            nps,
            hashfull: Some(self.shared.transposition_table.hashfull()),
            score: Some(uci_score),
            pv: pv_vec.map(|pv| pv.into_iter().map(|m| m.to_uci_string()).collect()),
            ..UciInfo::new()
        };

        println!("{info}");
    }
}
