use std::{num::NonZeroU32, slice::Iter, time::Duration};

use crate::uci::position::UCIMove;

pub struct UCIGoSubcommand {
    searchmoves: Option<Vec<UCIMove>>,
    ponder: bool,
    wtime: Option<Duration>, 
    btime: Option<Duration>, 
    winc: Option<Duration>, 
    binc: Option<Duration>, 
    movestogo: Option<NonZeroU32>, 
    depth: Option<u32>,
    nodes: Option<u32>,
    mate: Option<u32>,
    movetime: Option<u32>,
    infinite: bool,
}


impl From<&mut Iter<'_, &str>> for UCIGoSubcommand {
    fn from(_value: &mut Iter<'_, &str>) -> Self {
        todo!("Implement this parser")
    }
}