use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

use anyhow::Result;

use crate::uci::{BestMove, Engine, GoParameters};

/// Reads UCI commands from a [`BufRead`] source (typically stdin), parses
/// them, and dispatches to the provided [`Engine`] implementation.
///
/// The handler is the sole owner of stdout output. Engine methods return typed
/// values; the handler formats and prints them.
///
/// ## Stop flag
/// The handler holds an `Arc<AtomicBool>` that is passed through
/// [`GoParameters::stop`] to the engine's `go` implementation. When the GUI
/// sends a `stop` command the handler sets this flag to `true` so the engine's
/// search loop can exit cleanly and return the best move found so far.
pub struct UCIHandler<T: Engine> {
    engine: T,
    /// Shared stop signal. Set to `true` by the `stop` handler; reset to
    /// `false` at the beginning of every `go` handler.
    stop_flag: Arc<AtomicBool>,
}

impl<T: Engine> UCIHandler<T> {
    pub fn new(engine: T) -> Self {
        Self {
            engine,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Handle a single line of UCI input.
    ///
    /// Returns `true` when the engine should quit.
    fn handle_command(&mut self, full_command: &str) -> Result<bool> {
        let tokens = full_command.split_whitespace().collect::<Vec<&str>>();
        let mut token_stream = tokens.iter();

        // Outer loop handles the first recognised command token on each line.
        // Per the UCI spec, unknown tokens are silently ignored.
        while let Some(&token) = token_stream.next() {
            match token {
                "uci" => {
                    let (id, options) = self.engine.uci();
                    println!("{id}");
                    for opt in options {
                        println!("{opt}");
                    }
                    println!("uciok");
                }
                "debug" => {
                    if let Some(&val) = token_stream.next()
                        && (val == "on" || val == "off")
                    {
                        self.engine.debug(val == "on");
                    }
                }
                "isready" => {
                    self.engine.isready();
                    println!("readyok");
                }
                "setoption" => {
                    if let Err(e) = self.engine.setoption((&mut token_stream).try_into()?) {
                        log::error!("setoption failed: {e:?}");
                    }
                }
                "register" => {
                    self.engine.register((&mut token_stream).try_into()?);
                }
                "ucinewgame" => {
                    self.engine.ucinewgame();
                }
                "position" => {
                    if let Err(e) = self.engine.position((&mut token_stream).try_into()?) {
                        log::error!("position failed: {e:?}");
                    }
                }
                "go" => {
                    // Reset the stop flag before each new search so stale
                    // `stop` commands from a previous search don't affect this one.
                    self.stop_flag.store(false, Ordering::SeqCst);

                    let mut params = GoParameters::try_from(&mut token_stream)?;
                    // Hand the engine a clone of the flag; the handler keeps
                    // its own Arc to set when `stop` arrives.
                    params.stop = Arc::clone(&self.stop_flag);

                    let (tx, rx) = mpsc::channel();
                    let best = match self.engine.go(params, tx) {
                        Ok(b) => b,
                        Err(e) => {
                            log::error!("go failed: {e:?}");
                            BestMove {
                                mv: "0000".into(),
                                ponder: None,
                            }
                        }
                    };
                    // Drain all info messages the engine sent during search.
                    for info in rx.try_iter() {
                        println!("{info}");
                    }
                    // The handler always prints bestmove, never the engine.
                    println!("{best}");
                }
                "stop" => {
                    // Signal the search loop to exit.
                    self.stop_flag.store(true, Ordering::SeqCst);
                    self.engine.stop();
                }
                "ponderhit" => {
                    self.engine.ponderhit();
                }
                "quit" => {
                    self.engine.quit();
                    return Ok(true);
                }
                _ => {
                    // Unknown command token — ignore per UCI spec.
                    continue;
                }
            }

            // Only one command is processed per line.
            break;
        }

        Ok(false)
    }

    /// Run the UCI loop until EOF or a `quit` command.
    pub fn run<R: BufRead>(mut self, mut reader: R) -> Result<()> {
        let mut line = String::new();

        loop {
            line.clear();

            if reader.read_line(&mut line)? == 0 {
                // Reached EOF — exit cleanly.
                break Ok(());
            }

            match self.handle_command(&line) {
                Ok(true) => break Ok(()),
                Ok(false) => {}
                Err(err) => log::error!("UCI error: {err:?}"),
            }
        }
    }
}
