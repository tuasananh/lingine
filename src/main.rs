use anyhow::Result;

use crate::{bot::PrintBot, uci::UCIHandler};

mod bot;
mod uci;

fn main() -> Result<()> {
    let bot = PrintBot::new();
    let uci_handler = UCIHandler::new(bot);
    let reader = std::io::stdin().lock();
    uci_handler.run(reader)?;
    Ok(())
}
