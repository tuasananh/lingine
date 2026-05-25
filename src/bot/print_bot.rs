use std::sync::mpsc::Sender;

use anyhow::Result;

use crate::uci::{
    BestMove, Engine, GoParameters, RegisterParameters, SetOptionParameters, UciId, UciInfo,
    UciOption, UciPosition,
};

/// A stub [`Engine`] implementation used to verify the UCI protocol layer
/// end-to-end before the real search engine is written.
///
/// Every method is a no-op or log statement. `go` always returns
/// [`BestMove::null()`] (`"bestmove 0000"`).
///
/// # Usage
/// Construct with the unit-struct literal:
/// ```rust,ignore
/// UCIHandler::new(PrintBot).run(stdin().lock())?;
/// ```
///
/// Replace `PrintBot` with the real engine type once search is implemented.
#[derive(Default)]
pub struct PrintBot;

impl Engine for PrintBot {
    /// Returns the engine name `"Lingine"` and two options:
    /// - `Hash` (spin, 1–1024 MB, default 16)
    /// - `Ponder` (check, default false)
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

    /// Logs the debug mode change. No behaviour changes in the stub.
    fn debug(&self, is_on: bool) {
        log::debug!("debug mode: {is_on}");
    }

    /// No initialisation needed for the stub; always ready immediately.
    fn isready(&self) {
        // Nothing to initialise yet.
    }

    /// Logs the option change. Always succeeds (`Ok(())`); a real engine
    /// would parse `params.name`/`params.value` and update its configuration.
    fn setoption(&mut self, params: SetOptionParameters) -> Result<()> {
        log::debug!("setoption: name={:?} value={:?}", params.name, params.value);
        Ok(())
    }

    /// Logs; a real engine should clear its hash table and reset search state.
    fn ucinewgame(&mut self) {
        log::debug!("ucinewgame");
    }

    /// Logs the registration details. No copy-protection in Lingine.
    fn register(&self, params: RegisterParameters) {
        match params {
            RegisterParameters::Later => log::debug!("register later"),
            RegisterParameters::Identity { name, code } => {
                log::debug!("register name={name:?} code={code:?}");
            }
        }
    }

    /// Logs the FEN and move list. Always succeeds; a real engine would apply
    /// the moves to its internal board representation.
    fn position(&mut self, position: UciPosition) -> Result<()> {
        log::debug!("position fen={:?} moves={:?}", position.fen, position.moves);
        Ok(())
    }

    /// Sends one `info string` message and immediately returns the null move.
    ///
    /// A real implementation would run a search loop here, checking the stop
    /// flag periodically:
    /// ```rust,ignore
    /// while !params.stop.load(Ordering::Relaxed) {
    ///     // search deeper…
    ///     tx.send(UciInfo { depth: Some(current_depth), … }).ok();
    /// }
    /// Ok(best_move_found)
    /// ```
    fn go(&mut self, _params: GoParameters, tx: Sender<UciInfo>) -> Result<BestMove> {
        // No real search — send a single info string so the protocol layer
        // has something to print, then return a null move.
        //
        // When real search is added, check `_params.stop.load(Ordering::Relaxed)`
        // periodically in the search loop to respect the stop command.
        let _ = tx.send(UciInfo {
            string: Some("PrintBot has no search implemented".into()),
            ..UciInfo::new()
        });
        Ok(BestMove {
            mv: "0000".into(),
            ponder: None,
        })
    }

    /// Logs the stop signal. The shared `stop_flag` was already set by
    /// Thread A before this method is called; no extra action is needed in
    /// the stub.
    fn stop(&mut self) {
        // The stop flag in GoParameters is already set by the handler before
        // this method is called. Nothing extra to do for a synchronous stub.
        log::debug!("stop");
    }

    /// Logs. No pondering implemented yet; see [`EngineCommand::PonderHit`]
    /// for the current limitation.
    fn ponderhit(&mut self) {
        log::debug!("ponderhit");
    }

    /// Logs. A real engine should flush any buffers and free resources.
    fn quit(&self) {
        log::debug!("quit");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uci::GoParameters;
    use std::sync::mpsc;

    #[test]
    fn test_print_bot_interface() {
        let mut bot = PrintBot;
        let (id, options) = bot.uci();
        assert_eq!(id.name, "Lingine");
        assert_eq!(id.author, "tuasananh");
        assert_eq!(options.len(), 2);

        // Call other methods to ensure no panics and expected behavior
        bot.debug(true);
        bot.isready();

        let set_opt = SetOptionParameters {
            name: "Hash".into(),
            value: Some("64".into()),
        };
        assert!(bot.setoption(set_opt).is_ok());

        bot.ucinewgame();
        bot.register(RegisterParameters::Later);

        let pos = UciPosition {
            fen: "startpos".into(),
            moves: vec![],
        };
        assert!(bot.position(pos).is_ok());

        let (tx, rx) = mpsc::channel();
        let best_move = bot.go(GoParameters::default(), tx).unwrap();

        bot.stop();
        bot.ponderhit();
        bot.quit();

        assert_eq!(best_move.mv, "0000");
        assert_eq!(best_move.ponder, None);

        // Verify the stream received the expected info message
        let info = rx.recv().unwrap();
        assert_eq!(
            info.string,
            Some("PrintBot has no search implemented".into())
        );
    }
}
