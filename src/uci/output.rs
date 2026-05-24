use std::fmt;

use crate::uci::{BestMove, UciId, UciInfo, UciOption};

/// A message sent from the engine actor to the output printer thread.
///
/// Each variant corresponds to exactly one `println!` call in the output
/// thread. Routing all output through this enum ensures there are no stdout
/// races between concurrent threads.
///
/// # Limitations
/// - No `Registration` variant yet (`registration ok` / `registration error`).
///   Add `Registration(bool)` when copy-protection is wired in.
/// - No `CopyProtection` variant (`copyprotection checking` / `ok` / `error`).
pub enum EngineOutput {
    /// Prints `"id name …\nid author …"`.
    Identity(UciId),
    /// Prints `"option name … type …"`.
    ///
    /// Named `Opt` to avoid shadowing `std::option::Option`.
    Opt(UciOption),
    /// Prints `"uciok"`.
    UciOk,
    /// Prints `"readyok"`.
    ReadyOk,
    /// Prints `"info …"` — streamed in real-time while the engine searches.
    Info(UciInfo),
    /// Prints `"bestmove … [ponder …]"` — sent once after every `go`.
    BestMove(BestMove),
}

impl fmt::Display for EngineOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(id) => write!(f, "{id}"),
            Self::Opt(opt) => write!(f, "{opt}"),
            Self::UciOk => write!(f, "uciok"),
            Self::ReadyOk => write!(f, "readyok"),
            Self::Info(info) => write!(f, "{info}"),
            Self::BestMove(bm) => write!(f, "{bm}"),
        }
    }
}
