use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

use crate::core::movegen::generate_moves;
use crate::core::{
    Position,
    types::{Color, File, MAX_MOVES, Move, MoveGenType, Rank, Square},
};
use crate::search::{INFINITY, MATE_VALUE, SearchContext, negamax};
use crate::uci::{
    BestMove, Engine, GoParameters, RegisterParameters, SetOptionParameters, UciId, UciInfo,
    UciOption, UciPosition, UciScore, UciScoreBound,
};

/// Helper to format an internal `Move` to its UCI algebraic string format (e.g., `"a0b1"`).
fn format_move(m: Move) -> String {
    if m.is_none() {
        return "0000".to_string();
    }
    let from = m.square_from();
    let to = m.square_to();
    let from_file = (b'a' + from.file() as u8) as char;
    let from_rank = (b'0' + from.rank() as u8) as char;
    let to_file = (b'a' + to.file() as u8) as char;
    let to_rank = (b'0' + to.rank() as u8) as char;
    format!("{}{}{}{}", from_file, from_rank, to_file, to_rank)
}

/// A real, functional [`Engine`] implementation mapping to our search and evaluation.
#[derive(Default)]
pub struct EngineBot {
    /// Internal board state tracker.
    position: Position,
}

impl EngineBot {
    /// Creates a new EngineBot instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Determines the time limit budget for a given search color/increments.
    fn calculate_search_time(&self, params: &GoParameters, side: Color) -> Option<Duration> {
        if let Some(movetime) = params.movetime {
            return Some(movetime.saturating_sub(Duration::from_millis(50)));
        }

        let (time_left, inc) = match side {
            Color::White => (params.wtime, params.winc),
            Color::Black => (params.btime, params.binc),
        };

        if let Some(time) = time_left {
            let inc_val = inc.unwrap_or(Duration::ZERO);
            // spend 5% of time left + 50% of increment
            let allocated = time / 20 + inc_val / 2;
            // safety margin
            let limit = time.saturating_sub(Duration::from_millis(100));
            Some(allocated.min(limit).max(Duration::from_millis(50)))
        } else {
            None
        }
    }
}

impl Engine for EngineBot {
    fn uci(&self) -> (UciId, Vec<UciOption>) {
        let id = UciId {
            name: "Lingine".into(),
            author: "tuasananh".into(),
        };
        let options = vec![
            UciOption::Spin {
                name: "Hash".into(),
                default: 16,
                min: 1,
                max: 1024,
            },
            UciOption::Check {
                name: "Ponder".into(),
                default: false,
            },
        ];
        (id, options)
    }

    fn debug(&self, is_on: bool) {
        log::debug!("debug mode: {is_on}");
    }

    fn isready(&self) {
        // Nothing to prepare yet
    }

    fn setoption(&mut self, params: SetOptionParameters) -> Result<()> {
        log::debug!("setoption: name={:?} value={:?}", params.name, params.value);
        Ok(())
    }

    fn ucinewgame(&mut self) {
        log::debug!("ucinewgame");
        self.position = Position::new();
    }

    fn register(&self, params: RegisterParameters) {
        match params {
            RegisterParameters::Later => log::debug!("register later"),
            RegisterParameters::Identity { name, code } => {
                log::debug!("register name={name:?} code={code:?}");
            }
        }
    }

    fn position(&mut self, position: UciPosition) -> Result<()> {
        log::debug!("position fen={:?} moves={:?}", position.fen, position.moves);

        // Parse starting FEN
        self.position.set(&position.fen)?;

        // Apply moves iteratively
        for uci_mv in position.moves {
            if uci_mv.is_null() {
                continue;
            }

            // Find matching move inside our generated legal moves list to guarantee validity
            let mut moves = [Move::none(); MAX_MOVES];
            let count = generate_moves(&self.position, MoveGenType::Legal, &mut moves);

            let matched = moves[..count].iter().find(|m| {
                let from = m.square_from();
                let to = m.square_to();
                from.file() as u8 == uci_mv.src_file()
                    && from.rank() as u8 == uci_mv.src_rank()
                    && to.file() as u8 == uci_mv.dst_file()
                    && to.rank() as u8 == uci_mv.dst_rank()
            });

            if let Some(&mv) = matched {
                self.position.do_move(mv);
            } else {
                return Err(anyhow!(
                    "Illegal move sequence requested in position command: {:?}",
                    format_move(Move::new(
                        Square::from_file_rank(
                            File::from_repr(uci_mv.src_file()).unwrap(),
                            Rank::from_repr(uci_mv.src_rank()).unwrap(),
                        ),
                        Square::from_file_rank(
                            File::from_repr(uci_mv.dst_file()).unwrap(),
                            Rank::from_repr(uci_mv.dst_rank()).unwrap(),
                        ),
                    ))
                ));
            }
        }

        Ok(())
    }

    fn go(&mut self, params: GoParameters, tx: Sender<UciInfo>) -> Result<BestMove> {
        let mut pos = self.position.clone();
        let start_time = Instant::now();
        let mut nodes = 0u64;

        let max_depth = params.depth.unwrap_or(100) as i32;
        let time_limit = self.calculate_search_time(&params, pos.side_to_move());

        let mut best_move = Move::none();

        for depth in 1..=max_depth {
            if params.stop.load(Ordering::Relaxed) {
                break;
            }

            // Check if we have spent >50% of the allowed time to avoid timing out in next ply
            if let Some(limit) = time_limit
                && start_time.elapsed() > limit / 2
            {
                break;
            }

            let mut depth_best_move = Move::none();
            let mut best_score = -INFINITY;

            let mut moves = [Move::none(); MAX_MOVES];
            let count = generate_moves(&pos, MoveGenType::Legal, &mut moves);

            if count == 0 {
                break;
            }

            // Search root moves
            let mut alpha = -INFINITY;
            let beta = INFINITY;

            // Sort root moves to maximize alpha-beta pruning (captures first)
            for i in 0..count {
                let mut best_idx = i;
                let mut best_val = if !pos.is_empty(moves[i].square_to()) {
                    1
                } else {
                    0
                };
                for (j, mv) in moves.iter().enumerate().take(count).skip(i + 1) {
                    let val = if !pos.is_empty(mv.square_to()) { 1 } else { 0 };
                    if val > best_val {
                        best_val = val;
                        best_idx = j;
                    }
                }
                if best_idx != i {
                    moves.swap(i, best_idx);
                }
            }

            let mut ctx = SearchContext {
                stop: &params.stop,
                nodes: &mut nodes,
                start_time,
                time_limit,
            };

            for m in moves.iter().copied().take(count) {
                pos.do_move(m);
                let score = -negamax(&mut pos, depth - 1, 1, -beta, -alpha, &mut ctx);
                pos.undo_move(m);

                if score > best_score {
                    best_score = score;
                    depth_best_move = m;
                }
                if score > alpha {
                    alpha = score;
                }
            }

            // If the search was not aborted, save search outcomes and print UCI progress
            if !params.stop.load(Ordering::Relaxed) {
                if !depth_best_move.is_none() {
                    best_move = depth_best_move;
                }

                let pv_vec = vec![format_move(best_move)];
                let time_elapsed = start_time.elapsed();
                let nps = if time_elapsed.as_secs_f64() > 0.001 {
                    Some((nodes as f64 / time_elapsed.as_secs_f64()) as u64)
                } else {
                    None
                };

                let uci_score = if best_score.abs() > MATE_VALUE - 100 {
                    let mate_plies = MATE_VALUE - best_score.abs();
                    let mate_moves = (mate_plies + 1) / 2;
                    let sign = if best_score > 0 { 1 } else { -1 };
                    UciScoreBound {
                        score: UciScore::Mate(sign * mate_moves),
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
                    nodes: Some(nodes),
                    time: Some(time_elapsed),
                    nps,
                    score: Some(uci_score),
                    pv: Some(pv_vec),
                    ..UciInfo::new()
                };

                tx.send(info).ok();
            }
        }

        let best_move_str = if best_move.is_none() {
            // Pick first legal move as fallback
            let mut moves = [Move::none(); MAX_MOVES];
            let count = generate_moves(&self.position, MoveGenType::Legal, &mut moves);
            if count > 0 {
                format_move(moves[0])
            } else {
                "0000".to_string()
            }
        } else {
            format_move(best_move)
        };

        Ok(BestMove {
            mv: best_move_str,
            ponder: None,
        })
    }

    fn stop(&mut self) {
        log::debug!("stop");
    }

    fn ponderhit(&mut self) {
        log::debug!("ponderhit");
    }

    fn quit(&self) {
        log::debug!("quit");
    }
}
