use std::{num::NonZeroU32, slice::Iter, sync::Arc, sync::atomic::AtomicBool, time::Duration};

use anyhow::{Error, Result, anyhow};

use crate::uci::UciMove;

#[derive(Clone, Debug, Default)]
pub struct GoParameters {
    /// Restrict search to these moves only.
    pub searchmoves: Option<Vec<UciMove>>,
    /// Search in pondering mode.
    pub ponder: bool,
    /// Time remaining for White in milliseconds.
    pub wtime: Option<Duration>,
    /// Time remaining for Black in milliseconds.
    pub btime: Option<Duration>,
    /// White's increment per move in milliseconds.
    pub winc: Option<Duration>,
    /// Black's increment per move in milliseconds.
    pub binc: Option<Duration>,
    /// Moves until the next time control (only sent when > 0).
    pub movestogo: Option<NonZeroU32>,
    /// Search to this depth in plies only.
    pub depth: Option<u32>,
    /// Search at most this many nodes.
    pub nodes: Option<u64>,
    /// Search for a forced mate in this many moves.
    pub mate: Option<u32>,
    /// Search for exactly this many milliseconds.
    pub movetime: Option<Duration>,
    /// Search until a `stop` command is received.
    pub infinite: bool,
    /// Shared flag set by the handler when `stop` is received.
    ///
    /// The engine's `go` implementation should check this periodically and
    /// exit the search loop when it is `true`.
    pub stop: Arc<AtomicBool>,
}

impl TryFrom<&mut Iter<'_, &str>> for GoParameters {
    type Error = Error;
    fn try_from(value: &mut Iter<'_, &str>) -> Result<Self> {
        let mut res = Self::default();

        while let Some(tok) = value.next() {
            match *tok {
                "searchmoves" => {
                    let mut searchmoves = Vec::new();

                    for next_token in value.by_ref() {
                        match *next_token {
                            "ponder" | "wtime" | "btime" | "winc" | "binc" | "movestogo"
                            | "depth" | "nodes" | "mate" | "movetime" | "infinite" => {
                                break;
                            }
                            _ => {
                                searchmoves.push((*next_token).try_into()?);
                            }
                        }
                    }

                    res.searchmoves = Some(searchmoves);
                }
                "ponder" => {
                    res.ponder = true;
                }
                "wtime" => {
                    let val = value
                        .next()
                        .ok_or_else(|| anyhow!("missing value for 'wtime'"))?;
                    let ms = val
                        .parse::<u64>()
                        .map_err(|_| anyhow!("invalid 'wtime' value: {val}"))?;
                    res.wtime = Some(Duration::from_millis(ms));
                }
                "btime" => {
                    let val = value
                        .next()
                        .ok_or_else(|| anyhow!("missing value for 'btime'"))?;
                    let ms = val
                        .parse::<u64>()
                        .map_err(|_| anyhow!("invalid 'btime' value: {val}"))?;
                    res.btime = Some(Duration::from_millis(ms));
                }
                "winc" => {
                    let val = value
                        .next()
                        .ok_or_else(|| anyhow!("missing value for 'winc'"))?;
                    let ms = val
                        .parse::<u64>()
                        .map_err(|_| anyhow!("invalid 'winc' value: {val}"))?;
                    res.winc = Some(Duration::from_millis(ms));
                }
                "binc" => {
                    let val = value
                        .next()
                        .ok_or_else(|| anyhow!("missing value for 'binc'"))?;
                    let ms = val
                        .parse::<u64>()
                        .map_err(|_| anyhow!("invalid 'binc' value: {val}"))?;
                    res.binc = Some(Duration::from_millis(ms));
                }
                "movestogo" => {
                    let val = value
                        .next()
                        .ok_or_else(|| anyhow!("missing value for 'movestogo'"))?;
                    let n = val
                        .parse::<u32>()
                        .map_err(|_| anyhow!("invalid 'movestogo' value: {val}"))?;
                    res.movestogo =
                        Some(NonZeroU32::new(n).ok_or_else(|| anyhow!("'movestogo' must be > 0"))?);
                }
                "depth" => {
                    let val = value
                        .next()
                        .ok_or_else(|| anyhow!("missing value for 'depth'"))?;
                    res.depth = Some(
                        val.parse::<u32>()
                            .map_err(|_| anyhow!("invalid 'depth' value: {val}"))?,
                    );
                }
                "nodes" => {
                    let val = value
                        .next()
                        .ok_or_else(|| anyhow!("missing value for 'nodes'"))?;
                    res.nodes = Some(
                        val.parse::<u64>()
                            .map_err(|_| anyhow!("invalid 'nodes' value: {val}"))?,
                    );
                }
                "mate" => {
                    let val = value
                        .next()
                        .ok_or_else(|| anyhow!("missing value for 'mate'"))?;
                    res.mate = Some(
                        val.parse::<u32>()
                            .map_err(|_| anyhow!("invalid 'mate' value: {val}"))?,
                    );
                }
                "movetime" => {
                    let val = value
                        .next()
                        .ok_or_else(|| anyhow!("missing value for 'movetime'"))?;
                    let ms = val
                        .parse::<u64>()
                        .map_err(|_| anyhow!("invalid 'movetime' value: {val}"))?;
                    res.movetime = Some(Duration::from_millis(ms));
                }
                "infinite" => {
                    res.infinite = true;
                }
                _ => {
                    // Unknown go tokens are ignored per UCI convention.
                }
            }
        }

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::GoParameters;
    use std::time::Duration;

    #[test]
    fn parses_searchmoves_and_following_options() {
        let tokens = "searchmoves a0a1 b0b1 wtime 1000 ponder"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = GoParameters::try_from(&mut iter).unwrap();

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

        let parsed = GoParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.wtime, Some(Duration::from_millis(1200)));
        assert_eq!(parsed.btime, Some(Duration::from_millis(3400)));
        assert_eq!(parsed.winc, Some(Duration::from_millis(100)));
        assert_eq!(parsed.binc, Some(Duration::from_millis(200)));
        assert_eq!(parsed.movestogo.map(|v| v.get()), Some(30));
        assert_eq!(parsed.depth, Some(4));
        assert_eq!(parsed.nodes, Some(1000));
        assert_eq!(parsed.mate, Some(2));
        assert_eq!(parsed.movetime, Some(Duration::from_millis(1500)));
        assert!(parsed.infinite);
    }

    #[test]
    fn ignores_unknown_tokens_and_parses_known_ones() {
        let tokens = "unknown_token depth 3"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = GoParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.depth, Some(3));
    }

    #[test]
    fn returns_error_on_missing_wtime_value() {
        let tokens = "wtime".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();
        assert!(GoParameters::try_from(&mut iter).is_err());
    }

    #[test]
    fn returns_error_on_invalid_depth_value() {
        let tokens = "depth abc".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();
        assert!(GoParameters::try_from(&mut iter).is_err());
    }

    #[test]
    fn returns_error_on_zero_movestogo() {
        let tokens = "movestogo 0".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();
        assert!(GoParameters::try_from(&mut iter).is_err());
    }
}
