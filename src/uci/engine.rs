use anyhow::Result;

use crate::uci::{GoParameters, Position, RegisterParameters, SetOptionParameters};

pub trait Engine {
    fn uci(&self) -> Result<()>;

    fn debug(&self, is_on: bool) -> Result<()>;

    fn isready(&self) -> Result<()>;

    fn setoption(&self, params: SetOptionParameters) -> Result<()>;

    fn ucinewgame(&self) -> Result<()>;

    fn register(&self, params: RegisterParameters) -> Result<()>;

    fn position(&self, position: Position) -> Result<()>;

    fn go(&self, params: GoParameters) -> Result<()>;

    fn stop(&self) -> Result<()>;

    fn ponderhit(&self) -> Result<()>;

    fn quit(&self) -> Result<()>;
}
