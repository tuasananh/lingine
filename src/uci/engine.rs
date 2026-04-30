use anyhow::Result;

use crate::uci::{go_subcommand::UCIGoSubcommand, option::UCISetOption, position::UCIPosition};

pub trait UCIEngine {
    fn new() -> Result<Self>;

    fn uci(&self) -> Result<()>;

    fn debug(&self, is_on: bool) -> Result<()>;

    fn isready(&self) -> Result<()>;

    fn setoption(&self, option: UCISetOption) -> Result<()>;

    fn ucinewgame(&self) -> Result<()>;

    fn position(&mut self, position: UCIPosition) -> Result<()>;

    fn go(&self, position: UCIGoSubcommand) -> Result<()>;

    fn stop(&self) -> Result<()>;

    fn ponderhit(&self) -> Result<()>;

    fn quit(&self) -> Result<()>;
}
