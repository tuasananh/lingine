use std::sync::Arc;

use anyhow::{Result, anyhow};

use crate::core::{File, Move, MoveGenType, MoveList, Position, Rank, Square, generate_moves};
use crate::search::{HistoryMoves, Searcher, SharedContext, TimeManager, TranspositionTable};
use crate::uci::{
    BestMove, Engine, GoParameters, PositionParameters, RegisterParameters, RunningStatus,
    SetOptionParameters, UciId, UciOption,
};

#[derive(Default)]
pub struct Lingine {
    /// Current position of the engine.
    position: Position,
    /// Transposition table for caching results of previously evaluated
    /// positions.
    transposition_table: TranspositionTable,
    /// The history heuristic table, which tracks the effectiveness of quiet
    /// moves.
    history_moves: HistoryMoves,
    /// Shared flag to indicate whether the engine should keep searching
    keep_running: Arc<RunningStatus>,
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
        eprintln!("debug: debug mode: {is_on}");
    }

    fn isready(&self) {
        // Nothing to do here
        // The UCI Handler will send "readyok" response after this method
        // returns
    }

    fn setoption(&mut self, params: SetOptionParameters) -> Result<()> {
        eprintln!(
            "debug: setoption: name={:?} value={:?}",
            params.name, params.value
        );
        if params.name.to_lowercase() == "hash"
            && let Some(val) = params.value
            && let Ok(mb_size) = val.parse::<usize>()
        {
            self.transposition_table.resize(mb_size);
            eprintln!("info: Resized transposition table to {} MB", mb_size);
        }
        Ok(())
    }

    fn ucinewgame(&mut self) {
        eprintln!("debug: ucinewgame");
        self.position = Position::new();
        self.transposition_table.clear();
    }

    fn register(&self, params: RegisterParameters) {
        match params {
            RegisterParameters::Later => eprintln!("debug: register later"),
            RegisterParameters::Identity { name, code } => {
                eprintln!("debug: register name={name:?} code={code:?}");
            }
        }
    }

    fn position(&mut self, position: PositionParameters) -> Result<()> {
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
                let from = m.from();
                let to = m.to();
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

    fn go(&mut self, params: GoParameters) -> Result<BestMove> {
        let time_manager = TimeManager::new(&params, self.position.side_to_move());

        let best_move = Searcher::start_search(
            self.position.clone(),
            time_manager,
            SharedContext {
                keep_running: self.keep_running.clone(),
                transposition_table: &mut self.transposition_table,
                history_moves: &mut self.history_moves,
            },
        );

        // Returns the best move found in UCI format. Pondering is not implemented in
        // this version, so we return `None` for the ponder move.
        Ok(BestMove {
            mv: best_move.to_uci_string(),
            ponder: None,
        })
    }

    fn ponderhit(&mut self) {
        eprintln!("debug: ponderhit");
    }

    fn quit(&self) {
        eprintln!("debug: quit");
    }

    fn running_status(&self) -> Arc<RunningStatus> {
        self.keep_running.clone()
    }
}
