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

    /// Called when the GUI sends a `stop` command.
    ///
    /// By the time this method is called, the shared `stop_flag` inside
    /// [`GoParameters::stop`] has **already been set to `true`** by Thread A
    /// (the stdin reader). The engine's search loop should have observed the
    /// flag and returned a `BestMove` before this is called.
    ///
    /// Use this method for any engine-side cleanup after a search is
    /// interrupted — e.g. clearing search state, joining internal threads.
    fn stop(&mut self);

    /// Called when the GUI sends `ponderhit` — the opponent played the move the
    /// engine was pondering on; switch from ponder mode to normal search.
    ///
    /// # Current limitation
    /// Thread B (the engine actor) is blocked inside `engine.go()` while
    /// searching, so a `ponderhit` command queues in the command channel but
    /// is **not processed until the current search returns**. This means
    /// pondering cannot be properly supported until `go` is made non-blocking
    /// (e.g. by managing an internal search thread inside the engine).
    fn ponderhit(&mut self);

    /// Quit the engine as soon as possible.
    fn quit(&self);
}
