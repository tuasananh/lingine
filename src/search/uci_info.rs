use crate::{
    core::{Score, score},
    uci::{UciInfo, UciScore, UciScoreBound},
};

impl super::Searcher<'_> {
    /// Sends UCI info updates back to the main thread after each completed
    /// depth iteration, including the best move, score, principal
    /// variation, nodes searched, time taken, and NPS.
    pub(super) fn send_uci_info(&self, depth: i8, best_score: Score) {
        let pv = self.pv_table.get_line(0).to_vec();
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
            pv: Some(pv.into_iter().map(|m| m.to_uci_string()).collect()),
            ..UciInfo::new()
        };

        println!("{info}");
    }
}
