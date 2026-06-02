use anyhow::Result;

use crate::uci::{GoParameters, RegisterParameters, SetOptionParameters, UciPosition};

/// A parsed UCI command received from the GUI on stdin.
///
/// [`EngineCommand::parse`] converts a raw input line into a typed command.
/// Blank lines and unrecognised commands produce `Ok(None)` (per spec: silently
/// ignored). Structurally invalid known commands (e.g. a malformed FEN string
/// in `position`) produce `Err`.
///
/// The parsed value is sent through the command channel to the engine actor
/// thread, which calls the corresponding [`Engine`] trait method.
pub enum EngineCommand {
    /// `uci` — GUI asks for engine identity and options.
    Uci,
    /// `debug on|off` — enable or disable verbose `info string` output.
    ///
    /// Only produced when the keyword is followed by exactly `"on"` or `"off"`;
    /// a missing or unrecognised value silently yields `Ok(None)`.
    Debug(bool),
    /// `isready` — GUI asks whether the engine is ready; handler replies
    /// `readyok`.
    IsReady,
    /// `setoption name <name> [value <value>]` — change a configuration option.
    SetOption(SetOptionParameters),
    /// `register later|name <n> code <c>` — engine registration
    /// (copy-protection).
    Register(RegisterParameters),
    /// `ucinewgame` — the next position will be from a different game; reset
    /// state.
    NewGame,
    /// `position startpos|fen <fen> [moves <mv>…]` — set the current board
    /// position.
    Position(UciPosition),
    /// `go <params>` — start searching the current position.
    ///
    /// The `stop` field of [`GoParameters`] is left at its `Default` value
    /// here (a fresh, unshared `Arc<AtomicBool>`). The engine actor injects
    /// the real shared flag just before calling `engine.go()` so that Thread A
    /// can interrupt the search via `stop_flag.store(true)`.
    Go(GoParameters),
    /// `stop` — interrupt the running search as soon as possible.
    ///
    /// Thread A sets the shared `stop_flag` atomically *before* enqueuing this
    /// command, so the engine's search loop can observe the signal without
    /// waiting for the command queue to drain.
    Stop,
    /// `ponderhit` — the opponent played the ponder move; switch to normal
    /// search.
    ///
    /// # Limitation
    /// Thread B is blocked inside `engine.go()` while searching, so this
    /// command queues but is not processed until the search returns. There is
    /// no plumbing to handle it mid-search yet.
    PonderHit,
    /// `quit` — terminate the engine process.
    Quit,
}

impl EngineCommand {
    /// Parse one line of UCI input into a command.
    ///
    /// Returns `Ok(None)` for blank lines and unknown commands (silently
    /// ignored per the UCI spec).
    /// Returns `Ok(Some(cmd))` for valid, fully-parsed commands.
    /// Returns `Err` only for *structurally invalid* known commands — for
    /// example, a `position` line with a malformed FEN, or a `go` line with a
    /// non-numeric `depth` value.
    ///
    /// # Limitations
    /// - `debug` requires exactly `"on"` or `"off"` after the keyword; any
    ///   other value (including a missing value) silently yields `Ok(None)` and
    ///   the engine receives nothing. The UCI spec is silent on this edge case.
    pub fn parse(line: &str) -> Result<Option<Self>> {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let mut iter = tokens.iter();

        match iter.next().copied() {
            None => Ok(None),
            Some("uci") => Ok(Some(Self::Uci)),
            Some("debug") => match iter.next().copied() {
                Some("on") => Ok(Some(Self::Debug(true))),
                Some("off") => Ok(Some(Self::Debug(false))),
                // Missing or unrecognised value — spec says nothing, so ignore.
                _ => Ok(None),
            },
            Some("isready") => Ok(Some(Self::IsReady)),
            Some("setoption") => Ok(Some(Self::SetOption(SetOptionParameters::try_from(
                &mut iter,
            )?))),
            Some("register") => Ok(Some(Self::Register(RegisterParameters::try_from(
                &mut iter,
            )?))),
            Some("ucinewgame") => Ok(Some(Self::NewGame)),
            Some("position") => Ok(Some(Self::Position(UciPosition::try_from(&mut iter)?))),
            Some("go") => Ok(Some(Self::Go(GoParameters::try_from(&mut iter)?))),
            Some("stop") => Ok(Some(Self::Stop)),
            Some("ponderhit") => Ok(Some(Self::PonderHit)),
            Some("quit") => Ok(Some(Self::Quit)),
            // Unknown command — UCI spec says ignore silently.
            Some(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_commands() {
        assert!(matches!(EngineCommand::parse(""), Ok(None)));
        assert!(matches!(EngineCommand::parse("   "), Ok(None)));
        assert!(matches!(EngineCommand::parse("unknown_command"), Ok(None)));

        assert!(matches!(
            EngineCommand::parse("uci"),
            Ok(Some(EngineCommand::Uci))
        ));
        assert!(matches!(
            EngineCommand::parse("isready"),
            Ok(Some(EngineCommand::IsReady))
        ));
        assert!(matches!(
            EngineCommand::parse("ucinewgame"),
            Ok(Some(EngineCommand::NewGame))
        ));
        assert!(matches!(
            EngineCommand::parse("stop"),
            Ok(Some(EngineCommand::Stop))
        ));
        assert!(matches!(
            EngineCommand::parse("ponderhit"),
            Ok(Some(EngineCommand::PonderHit))
        ));
        assert!(matches!(
            EngineCommand::parse("quit"),
            Ok(Some(EngineCommand::Quit))
        ));
    }

    #[test]
    fn test_parse_debug() {
        assert!(matches!(
            EngineCommand::parse("debug on"),
            Ok(Some(EngineCommand::Debug(true)))
        ));
        assert!(matches!(
            EngineCommand::parse("debug off"),
            Ok(Some(EngineCommand::Debug(false)))
        ));
        assert!(matches!(EngineCommand::parse("debug"), Ok(None)));
        assert!(matches!(EngineCommand::parse("debug foo"), Ok(None)));
    }

    #[test]
    fn test_parse_setoption() {
        let cmd = EngineCommand::parse("setoption name Hash value 16")
            .unwrap()
            .unwrap();
        if let EngineCommand::SetOption(params) = cmd {
            assert_eq!(params.name, "hash");
            assert_eq!(params.value, Some("16".to_string()));
        } else {
            panic!("Expected SetOption");
        }

        // Invalid setoption format should fail
        assert!(EngineCommand::parse("setoption").is_err());
    }

    #[test]
    fn test_parse_register() {
        let cmd = EngineCommand::parse("register later").unwrap().unwrap();
        assert!(matches!(
            cmd,
            EngineCommand::Register(RegisterParameters::Later)
        ));
    }

    #[test]
    fn test_parse_position() {
        let cmd = EngineCommand::parse("position startpos").unwrap().unwrap();
        if let EngineCommand::Position(pos) = cmd {
            assert_eq!(pos.fen, crate::uci::START_FEN);
            assert!(pos.moves.is_empty());
        } else {
            panic!("Expected Position");
        }
    }

    #[test]
    fn test_parse_go() {
        let cmd = EngineCommand::parse("go depth 5").unwrap().unwrap();
        if let EngineCommand::Go(params) = cmd {
            assert_eq!(params.depth, Some(5));
        } else {
            panic!("Expected Go");
        }
    }
}
