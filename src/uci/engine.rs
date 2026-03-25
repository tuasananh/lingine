use crate::uci::{go_subcommand::UCIGoSubcommand, option::UCISetOption, position::UCIPosition};

pub trait UCIEngine {
    fn new() -> Self;

    fn uci(&self);

    fn debug(&self, is_on: bool);

    fn isready(&self);
    
    fn setoption(&self, option: UCISetOption);

    fn ucinewgame(&self);

    fn position(&self, position: UCIPosition);

    fn go(&self, position: UCIGoSubcommand);

    fn stop(&self);

    fn ponderhit(&self);

    fn quit(&self);
}