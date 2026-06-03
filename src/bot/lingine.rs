//! Standard `Engine` implementation wrapping Lingine's search and evaluation.
//!
//! This module coordinates the UCI command inputs to drive the engine:
//! 1. **Time Management (`calculate_search_time`)**: Allocates time budgets
//!    based on remaining time, increments, remaining plies, and leaves a safe
//!    time-buffer (to avoid timeouts from GUI latency).
//! 2. **Iterative Deepening Search Loop (`go`)**: Coordinates iterative
//!    deepening from depth 1 upwards, updating aspiration windows, sorting
//!    moves at the root, checking stop flags, and sending real-time progress
//!    info (`UciInfo`) back via mpsc channels.
//! 3. **Search Fallbacks**: Ensures only legal moves are played and falls back
//!    to first-available legal move in case of emergency search errors or hash
//!    collisions.

use std::sync::mpsc::Sender;

use anyhow::{Result, anyhow};

use crate::core::{File, Move, MoveGenType, MoveList, Position, Rank, Square, generate_moves};
use crate::search::{TranspositionTable, search};
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
    transposition_table: TranspositionTable,
    /// Generational sequence age to track outdated transposition table entries.
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

        let (best_move, _score, _nodes) = search(
            self.position.clone(),
            params,
            &mut self.transposition_table,
            self.age,
            tx,
        );

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
