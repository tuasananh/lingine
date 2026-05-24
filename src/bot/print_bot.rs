use std::sync::mpsc::Sender;

use anyhow::Result;

use crate::uci::{
    BestMove, Engine, GoParameters, RegisterParameters, SetOptionParameters, UciId, UciInfo,
    UciOption, UciPosition,
};

/// A stub engine implementation that satisfies the [`Engine`] trait.
///
/// It does not actually search; it is used to verify that the UCI protocol
/// layer works end-to-end before the real engine is wired in.
///
/// Use [`Default::default()`] to construct it.
#[derive(Default)]
pub struct PrintBot;

impl Engine for PrintBot {
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

    fn debug(&self, is_on: bool) {
        log::debug!("debug mode: {is_on}");
    }

    fn isready(&self) {
        // Nothing to initialise yet.
    }

    fn setoption(&mut self, params: SetOptionParameters) -> Result<()> {
        log::debug!("setoption: name={:?} value={:?}", params.name, params.value);
        Ok(())
    }

    fn ucinewgame(&mut self) {
        log::debug!("ucinewgame");
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
        Ok(())
    }

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

    fn stop(&mut self) {
        // The stop flag in GoParameters is already set by the handler before
        // this method is called. Nothing extra to do for a synchronous stub.
        log::debug!("stop");
    }

    fn ponderhit(&mut self) {
        log::debug!("ponderhit");
    }

    fn quit(&self) {
        log::debug!("quit");
    }
}
