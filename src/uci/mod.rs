mod command;
mod engine;
mod output;

use std::sync::{Arc, mpsc::Receiver};

pub use command::*;
pub use engine::*;
pub use output::*;

fn spawn_listener(is_running: Arc<RunningStatus>) -> Receiver<EngineCommand> {
    let (tx, rx) = std::sync::mpsc::channel();

    let mut command_str = String::new();

    std::thread::spawn(move || {
        loop {
            command_str.clear();
            std::io::stdin()
                .read_line(&mut command_str)
                .expect("Failed to read line from stdin");

            let Some(command) = EngineCommand::parse(&command_str) else {
                continue;
            };

            match command {
                EngineCommand::IsReady => println!("readyok"),
                EngineCommand::Stop => is_running.set(RunningStatus::STOPPED),
                EngineCommand::Quit => {
                    is_running.set(RunningStatus::STOPPED);
                    tx.send(command).ok();
                    break;
                }
                _ => {
                    // According to the UCI specs, commands that are unexpected
                    // in the current state should be ignored silently.
                    // (https://backscattering.de/chess/uci/#unexpected)
                    if !is_running.get() {
                        tx.send(command).ok();
                    }
                }
            }
        }
    });

    rx
}

pub fn message_loop<E: Engine + 'static>(mut bot: E) {
    let is_running = bot.get_running_status();
    let rx = spawn_listener(is_running);

    for command in rx {
        match command {
            EngineCommand::Uci => {
                let (id, options) = bot.uci();
                println!("{id}");
                for opt in options {
                    println!("{opt}");
                }
                println!("uciok");
            }
            EngineCommand::Debug(on) => bot.debug(on),
            EngineCommand::IsReady => {
                bot.isready();
                println!("readyok");
            }
            EngineCommand::SetOption(p) => {
                if let Err(e) = bot.setoption(p) {
                    eprintln!("error: setoption failed: {e:?}");
                }
            }
            EngineCommand::Register(p) => bot.register(p),
            EngineCommand::NewGame => bot.ucinewgame(),
            EngineCommand::Position(p) => {
                if let Err(e) = bot.position(p) {
                    eprintln!("error: position failed: {e:?}");
                }
            }
            EngineCommand::Go(params) => {
                let best = match bot.go(params) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("error: go failed: {e:?}");
                        BestMove::null()
                    }
                };
                println!("{best}");
            }
            EngineCommand::PonderHit => bot.ponderhit(),
            EngineCommand::Quit => {
                bot.quit();
                break;
            }
            _ => {
                unreachable!();
            }
        }
    }
}
