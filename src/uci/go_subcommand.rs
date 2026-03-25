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
    fn from(value: &mut Iter<'_, &str>) -> Self {
        let mut res = Self {
            searchmoves: None,
            ponder: false,
            wtime: None, 
            btime: None,
            winc: None,
            binc: None,
            movestogo: None,
            depth: None,
            nodes: None,
            mate: None,
            movetime: None,
            infinite: false
        };

        while let Some(tok) = value.next() {
            match *tok {
                "searchmoves" => {
                    let searchmoves = value.map(|e| UCIMove::from(*e)).collect();
                    res.searchmoves = Some(searchmoves);
                }, 
                "ponder" => {
                    res.ponder = true;
                },
                "wtime" => {
                    let wtime = value.next().expect("Expect wtime, got nothing");
                    res.wtime = Some(Duration::from_millis(wtime.parse::<u64>().expect("Expect u64")));
                }, 
                "btime" => {
                    let btime = value.next().expect("Expect btime, got nothing");
                    res.btime = Some(Duration::from_millis(btime.parse::<u64>().expect("Expect u64")));
                }, 
                "winc" => {
                    let winc = value.next().expect("Expect winc, got nothing");
                    res.winc = Some(Duration::from_millis(winc.parse::<u64>().expect("Expect u64")));
                }, 
                "binc" => {
                    let binc = value.next().expect("Expect binc, got nothing");
                    res.binc = Some(Duration::from_millis(binc.parse::<u64>().expect("Expect u64")));
                }, 
                "movestogo" => {
                    let movestogo = value.next().expect("Expect movestogo, got nothing");
                    res.movestogo = Some(NonZeroU32::new(movestogo.parse::<u32>().expect("Expect u32")).expect("Expect non-zero value"));
                },
                "depth" => {
                    let depth = value.next().expect("Expect depth, got nothing");
                    res.depth = Some(depth.parse::<u32>().expect("Expect u32"));
                },
                "nodes" => {
                    let nodes = value.next().expect("Expect nodes, got nothing");
                    res.nodes = Some(nodes.parse::<u32>().expect("Expect u32"));
                },
                "mate" => {
                    let mate = value.next().expect("Expect mate, got nothing");
                    res.mate = Some(mate.parse::<u32>().expect("Expect u32"));
                },
                "movetime" => {
                    let movetime = value.next().expect("Expect movetime, got nothing");
                    res.movetime = Some(movetime.parse::<u32>().expect("Expect u32"));
                },
                "infinite" => {
                    res.infinite = true;
                },
                _ => {
                    panic!("Unknown token received in go command");
                }
            }
        }
        
        res
    }
}