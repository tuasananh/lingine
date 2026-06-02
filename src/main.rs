use anyhow::Result;
use simple_logger::SimpleLogger;

use lingine::{bot::Lingine, uci::Handler};

fn main() -> Result<()> {
    SimpleLogger::new().init()?;
    let bot = Lingine::new();
    let uci_handler = Handler::new(bot);
    let reader = std::io::stdin().lock();
    uci_handler.run(reader)?;
    Ok(())
}
