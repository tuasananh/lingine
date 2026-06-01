use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

use crate::core::movegen::generate_moves;
use crate::core::{
    Position,
    types::{Color, File, MAX_MOVES, Move, MoveGenType, Rank, Square},
};
use crate::search::{
    INFINITY, MATE_VALUE, SearchContext, SearchExtension, SearchWindow, TranspositionTable,
    negamax,
};
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
pub struct EngineBot {
    /// Internal board state tracker.
    position: Position,
    /// Transposition table to cache evaluated search nodes.
    transposition_table: TranspositionTable,
    /// Generational sequence age to track outdated transposition table entries.
    age: u8,
}

impl Default for EngineBot {
    fn default() -> Self {
        Self {
            position: Position::default(),
            transposition_table: TranspositionTable::new(16), // Default to 16 MB
            age: 0,
        }
    }
}

impl EngineBot {
    /// Creates a new EngineBot instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Determines the time limit budget for a given search color/increments.
    fn calculate_search_time(&self, params: &GoParameters, side: Color) -> Option<Duration> {
        if let Some(movetime) = params.movetime {
            return Some(movetime.saturating_sub(Duration::from_millis(10)));
        }

        let (time_left, inc) = match side {
            Color::White => (params.wtime, params.winc),
            Color::Black => (params.btime, params.binc),
        };

        if let Some(time) = time_left {
            let inc_val = inc.unwrap_or(Duration::ZERO);

            // Determine divisor based on movestogo, default to 20
            let divisor = if let Some(movestogo) = params.movestogo {
                movestogo.get() as u64
            } else {
                20
            };

            // Basic allocation: time_left / divisor + inc / 2
            let allocated = time / divisor as u32 + inc_val / 2;

            // Safety buffer: reserve at least 50ms or 10% of remaining time, whichever is smaller,
            // to account for process/communication latency.
            let buffer = Duration::from_millis(50).min(time / 10);
            let limit = time.saturating_sub(buffer);

            // Ensure we allocate at least 10ms (or the remaining limit if it's even smaller)
            let min_time = Duration::from_millis(10).min(limit);

            Some(allocated.min(limit).max(min_time))
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
        let options = vec![UciOption::Spin {
            name: "Hash".into(),
            default: 16,
            min: 1,
            max: 1024,
        }];
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
        if params.name.to_lowercase() == "hash"
            && let Some(val) = params.value
            && let Ok(mb_size) = val.parse::<usize>()
        {
            self.transposition_table.resize(mb_size);
            log::info!("Resized transposition table to {} MB", mb_size);
        }
        Ok(())
    }

    fn ucinewgame(&mut self) {
        log::debug!("ucinewgame");
        self.position = Position::new();
        self.transposition_table.clear();
        self.age = 0;
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

        // Increment the age generation at the start of a search session
        self.age = self.age.wrapping_add(1);

        let mut best_move = Move::none();
        let mut last_depth_score = -INFINITY;

        let mut killers = [[Move::none(); 2]; 128];
        let mut history_table = [[[0; 90]; 90]; 2];

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

            let mut moves = [Move::none(); MAX_MOVES];
            let count = generate_moves(&pos, MoveGenType::Legal, &mut moves);

            if count == 0 {
                break;
            }

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

            let mut best_score;
            let mut depth_best_move;

            // Aspiration Windows Setup
            let mut alpha = -INFINITY;
            let mut beta = INFINITY;
            let mut delta = 25; // aspiration window size in centipawns

            if depth >= 5 && last_depth_score.abs() < MATE_VALUE - 1000 {
                alpha = last_depth_score - delta;
                beta = last_depth_score + delta;
            }

            loop {
                let search_alpha = alpha.max(-INFINITY);
                let search_beta = beta.min(INFINITY);

                let mut ctx = SearchContext {
                    stop: &params.stop,
                    nodes: &mut nodes,
                    start_time,
                    time_limit,
                    transposition_table: &mut self.transposition_table,
                    age: self.age,
                    killers: &mut killers,
                    history_table: &mut history_table,
                };

                let mut curr_alpha = search_alpha;
                best_score = -INFINITY;
                depth_best_move = Move::none();

                for m in moves.iter().copied().take(count) {
                    if params.stop.load(Ordering::Relaxed) {
                        break;
                    }
                    pos.do_move(m);
                    let score = -negamax(
                        &mut pos,
                        depth - 1,
                        1,
                        SearchWindow::new(-search_beta, -curr_alpha),
                        SearchExtension::default(),
                        &mut ctx,
                    );
                    pos.undo_move(m);

                    if params.stop.load(Ordering::Relaxed) {
                        break;
                    }

                    if score > best_score {
                        best_score = score;
                        depth_best_move = m;
                    }
                    if score > curr_alpha {
                        curr_alpha = score;
                    }
                }

                if params.stop.load(Ordering::Relaxed) {
                    break;
                }

                // If window was already full (-INFINITY, INFINITY), we stop, no re-search.
                if search_alpha == -INFINITY && search_beta == INFINITY {
                    break;
                }

                // Check fail-low / fail-high
                if best_score <= search_alpha {
                    // Fail low: score worse or equal to alpha. Widen alpha.
                    alpha -= delta;
                    beta = best_score + delta;
                    delta = delta.saturating_mul(2);
                } else if best_score >= search_beta {
                    // Fail high: score better or equal to beta. Widen beta.
                    beta += delta;
                    alpha = best_score - delta;
                    delta = delta.saturating_mul(2);
                } else {
                    // Stable score inside window!
                    break;
                }
            }

            if params.stop.load(Ordering::Relaxed) {
                break;
            }

            last_depth_score = best_score;

            // If the search was not aborted, save search outcomes and print UCI progress
            if !params.stop.load(Ordering::Relaxed) {
                if !depth_best_move.is_none() {
                    best_move = depth_best_move;
                }

                let pv_vec = if best_move.is_none() {
                    None
                } else {
                    Some(vec![format_move(best_move)])
                };
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
                    pv: pv_vec,
                    ..UciInfo::new()
                };

                tx.send(info).ok();
            }
        }

        // Safety check: validate best_move is legal in the current position.
        // This guards against rare TT corruption, hash collisions, or search bugs
        // that could otherwise cause the engine to output an illegal move.
        let mut legal_moves = [Move::none(); MAX_MOVES];
        let legal_count = generate_moves(&self.position, MoveGenType::Legal, &mut legal_moves);

        let best_move_str = if best_move.is_none()
            || !legal_moves[..legal_count].contains(&best_move)
        {
            if !best_move.is_none() {
                log::warn!(
                    "bestmove {} is not legal in current position — falling back to first legal move",
                    format_move(best_move)
                );
            }
            // Pick first legal move as fallback
            if legal_count > 0 {
                format_move(legal_moves[0])
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
