use anyhow::Result;
use simple_logger::SimpleLogger;

use crate::{bot::PrintBot, uci::UCIHandler};

mod bot;
mod uci;

fn main() -> Result<()> {
    SimpleLogger::new().init()?;
    let bot = PrintBot::default();
    let uci_handler = UCIHandler::new(bot);
    let reader = std::io::stdin().lock();
    uci_handler.run(reader)?;
    Ok(())
}
