use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use anyhow::Result;

use crate::uci::{BestMove, Engine, UciInfo};

use super::command::EngineCommand;
use super::output::EngineOutput;

/// Orchestrates three persistent threads to implement the UCI protocol.
///
/// ## Thread A — Stdin reader (caller's thread)
/// Reads lines from the provided [`BufRead`] source, parses them into typed
/// [`EngineCommand`] values, and sends them to Thread B. On `stop` it sets the
/// shared `stop_flag` atomically *before* enqueuing the command, so Thread B's
/// engine can observe the flag without waiting for the command queue to drain.
///
/// ## Thread B — Engine actor (spawned)
/// Owns the engine. Receives commands from Thread A and calls the corresponding
/// [`Engine`] trait methods. During `go` it blocks while the engine searches;
/// a short-lived forwarder thread streams `UciInfo` messages to Thread C in
/// real-time. Because Thread A remains unblocked, `stop` can interrupt the
/// search via the shared `Arc<AtomicBool>`.
///
/// ## Thread C — Output printer (spawned)
/// Drains the output channel and calls `println!` for every message. Owning
/// all output in a single thread prevents any stdout races.
pub struct UCIHandler<T: Engine> {
    engine: T,
}

impl<T: Engine + 'static> UCIHandler<T> {
    pub fn new(engine: T) -> Self {
        Self { engine }
    }

    /// Start the UCI loop.
    ///
    /// Spawns Threads B and C, then runs Thread A logic on the calling thread
    /// until EOF or a `quit` command. Joins both spawned threads before returning.
    pub fn run<R: BufRead>(self, reader: R) -> Result<()> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let (cmd_tx, cmd_rx) = mpsc::channel::<EngineCommand>();
        let (out_tx, out_rx) = mpsc::channel::<EngineOutput>();

        // Thread C — output printer: the sole owner of stdout.
        let printer = thread::spawn(move || {
            for output in out_rx {
                println!("{output}");
            }
        });

        // Thread B — engine actor: the sole owner of the engine.
        let stop_flag_b = Arc::clone(&stop_flag);
        let actor = thread::spawn(move || {
            run_actor(self.engine, cmd_rx, out_tx, stop_flag_b);
        });

        // Thread A — stdin reader: runs on the caller's thread.
        run_reader(reader, cmd_tx, stop_flag);

        actor.join().expect("engine actor thread panicked");
        printer.join().expect("output printer thread panicked");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Thread A
// ---------------------------------------------------------------------------

/// Reads UCI lines, parses them into [`EngineCommand`]s, and sends them to
/// the engine actor. Dropping `cmd_tx` on exit signals Thread B to shut down.
fn run_reader<R: BufRead>(
    reader: R,
    cmd_tx: mpsc::Sender<EngineCommand>,
    stop_flag: Arc<AtomicBool>,
) {
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                log::error!("stdin read error: {e}");
                break;
            }
        };

        match EngineCommand::parse(&line) {
            Ok(Some(cmd)) => {
                // Set the flag before enqueuing Stop *or* Quit so the engine's
                // search loop can observe it immediately without waiting for the
                // command queue to drain. This ensures engine.go() exits
                // promptly even if the GUI sends `quit` during infinite search.
                if matches!(cmd, EngineCommand::Stop | EngineCommand::Quit) {
                    stop_flag.store(true, Ordering::SeqCst);
                }
                let quit = matches!(cmd, EngineCommand::Quit);
                // Ignore send errors (Thread B exited early).
                cmd_tx.send(cmd).ok();
                if quit {
                    break;
                }
            }
            Ok(None) => {} // blank line or unknown command — ignore per spec
            Err(e) => log::error!("UCI parse error: {e:?}"),
        }
    }
    // The loop exited: either via `quit`, EOF, or an I/O error.
    //
    // Guarantee 1: any in-progress engine.go() exits promptly by setting the
    // stop flag. (For the `quit` path this is already true from the arm above;
    // the store here is a harmless no-op in that case.)
    stop_flag.store(true, Ordering::SeqCst);
    //
    // Guarantee 2: engine.quit() is always called for clean teardown, even
    // when the session ended with EOF rather than an explicit `quit` command.
    // If Thread B's receiver is already gone the send returns Err, which we
    // ignore with `.ok()` — engine.quit() will not be called twice.
    cmd_tx.send(EngineCommand::Quit).ok();
    // Dropping cmd_tx here signals Thread B's cmd_rx loop to exit.
}

// ---------------------------------------------------------------------------
// Thread B
// ---------------------------------------------------------------------------

/// Receives [`EngineCommand`]s and dispatches to the [`Engine`] trait.
/// Dropping `out_tx` on exit signals Thread C to shut down.
fn run_actor<T: Engine>(
    mut engine: T,
    cmd_rx: mpsc::Receiver<EngineCommand>,
    out_tx: mpsc::Sender<EngineOutput>,
    stop_flag: Arc<AtomicBool>,
) {
    for cmd in cmd_rx {
        match cmd {
            EngineCommand::Uci => {
                let (id, options) = engine.uci();
                out_tx.send(EngineOutput::Identity(id)).ok();
                for opt in options {
                    out_tx.send(EngineOutput::Opt(opt)).ok();
                }
                out_tx.send(EngineOutput::UciOk).ok();
            }
            EngineCommand::Debug(on) => engine.debug(on),
            EngineCommand::IsReady => {
                engine.isready();
                out_tx.send(EngineOutput::ReadyOk).ok();
            }
            EngineCommand::SetOption(p) => {
                if let Err(e) = engine.setoption(p) {
                    log::error!("setoption failed: {e:?}");
                }
            }
            EngineCommand::Register(p) => engine.register(p),
            EngineCommand::NewGame => engine.ucinewgame(),
            EngineCommand::Position(p) => {
                if let Err(e) = engine.position(p) {
                    log::error!("position failed: {e:?}");
                }
            }
            EngineCommand::Go(mut params) => {
                // Reset before each search so stale stops don't short-circuit
                // the next go call.
                stop_flag.store(false, Ordering::SeqCst);
                // Inject the real shared flag so engine.go() can check it.
                params.stop = Arc::clone(&stop_flag);

                let (info_tx, info_rx) = mpsc::channel::<UciInfo>();

                // Short-lived forwarder: bridges UciInfo → EngineOutput in
                // real-time. Exits automatically when info_tx is dropped
                // (i.e. when engine.go() returns and info_tx goes out of scope).
                let out_tx2 = out_tx.clone();
                let forwarder = thread::spawn(move || {
                    for info in info_rx {
                        out_tx2.send(EngineOutput::Info(info)).ok();
                    }
                });

                // Blocks Thread B for the duration of the search.
                // Thread A continues reading stdin and can set stop_flag at any time.
                let best = match engine.go(params, info_tx) {
                    Ok(b) => b,
                    Err(e) => {
                        log::error!("go failed: {e:?}");
                        BestMove::null()
                    }
                };

                // Wait for the forwarder to flush all remaining info messages
                // *before* sending bestmove, preserving output order.
                forwarder.join().ok();
                out_tx.send(EngineOutput::BestMove(best)).ok();
            }
            EngineCommand::Stop => {
                // The flag was already set atomically by Thread A.
                // Call engine.stop() for any engine-side cleanup.
                engine.stop();
            }
            EngineCommand::PonderHit => engine.ponderhit(),
            EngineCommand::Quit => {
                engine.quit();
                break;
            }
        }
    }
    // Dropping out_tx here signals Thread C's for-loop to exit.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::PrintBot;
    use std::io::Cursor;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_run_reader() {
        let input = "uci\nisready\nstop\nquit\n";
        let reader = Cursor::new(input);
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let stop_flag = Arc::new(AtomicBool::new(false));

        run_reader(reader, cmd_tx, stop_flag.clone());

        // Verify commands sent to actor
        let mut cmds = Vec::new();
        while let Ok(cmd) = cmd_rx.try_recv() {
            cmds.push(cmd);
        }

        assert_eq!(cmds.len(), 5);
        assert!(matches!(cmds[0], EngineCommand::Uci));
        assert!(matches!(cmds[1], EngineCommand::IsReady));
        assert!(matches!(cmds[2], EngineCommand::Stop));
        assert!(matches!(cmds[3], EngineCommand::Quit));
        assert!(matches!(cmds[4], EngineCommand::Quit));

        // Stop flag should be set to true due to Stop/Quit commands
        assert!(stop_flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_run_actor() {
        let engine = PrintBot::default();
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Queue some commands
        cmd_tx.send(EngineCommand::Uci).unwrap();
        cmd_tx.send(EngineCommand::IsReady).unwrap();
        cmd_tx.send(EngineCommand::Quit).unwrap();

        // Dropping cmd_tx so the receiver channel closes when drained
        drop(cmd_tx);

        run_actor(engine, cmd_rx, out_tx, stop_flag);

        // Gather outputs
        let mut outputs = Vec::new();
        while let Ok(out) = out_rx.try_recv() {
            outputs.push(out);
        }

        // Expected output from PrintBot for Uci is Identity, 2 Opts, UciOk
        // For IsReady is ReadyOk
        assert_eq!(outputs.len(), 5);
        assert!(matches!(outputs[0], EngineOutput::Identity(_)));
        assert!(matches!(outputs[1], EngineOutput::Opt(_)));
        assert!(matches!(outputs[2], EngineOutput::Opt(_)));
        assert!(matches!(outputs[3], EngineOutput::UciOk));
        assert!(matches!(outputs[4], EngineOutput::ReadyOk));
    }
}
