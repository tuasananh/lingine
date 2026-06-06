use std::sync::mpsc::Sender;
use std::time::Duration;

use anyhow::{Result, anyhow};
use strum::EnumCount;

use crate::core::{
    Color, File, MAX_DEPTH, Move, MoveGenType, MoveList, Position, Rank, Square, generate_moves,
};
use crate::search::{Search, SearchParameters, TranspositionTable};
use crate::uci::{
    BestMove, Engine, GoParameters, PositionParameters, RegisterParameters, SetOptionParameters,
    UciId, UciInfo, UciOption,
};

/// A real, functional [`Engine`] implementation mapping to our search and
/// evaluation.
pub struct Lingine {
    /// Internal board state tracker.
    position: Position,
    /// Transposition table to cache evaluated search nodes.
    ///
    /// Find out more: https://www.chessprogramming.org/Transposition_Table
    transposition_table: TranspositionTable,
    /// Generational sequence age to track outdated transposition table entries,
    /// used in [`crate::search::TranspositionTable::store`] to determine
    /// whether to replace an existing entry.
    age: u8,
}

impl Default for Lingine {
    fn default() -> Self {
        Self {
            position: Position::default(),
            transposition_table: TranspositionTable::new(16), // Default to 16 MB
            age: 0,
        }
    }
}

impl Lingine {
    /// Creates a new EngineBot instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Determines the time limit budget for a given search color/increments.
    fn calculate_search_time(params: &GoParameters, side: Color) -> Option<Duration> {
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

            // Safety buffer: reserve at least 50ms or 10% of remaining time, whichever is
            // smaller, to account for process/communication latency.
            let buffer = Duration::from_millis(50).min(time / 10);
            let limit = time.saturating_sub(buffer);

            // Ensure we allocate at least 10ms (or the remaining limit if it's even
            // smaller)
            let min_time = Duration::from_millis(10).min(limit);

            Some(allocated.min(limit).max(min_time))
        } else {
            None
        }
    }
}

impl Engine for Lingine {
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
        // Nothing to do here
        // The UCI Handler will send "readyok" response after this method
        // returns
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

    fn position(&mut self, position: PositionParameters) -> Result<()> {
        log::debug!("position fen={:?} moves={:?}", position.fen, position.moves);

        // Parse starting FEN
        self.position.set(&position.fen)?;

        // Apply moves iteratively
        for uci_mv in position.moves {
            if uci_mv.is_null() {
                continue;
            }

            // Find matching move inside our generated legal moves list to guarantee
            // validity
            let mut moves = MoveList::new();
            generate_moves(&self.position, MoveGenType::Legal, &mut moves);

            let matched = moves.iter().find(|m| {
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
                    Move::new(
                        Square::from_file_rank(
                            File::from_repr(uci_mv.src_file()).unwrap(),
                            Rank::from_repr(uci_mv.src_rank()).unwrap(),
                        ),
                        Square::from_file_rank(
                            File::from_repr(uci_mv.dst_file()).unwrap(),
                            Rank::from_repr(uci_mv.dst_rank()).unwrap(),
                        ),
                    )
                    .to_uci_string()
                ));
            }
        }

        Ok(())
    }

    fn go(&mut self, params: GoParameters, tx: Sender<UciInfo>) -> Result<BestMove> {
        // Increment the age generation at the start of a search session
        self.age = self.age.wrapping_add(1);

        // Calculate time limit for this search based on the provided parameters and the
        // side to move. This will help us determine how long to search before returning
        // a best move.
        let time_limit = Self::calculate_search_time(&params, self.position.side_to_move());

        let max_depth = params.depth.unwrap_or(MAX_DEPTH as u32) as i8;

        let mut history_table = [[[0i32; Square::COUNT]; Square::COUNT]; Color::COUNT];

        let (_score, best_move, _nodes) = Search::start_search(SearchParameters {
            pos: self.position.clone(),
            allocated_time: time_limit,
            stop: params.stop.clone(),
            max_depth,
            transposition_table: &mut self.transposition_table,
            history_table: &mut history_table,
            tx,
            age: self.age,
        });

        // Returns the best move found in UCI format. Pondering is not implemented in
        // this version, so we return `None` for the ponder move.
        Ok(BestMove {
            mv: best_move.to_uci_string(),
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
