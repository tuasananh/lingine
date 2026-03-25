use crate::uci::{engine::UCIEngine, handler::UCIHandler};

mod uci;
mod utils;

struct TodoBot {

}

impl UCIEngine for TodoBot {
    fn new() -> Self {
        todo!()
    }

    fn uci(&self) {
        todo!()
    }

    fn debug(&self, _is_on: bool) {
        todo!()
    }

    fn isready(&self) {
        todo!()
    }

    fn setoption(&self, _option: uci::option::UCISetOption) {
        todo!()
    }

    fn ucinewgame(&self) {
        todo!()
    }

    fn position(&self, _position: uci::position::UCIPosition) {
        todo!()
    }

    fn go(&self, _position: uci::go_subcommand::UCIGoSubcommand) {
        todo!()
    }

    fn stop(&self) {
        todo!()
    }

    fn ponderhit(&self) {
        todo!()
    }

    fn quit(&self) {
        todo!()
    }
}

fn main() {
    let uci_handler: UCIHandler<TodoBot> = UCIHandler::new();
    uci_handler.run();
}
