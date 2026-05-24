use std::{num::NonZeroU32, slice::Iter, sync::Arc, sync::atomic::AtomicBool, time::Duration};

use anyhow::{Error, Result, anyhow};

use crate::uci::UciMove;

/// Parameters parsed from a `go` UCI command.
///
/// All fields are `pub` so the engine's `go` implementation can read them
/// directly without getters.
///
/// # Time management
/// The engine is solely responsible for respecting these limits. The handler
/// imposes no wall-clock timeout. A typical priority order is:
/// `movetime` > `infinite` > `wtime`/`btime` + `winc`/`binc` + `movestogo` >
/// `depth` > `nodes`.
///
/// # Limitations
/// - `nodes` is `u64` but `depth`, `mate`, and `movetime`-as-milliseconds use
///   narrower integer types. A GUI sending an out-of-range value will get an
///   `Err` from `TryFrom`, which the handler logs and discards.
/// - `ponder` flag is parsed but there is no ponder-mode search yet.
#[derive(Clone, Debug, Default)]
pub struct GoParameters {
    /// Restrict search to these moves only at the root.
    pub searchmoves: Option<Vec<UciMove>>,
    /// If `true`, search in pondering mode (opponent's turn on the clock).
    ///
    /// # Limitation
    /// Ponder mode is not yet implemented. This flag is parsed and stored but
    /// the engine does not alter its behaviour based on it.
    pub ponder: bool,
    /// Time remaining for White.
    pub wtime: Option<Duration>,
    /// Time remaining for Black.
    pub btime: Option<Duration>,
    /// White's increment added after each move.
    pub winc: Option<Duration>,
    /// Black's increment added after each move.
    pub binc: Option<Duration>,
    /// Number of moves until the next time control. `NonZeroU32` because the
    /// UCI spec only sends this field when the value is greater than zero.
    pub movestogo: Option<NonZeroU32>,
    /// Search to at most this depth in plies.
    pub depth: Option<u32>,
    /// Search at most this many nodes.
    pub nodes: Option<u64>,
    /// Search for a forced mate in this many moves (half-moves = plies / 2).
    pub mate: Option<u32>,
    /// Search for exactly this long, ignoring all other time parameters.
    pub movetime: Option<Duration>,
    /// Search until a `stop` command arrives. The engine must still honour
    /// [`stop`][Self::stop] when set.
    pub infinite: bool,
    /// Shared stop signal injected by the engine actor just before `engine.go()`
    /// is called.
    ///
    /// Thread A (stdin reader) calls `stop_flag.store(true, SeqCst)` when it
    /// receives a `stop` command. The engine's search loop must check this
    /// periodically:
    ///
    /// ```rust,ignore
    /// if params.stop.load(Ordering::Relaxed) {
    ///     break; // return best move found so far
    /// }
    /// ```
    ///
    /// The `Default` value is a fresh, unshared `Arc<AtomicBool>` — it is
    /// replaced by the real shared flag in the actor's `Go` arm.
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

                    // Use as_slice().first() to *peek* at the next token
                    // without consuming it. This ensures that when we see a
                    // keyword like `wtime`, we break *before* pulling it out
                    // of the iterator so the outer loop can match and parse it.
                    loop {
                        match value.as_slice().first().copied() {
                            // Stop before any known go-parameter keyword.
                            Some(
                                "ponder" | "wtime" | "btime" | "winc" | "binc" | "movestogo"
                                | "depth" | "nodes" | "mate" | "movetime" | "infinite",
                            )
                            | None => break,
                            // Anything else is a move string; consume and parse.
                            Some(_) => {
                                let tok = value.next().unwrap();
                                searchmoves.push((*tok).try_into()?);
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
        // wtime must be parsed even though searchmoves appeared before it.
        assert_eq!(parsed.wtime, Some(Duration::from_millis(1000)));
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
