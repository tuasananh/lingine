use anyhow::Result;

use lingine::bot::Lingine;
use lingine::uci::message_loop;

fn main() -> Result<()> {
    let bot = Lingine::new();
    message_loop(bot);
    Ok(())
}
