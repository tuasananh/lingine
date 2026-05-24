use std::sync::mpsc::Sender;

use anyhow::Result;

use crate::uci::{
    BestMove, GoParameters, RegisterParameters, SetOptionParameters, UciId, UciInfo, UciOption,
    UciPosition,
};

/// The interface every engine implementation must satisfy.
///
/// The [`UCIHandler`] calls these methods in response to GUI commands and is
/// responsible for all output to stdout (printing `uciok`, `readyok`,
/// `bestmove`, `info`, etc.). Engine implementations only produce typed values
/// and must **never** write to stdout directly.
pub trait Engine {
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
    fn position(&mut self, position: UciPosition) -> Result<()>;

    /// Start searching the current position.
    ///
    /// The engine should stream [`UciInfo`] messages through `tx` while
    /// searching, then return a [`BestMove`] when the search is complete.
    ///
    /// The handler prints every received `info` line and the final `bestmove`
    /// line. The engine must **not** print anything itself.
    ///
    /// ## Stop flag
    /// [`GoParameters::stop`] is an `Arc<AtomicBool>` shared with the handler.
    /// The handler sets it to `true` when it receives a `stop` command.
    /// The engine's search loop **must** check this flag periodically and
    /// return the best move found so far when it becomes `true`.
    ///
    /// ## Threading
    /// The flag is `Arc<AtomicBool>` so `go` can safely be moved to a
    /// background thread in Phase 4 without changing this trait signature.
    fn go(&mut self, params: GoParameters, tx: Sender<UciInfo>) -> Result<BestMove>;

    /// Stop the current search as soon as possible.
    ///
    /// The engine must send a `bestmove` response even when stopped mid-search.
    /// Because `go` is currently synchronous this is only useful once threaded
    /// search is implemented.
    fn stop(&mut self);

    /// The user played the ponder move; switch from pondering to normal search.
    fn ponderhit(&mut self);

    /// Quit the engine as soon as possible.
    fn quit(&self);
}
