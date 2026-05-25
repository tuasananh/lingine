#![allow(dead_code)]

use anyhow::Result;
use simple_logger::SimpleLogger;

use lingine::{bot::PrintBot, uci::UCIHandler};

fn main() -> Result<()> {
    SimpleLogger::new().init()?;
    let bot = PrintBot;
    let uci_handler = UCIHandler::new(bot);
    let reader = std::io::stdin().lock();
    uci_handler.run(reader)?;
    Ok(())
}
