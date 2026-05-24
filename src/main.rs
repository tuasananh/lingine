use anyhow::Result;
use simple_logger::SimpleLogger;

use crate::{bot::PrintBot, uci::UCIHandler};

#[cfg(test)]
mod benchmark;
mod bitboard;
mod bot;
mod movegen;
mod position;
mod types;
mod uci;

fn main() -> Result<()> {
    SimpleLogger::new().init()?;
    let bot = PrintBot;
    let uci_handler = UCIHandler::new(bot);
    let reader = std::io::stdin().lock();
    uci_handler.run(reader)?;
    Ok(())
}
