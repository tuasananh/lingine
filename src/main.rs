#![feature(adt_const_params)]

use anyhow::Result;

use crate::{
    position::Position,
    uci::{engine::UCIEngine, handler::UCIHandler, position::UCIPosition},
};

mod benchmark;
mod bitboard;
mod movegen;
mod position;
mod types;
mod uci;

struct RandomBot {
    position: Position,
}

impl UCIEngine for RandomBot {
    fn new() -> Result<Self> {
        Ok(Self {
            position: Position::new(),
        })
    }

    fn uci(&self) -> Result<()> {
        println!("id name RandomBot");
        println!("id author tuasananh");
        println!("uciok");
        Ok(())
    }

    fn debug(&self, _is_on: bool) -> Result<()> {
        todo!()
    }

    fn isready(&self) -> Result<()> {
        println!("readyok");
        Ok(())
    }

    fn setoption(&self, _option: uci::option::UCISetOption) -> Result<()> {
        todo!()
    }

    fn ucinewgame(&self) -> Result<()> {
        // do nothing
        todo!();
    }

    fn position(&mut self, position: UCIPosition) -> Result<()> {
        self.position.set(&position.fen);
        Ok(())
    }

    fn go(&self, _position: uci::go_subcommand::UCIGoSubcommand) -> Result<()> {
        todo!()
    }

    fn stop(&self) -> Result<()> {
        todo!()
    }

    fn ponderhit(&self) -> Result<()> {
        todo!()
    }

    fn quit(&self) -> Result<()> {
        todo!()
    }
}

fn main() -> Result<()> {
    let uci_handler: UCIHandler<RandomBot> = UCIHandler::new()?;
    uci_handler.run();
    Ok(())
}
