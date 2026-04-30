use std::{num::NonZeroU32, slice::Iter, time::Duration};

use crate::types::Move;

#[derive(Clone, Debug, Default)]
pub struct UCIGoSubcommand {
    searchmoves: Option<Vec<Move>>,
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
            infinite: false,
        };

        while let Some(tok) = value.next() {
            match *tok {
                "searchmoves" => {
                    let mut searchmoves = Vec::new();

                    while let Some(next_token) = value.clone().next() {
                        match *next_token {
                            "ponder" | "wtime" | "btime" | "winc" | "binc" | "movestogo"
                            | "depth" | "nodes" | "mate" | "movetime" | "infinite" => {
                                break;
                            }
                            _ => {
                                searchmoves.push(Move::from(
                                    *value.next().expect("Expect search move, got nothing"),
                                ));
                            }
                        }
                    }

                    res.searchmoves = Some(searchmoves);
                }
                "ponder" => {
                    res.ponder = true;
                }
                "wtime" => {
                    let wtime = value.next().expect("Expect wtime, got nothing");
                    res.wtime = Some(Duration::from_millis(
                        wtime.parse::<u64>().expect("Expect u64"),
                    ));
                }
                "btime" => {
                    let btime = value.next().expect("Expect btime, got nothing");
                    res.btime = Some(Duration::from_millis(
                        btime.parse::<u64>().expect("Expect u64"),
                    ));
                }
                "winc" => {
                    let winc = value.next().expect("Expect winc, got nothing");
                    res.winc = Some(Duration::from_millis(
                        winc.parse::<u64>().expect("Expect u64"),
                    ));
                }
                "binc" => {
                    let binc = value.next().expect("Expect binc, got nothing");
                    res.binc = Some(Duration::from_millis(
                        binc.parse::<u64>().expect("Expect u64"),
                    ));
                }
                "movestogo" => {
                    let movestogo = value.next().expect("Expect movestogo, got nothing");
                    res.movestogo = Some(
                        NonZeroU32::new(movestogo.parse::<u32>().expect("Expect u32"))
                            .expect("Expect non-zero value"),
                    );
                }
                "depth" => {
                    let depth = value.next().expect("Expect depth, got nothing");
                    res.depth = Some(depth.parse::<u32>().expect("Expect u32"));
                }
                "nodes" => {
                    let nodes = value.next().expect("Expect nodes, got nothing");
                    res.nodes = Some(nodes.parse::<u32>().expect("Expect u32"));
                }
                "mate" => {
                    let mate = value.next().expect("Expect mate, got nothing");
                    res.mate = Some(mate.parse::<u32>().expect("Expect u32"));
                }
                "movetime" => {
                    let movetime = value.next().expect("Expect movetime, got nothing");
                    res.movetime = Some(movetime.parse::<u32>().expect("Expect u32"));
                }
                "infinite" => {
                    res.infinite = true;
                }
                _ => {
                    // Unknown go tokens are ignored per UCI convention.
                }
            }
        }

        res
    }
}

#[cfg(test)]
mod tests {
    use super::UCIGoSubcommand;
    use std::time::Duration;

    #[test]
    fn parses_searchmoves_and_following_options() {
        let tokens = "searchmoves a0a1 b0b1 wtime 1000 ponder"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UCIGoSubcommand::from(&mut iter);

        assert!(parsed.searchmoves.is_some());
        assert_eq!(parsed.searchmoves.unwrap().len(), 2);
        assert!(parsed.ponder);
        assert_eq!(parsed.movetime, None);
        assert!(!parsed.infinite);
    }

    #[test]
    fn parses_time_related_parameters() {
        let tokens = "wtime 1200 btime 3400 winc 100 binc 200 movestogo 30 depth 4 nodes 1000 mate 2 movetime 1500 infinite"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UCIGoSubcommand::from(&mut iter);

        assert_eq!(parsed.wtime, Some(Duration::from_millis(1200)));
        assert_eq!(parsed.btime, Some(Duration::from_millis(3400)));
        assert_eq!(parsed.winc, Some(Duration::from_millis(100)));
        assert_eq!(parsed.binc, Some(Duration::from_millis(200)));
        assert_eq!(parsed.movestogo.map(|v| v.get()), Some(30));
        assert_eq!(parsed.depth, Some(4));
        assert_eq!(parsed.nodes, Some(1000));
        assert_eq!(parsed.mate, Some(2));
        assert_eq!(parsed.movetime, Some(1500));
        assert!(parsed.infinite);
    }

    #[test]
    fn ignores_unknown_tokens_and_parses_known_ones() {
        let tokens = "unknown_token depth 3"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UCIGoSubcommand::from(&mut iter);

        assert_eq!(parsed.depth, Some(3));
    }
}
