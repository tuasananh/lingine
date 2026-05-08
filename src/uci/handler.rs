use std::io::BufRead;

use anyhow::Result;

use crate::uci::Engine;

pub struct UCIHandler<T: Engine> {
    engine: T,
}

impl<T: Engine> UCIHandler<T> {
    pub fn new(engine: T) -> Self {
        Self { engine }
    }

    pub fn run<R: BufRead>(self, mut reader: R) -> Result<()> {
        let mut full_command = String::new();

        loop {
            full_command.clear();

            if reader.read_line(&mut full_command)? == 0 {
                // Reached EOF
                break Ok(());
            }

            let tokens = full_command.split_whitespace().collect::<Vec<&str>>();
            let mut token_stream = tokens.iter();
            let mut quit = false;

            while let Some(token) = token_stream.next()
                && !quit
            {
                let token = *token;
                match token {
                    "uci" => {
                        self.engine.uci()?;
                    }
                    "debug" => {
                        if let Some(&val) = token_stream.next()
                            && (val == "on" || val == "off")
                        {
                            self.engine.debug(val == "on")?;
                        }
                    }
                    "isready" => {
                        self.engine.isready()?;
                    }
                    "setoption" => {
                        self.engine.setoption((&mut token_stream).try_into()?)?;
                    }
                    "register" => {
                        self.engine.register((&mut token_stream).try_into()?)?;
                    }
                    "ucinewgame" => {
                        self.engine.ucinewgame()?;
                    }
                    "position" => {
                        self.engine.position((&mut token_stream).try_into()?)?;
                    }
                    "go" => {
                        self.engine.go((&mut token_stream).try_into()?)?;
                    }
                    "stop" => {
                        self.engine.stop()?;
                    }
                    "ponderhit" => {
                        self.engine.ponderhit()?;
                    }
                    "quit" => {
                        self.engine.quit()?;
                        quit = true;
                        break;
                    }
                    _ => {
                        // Unknown command, ignore
                        continue;
                    }
                }

                break;
            }

            if quit {
                break Ok(());
            }
        }
    }
}
