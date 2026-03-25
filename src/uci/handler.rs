use crate::uci::engine::UCIEngine;

pub struct UCIHandler<T: UCIEngine> {
    engine: T
}

impl<T: UCIEngine> UCIHandler<T> {
    pub fn new() -> Self {
        Self {
            engine: T::new()
        }
    }

    pub fn run(self) {
        let mut full_command = String::new();

        loop {
            full_command.clear();

            std::io::stdin().read_line(&mut full_command).ok().unwrap();

            let tokens = full_command.split_whitespace().collect::<Vec<&str>>();
            let mut token_stream = tokens.iter();
            let mut quit = false;

            while let Some(token ) = token_stream.next() && !quit {
                let token = *token;
                match token {
                    "uci" => {
                        self.engine.uci();
                    }
                    "debug" => {
                        let val = *token_stream.next().expect("Expect 'on' or 'off', got nothing");
                        assert!(val != "on" || val != "off", "Expect 'on' or 'off', got {}", val);
                        self.engine.debug(val == "on");
                    }
                    "isready" => {
                        self.engine.isready();
                    }
                    "setoption" => {
                        self.engine.setoption((&mut token_stream).into());
                    }
                    // Probably not needed
                    // "register" => {
                    //     todo!("handle registration")
                    // }
                    "ucinewgame" => {
                        self.engine.ucinewgame();
                    }
                    "position" => {
                        self.engine.position((&mut token_stream).into());
                    }
                    "go" => {
                        self.engine.go((&mut token_stream).into());
                    }
                    "stop" => {
                        self.engine.stop();
                    }
                    "ponderhit" => {
                        self.engine.ponderhit();
                    }
                    "quit" => {
                        self.engine.quit();
                        quit = true;
                        break;
                    }
                    _ => {
                        print!("Unknown command {}", token);
                        continue;
                    }
                }

                break;
            }

            if quit {
                break
            }
        }
    }
}