use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;

use crate::uci::{
    BestMove, GoParameters, PositionParameters, RegisterParameters, SetOptionParameters, UciId,
    UciOption,
};

pub struct RunningStatus {
    inner: AtomicBool,
}

impl Default for RunningStatus {
    fn default() -> Self {
        Self {
            inner: AtomicBool::new(Self::STOPPED),
        }
    }
}

impl RunningStatus {
    pub const STOPPED: bool = false;
    pub const RUNNING: bool = true;

    pub fn get(&self) -> bool {
        self.inner.load(Ordering::Acquire)
    }

    pub fn set(&self, value: bool) {
        self.inner.store(value, Ordering::Release)
    }
}

/// The interface every engine implementation must satisfy.
pub trait Engine: Send {
    /// Return the engine's identity and its supported options.
    ///
    /// The handler will print:
    /// ```text
    /// id name <name>
    /// id author <author>
    /// option name ... type ...
    /// ...
    /// uciok
    /// ```
    fn uci(&self) -> (UciId, Vec<UciOption>);

    /// Switch debug mode on or off.
    ///
    /// In debug mode the engine may send additional `info string` messages.
    fn debug(&self, is_on: bool);

    /// Called when the GUI sends `isready`.
    ///
    /// The engine should finish any pending initialisation work here.
    /// The handler sends `readyok` after this method returns.
    fn isready(&self);

    /// Apply a configuration change.
    ///
    /// Returns an error if the option value is invalid or the engine cannot
    /// apply the change (e.g. allocation failure when resizing a hash table).
    fn setoption(&mut self, params: SetOptionParameters) -> Result<()>;

    /// Signal that the next search will be for a different game.
    fn ucinewgame(&mut self);

    /// Register the engine (e.g. with a name/code for copy-protection).
    fn register(&self, params: RegisterParameters);

    /// Set the current board position from a FEN string and a list of moves.
    ///
    /// Returns an error if the FEN string is malformed or any move is illegal.
    fn position(&mut self, position: PositionParameters) -> Result<()>;

    /// Start searching the current position.
    ///
    /// The engine should return a [`BestMove`] when the search is complete.
    ///
    /// Any additional info messages should be printed during the search.
    fn go(&mut self, params: GoParameters) -> Result<BestMove>;

    /// Called when the GUI sends `ponderhit` — the opponent played the move the
    /// engine was pondering on; switch from ponder mode to normal search.
    fn ponderhit(&mut self);

    /// Quit the engine as soon as possible.
    fn quit(&self);

    /// Return the running status of the engine.
    ///
    /// Useful for when we need to stop or check if the engine is running.
    fn running_status(&self) -> Arc<RunningStatus>;
}
