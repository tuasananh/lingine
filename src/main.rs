#![allow(dead_code)]

use anyhow::Result;
use simple_logger::SimpleLogger;

use lingine::{bot::EngineBot, uci::UCIHandler};

fn main() -> Result<()> {
    SimpleLogger::new().init()?;
    let bot = EngineBot::new();
    let uci_handler = UCIHandler::new(bot);
    let reader = std::io::stdin().lock();
    uci_handler.run(reader)?;
    Ok(())
}
