use crate::uci::{Engine, GoParameters, Position, RegisterParameters, SetOptionParameters};

use anyhow::Result;

pub struct PrintBot {}

impl PrintBot {
    pub fn new() -> PrintBot {
        PrintBot {}
    }
}

impl Engine for PrintBot {
    fn uci(&self) -> Result<()> {
        println!("uci called!");
        Ok(())
    }

    fn debug(&self, _is_on: bool) -> Result<()> {
        println!("debug called!");
        Ok(())
    }

    fn isready(&self) -> Result<()> {
        println!("isready called!");
        Ok(())
    }

    fn setoption(&self, option: SetOptionParameters) -> Result<()> {
        println!(
            "Set option: name={:?} value={:?}",
            option.name, option.value
        );
        Ok(())
    }

    fn ucinewgame(&self) -> Result<()> {
        println!("ucinewgame called!");
        Ok(())
    }

    fn position(&self, position: Position) -> Result<()> {
        println!(
            "position called with fen={:?} moves={:?}!",
            position.fen, position.moves
        );
        if let Some(first_move) = position.moves.first() {
            println!("First move as u32: {:?}", first_move.as_u32());
        }

        Ok(())
    }

    fn go(&self, params: GoParameters) -> Result<()> {
        println!("go called {:?}", params);
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        println!("stop called");
        Ok(())
    }

    fn ponderhit(&self) -> Result<()> {
        println!("ponderhit called");
        Ok(())
    }

    fn quit(&self) -> Result<()> {
        println!("Quit called");
        Ok(())
    }

    fn register(&self, params: RegisterParameters) -> Result<()> {
        match params {
            RegisterParameters::Later => println!("register later"),
            RegisterParameters::Identity { name, code } => {
                println!("register name={:?} code={:?}", name, code)
            }
        };
        Ok(())
    }
}
