use std::{fmt, num::NonZeroU32, slice::Iter, sync::Arc, sync::atomic::AtomicBool, time::Duration};

use anyhow::{Error, Result, anyhow, ensure};

// ===========================================================================
// Move
// ===========================================================================

/// A chess move as represented in the UCI protocol.
///
/// Stored as a `u32` with four byte-sized fields:
/// - bits  0– 7: source file (`src_file`, 0 = `'a'`, …, 8 = `'i'`)
/// - bits  8–15: source rank (`src_rank`, 0–9)
/// - bits 16–23: destination file (`dst_file`)
/// - bits 24–31: destination rank (`dst_rank`)
///
/// A null move is encoded as `0x00000000` (the string `"0000"`).
///
/// Xiangqi has no promotion, so no promotion piece field is encoded.
#[derive(Clone, Debug)]
pub struct Move(u32);

impl Move {
    /// Source file index (0 = 'a', …, 8 = 'i').
    pub fn src_file(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Source rank index (0–9).
    pub fn src_rank(&self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }

    /// Destination file index (0 = 'a', …, 8 = 'i').
    pub fn dst_file(&self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }

    /// Destination rank index (0–9).
    pub fn dst_rank(&self) -> u8 {
        ((self.0 >> 24) & 0xFF) as u8
    }

    /// Returns `true` if this is the null move (`"0000"`).
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

impl PartialEq for Move {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Move {}

impl TryFrom<&str> for Move {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        let value = value.as_bytes();
        ensure!(
            value.len() == 4,
            "A move must be 4 characters, got {} characters",
            value.len()
        );

        // Null move: the string "0000".
        if value == b"0000" {
            return Ok(Self(0));
        }

        ensure!(
            b'a' <= value[0] && value[0] <= b'i',
            "Source file must be between 'a' and 'i'"
        );
        ensure!(
            b'0' <= value[1] && value[1] <= b'9',
            "Source rank must be between '0' and '9'"
        );
        ensure!(
            b'a' <= value[2] && value[2] <= b'i',
            "Destination file must be between 'a' and 'i'"
        );
        ensure!(
            b'0' <= value[3] && value[3] <= b'9',
            "Destination rank must be between '0' and '9'"
        );

        let from_file = (value[0] - b'a') as u32;
        let from_rank = (value[1] - b'0') as u32;
        let to_file = (value[2] - b'a') as u32;
        let to_rank = (value[3] - b'0') as u32;

        Ok(Self(
            from_file | (from_rank << 8) | (to_file << 16) | (to_rank << 24),
        ))
    }
}

// ===========================================================================
// GoParameters
// ===========================================================================

/// Parameters parsed from a `go` UCI command.
///
/// All fields are `pub` so the engine's `go` implementation can read them
/// directly without getters.
#[derive(Clone, Debug, Default)]
pub struct GoParameters {
    /// Restrict search to these moves only at the root.
    pub searchmoves: Option<Vec<Move>>,
    /// If `true`, search in pondering mode (opponent's turn on the clock).
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
    /// Do perft from current position up to depth
    pub perft: Option<u32>,
    /// Shared stop signal injected by the engine actor just before
    /// `engine.go()` is called.
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
                    // without consuming it.
                    loop {
                        match value.as_slice().first().copied() {
                            // Stop before any known go-parameter keyword.
                            Some(
                                "ponder" | "wtime" | "btime" | "winc" | "binc" | "movestogo"
                                | "depth" | "nodes" | "mate" | "movetime" | "infinite" | "perft",
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
                "perft" => {
                    let val = value
                        .next()
                        .ok_or_else(|| anyhow!("missing value for 'perft'"))?;
                    res.perft = Some(
                        val.parse::<u32>()
                            .map_err(|_| anyhow!("invalid 'perft' value: {val}"))?,
                    );
                }
                _ => {
                    // Unknown go tokens are ignored per UCI convention.
                }
            }
        }

        Ok(res)
    }
}

// ===========================================================================
// SetOptionParameters
// ===========================================================================

/// Parsed parameters from the `setoption` UCI command.
///
/// The `name` field is always **lowercased** because the UCI spec says option
/// names are case-insensitive.
#[derive(Debug)]
pub struct SetOptionParameters {
    /// Option name, lowercased and joined with spaces for multi-word names.
    pub name: String,
    /// Option value, or `None` for button-type options (no `value` keyword).
    pub value: Option<String>,
}

impl TryFrom<&mut Iter<'_, &str>> for SetOptionParameters {
    type Error = Error;
    fn try_from(value: &mut Iter<'_, &str>) -> Result<Self> {
        let name_token = value.next().ok_or(anyhow!("Expect 'name', got nothing"))?;
        ensure!(name_token == &"name", "Expect 'name', got {}", name_token);
        let mut name_tokens = Vec::new();
        let mut last_token_is_value = false;
        for token in value.by_ref() {
            if token == &"value" {
                last_token_is_value = true;
                break;
            }
            name_tokens.push(*token);
        }
        ensure!(!name_tokens.is_empty(), "Option name must not be empty");
        let name = name_tokens.join(" ").to_lowercase();

        let value = if last_token_is_value {
            let collected = value
                .map(|tok| (*tok).to_string())
                .collect::<Vec<String>>()
                .join(" ");
            ensure!(!collected.is_empty(), "Option value must not be empty");
            Some(collected)
        } else {
            None
        };

        Ok(Self { name, value })
    }
}

// ===========================================================================
// RegisterParameters
// ===========================================================================

/// Parsed parameters from the `register` UCI command.
///
/// Lingine does not implement copy-protection, so `register` is effectively a
/// no-op.
pub enum RegisterParameters {
    /// `register later` — engine is not yet registered; try again later.
    Later,
    /// `register [name <n>] [code <c>]` — identity and/or registration code.
    Identity {
        name: Option<String>,
        code: Option<String>,
    },
}

impl TryFrom<&mut Iter<'_, &str>> for RegisterParameters {
    type Error = Error;

    fn try_from(iter: &mut Iter<'_, &str>) -> Result<Self> {
        if let Some(&first) = iter.clone().next()
            && first == "later"
        {
            iter.next(); // Consume "later"
            return Ok(RegisterParameters::Later);
        }

        let mut name_tokens = Vec::new();
        let mut code = None;
        let mut parsing_name = false;

        while let Some(&token) = iter.next() {
            match token {
                "name" => {
                    parsing_name = true;
                }
                "code" => {
                    parsing_name = false;
                    code = iter.next().map(|s| s.to_string());
                }
                _ => {
                    if parsing_name {
                        name_tokens.push(token);
                    }
                }
            }
        }

        let name = if name_tokens.is_empty() {
            None
        } else {
            Some(name_tokens.join(" "))
        };

        Ok(RegisterParameters::Identity { name, code })
    }
}

// ===========================================================================
// PositionParameters
// ===========================================================================

/// The starting FEN for a standard Xiangqi game.
pub const START_FEN: &str = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w";

/// The board position sent by the GUI via the `position` command.
#[derive(Clone, Debug)]
pub struct PositionParameters {
    /// The starting position in Forsyth-Edwards Notation.
    pub fen: String,
    /// Moves applied after the FEN position, in order.
    pub moves: Vec<Move>,
}

impl Default for PositionParameters {
    fn default() -> Self {
        Self {
            fen: START_FEN.to_string(),
            moves: Vec::new(),
        }
    }
}

impl TryFrom<&mut Iter<'_, &str>> for PositionParameters {
    type Error = Error;
    fn try_from(value: &mut Iter<'_, &str>) -> Result<Self> {
        let next_token = value.next();

        ensure!(
            next_token.is_some(),
            "Expect 'fen' or 'startpos', but got nothing"
        );
        let next_token = *next_token.unwrap();
        ensure!(
            next_token == "fen" || next_token == "startpos",
            "Expect 'fen' or 'startpos', but got {}",
            next_token
        );

        let fen = if next_token == "fen" {
            let mut val = String::new();
            while let Some(tok) = value.next()
                && *tok != "moves"
            {
                if !val.is_empty() {
                    val += " ";
                }
                val += *tok;
            }
            ensure!(!val.is_empty(), "Expect fen string, got nothing");
            val
        } else {
            for &tok in value.by_ref() {
                if tok == "moves" {
                    break;
                }
            }
            START_FEN.to_string()
        };

        let moves = value
            .map(|tok| Move::try_from(*tok))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { fen, moves })
    }
}

// ===========================================================================
// Responses / Output Structs
// ===========================================================================

/// Identity information sent in response to the `uci` command.
pub struct UciId {
    pub name: String,
    pub author: String,
}

impl fmt::Display for UciId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id name {}\nid author {}", self.name, self.author)
    }
}

/// A single engine option advertised to the GUI after the `uci` command.
pub enum UciOption {
    /// A boolean checkbox.
    Check { name: String, default: bool },

    /// An integer spin-wheel in a closed range.
    Spin {
        name: String,
        default: i64,
        min: i64,
        max: i64,
    },

    /// A combo-box with a fixed set of string choices.
    Combo {
        name: String,
        default: String,
        vars: Vec<String>,
    },

    /// A push-button with no value.
    Button { name: String },

    /// A free-form text field.
    Str { name: String, default: String },
}

impl fmt::Display for UciOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UciOption::Check { name, default } => {
                write!(f, "option name {name} type check default {default}")
            }
            UciOption::Spin {
                name,
                default,
                min,
                max,
            } => {
                write!(
                    f,
                    "option name {name} type spin default {default} min {min} max {max}"
                )
            }
            UciOption::Combo {
                name,
                default,
                vars,
            } => {
                write!(f, "option name {name} type combo default {default}")?;
                for var in vars {
                    write!(f, " var {var}")?;
                }
                Ok(())
            }
            UciOption::Button { name } => {
                write!(f, "option name {name} type button")
            }
            UciOption::Str { name, default } => {
                let value = if default.is_empty() {
                    "<empty>"
                } else {
                    default.as_str()
                };
                write!(f, "option name {name} type string default {value}")
            }
        }
    }
}

/// Whether the score is an exact value, a lower bound, or an upper bound.
pub enum Bound {
    Lower,
    Upper,
}

/// The type of score the engine reports.
pub enum UciScore {
    /// Score in centipawns from the engine's point of view.
    Centipawns(i32),
    /// Forced mate: positive = engine mates in N moves, negative = engine gets
    /// mated in N.
    Mate(i32),
}

/// A score value together with an optional bound qualifier.
pub struct UciScoreBound {
    pub score: UciScore,
    pub bound: Option<Bound>,
}

impl fmt::Display for UciScoreBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.score {
            UciScore::Centipawns(cp) => write!(f, "score cp {cp}")?,
            UciScore::Mate(m) => write!(f, "score mate {m}")?,
        }
        match &self.bound {
            Some(Bound::Lower) => write!(f, " lowerbound")?,
            Some(Bound::Upper) => write!(f, " upperbound")?,
            None => {}
        }
        Ok(())
    }
}

/// Search information streamed from the engine to the GUI while searching.
#[derive(Default)]
pub struct UciInfo {
    /// Search depth in plies.
    pub depth: Option<u32>,
    /// Selective search depth in plies (requires `depth`).
    pub seldepth: Option<u32>,
    /// Wall-clock time spent searching so far.
    pub time: Option<Duration>,
    /// Total nodes searched.
    pub nodes: Option<u64>,
    /// The principal variation (list of move strings).
    pub pv: Option<Vec<String>>,
    /// Which PV line this belongs to in multi-PV mode.
    pub multipv: Option<u32>,
    /// The score of the position.
    pub score: Option<UciScoreBound>,
    /// The move currently being searched at the root.
    pub currmove: Option<String>,
    /// The 1-based index of `currmove` among root moves.
    pub currmovenumber: Option<u32>,
    /// Hash table utilisation in per-mille (0–1000).
    pub hashfull: Option<u32>,
    /// Nodes per second.
    pub nps: Option<u64>,
    /// Endgame tablebase hits.
    pub tbhits: Option<u64>,
    /// Shredder endgame database hits.
    pub sbhits: Option<u64>,
    /// CPU load in per-mille (0–1000).
    pub cpuload: Option<u32>,
    /// Arbitrary display string; rest of the `info` line when present.
    pub string: Option<String>,
}

impl UciInfo {
    /// Construct an empty `UciInfo` with all fields set to `None`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl fmt::Display for UciInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "info")?;

        if let Some(d) = self.depth {
            write!(f, " depth {d}")?;

            if let Some(sd) = self.seldepth {
                write!(f, " seldepth {sd}")?;
            }
        }
        if let Some(t) = self.time {
            write!(f, " time {}", t.as_millis())?;
        }
        if let Some(n) = self.nodes {
            write!(f, " nodes {n}")?;
        }
        if let Some(mpv) = self.multipv {
            write!(f, " multipv {mpv}")?;
        }
        if let Some(score) = &self.score {
            write!(f, " {score}")?;
        }
        if let Some(pv) = &self.pv {
            write!(f, " pv")?;
            for mv in pv {
                write!(f, " {mv}")?;
            }
        }
        if let Some(cm) = &self.currmove {
            write!(f, " currmove {cm}")?;
        }
        if let Some(cmn) = self.currmovenumber {
            write!(f, " currmovenumber {cmn}")?;
        }
        if let Some(hf) = self.hashfull {
            write!(f, " hashfull {hf}")?;
        }
        if let Some(nps) = self.nps {
            write!(f, " nps {nps}")?;
        }
        if let Some(tbhits) = self.tbhits {
            write!(f, " tbhits {tbhits}")?;
        }
        if let Some(sbhits) = self.sbhits {
            write!(f, " sbhits {sbhits}")?;
        }
        if let Some(cpu) = self.cpuload {
            write!(f, " cpuload {cpu}")?;
        }
        // Per spec: "if there is a string command the rest of the line will be
        // interpreted as str" so `string` must come last.
        if let Some(s) = &self.string {
            write!(f, " string {s}")?;
        }

        Ok(())
    }
}

/// The engine's chosen move, sent after every `go` command completes.
pub struct BestMove {
    /// The best move in long algebraic notation (e.g. `"e2e4"`, `"0000"` for
    /// null).
    pub mv: String,
    /// Optional move to ponder on while the opponent thinks.
    pub ponder: Option<String>,
}

impl BestMove {
    /// A null move (`0000`), used as a fallback when `go` returns an error.
    pub fn null() -> Self {
        Self {
            mv: "null".into(),
            ponder: None,
        }
    }
}

impl fmt::Display for BestMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bestmove {}", self.mv)?;
        if let Some(p) = &self.ponder {
            write!(f, " ponder {p}")?;
        }
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_move_parsing() {
        let m = Move::try_from("a0b1").expect("Should parse valid move");
        // from: file 'a' = 0, rank '0' = 0
        // to:   file 'b' = 1, rank '1' = 1
        assert_eq!(m.src_file(), 0);
        assert_eq!(m.src_rank(), 0);
        assert_eq!(m.dst_file(), 1);
        assert_eq!(m.dst_rank(), 1);
    }

    #[test]
    fn test_boundary_moves() {
        let m = Move::try_from("i9i9").expect("Should parse boundary move");
        assert_eq!(m.src_file(), 8); // 'i' - 'a' = 8
        assert_eq!(m.src_rank(), 9); // '9' - '0' = 9
        assert_eq!(m.dst_file(), 8);
        assert_eq!(m.dst_rank(), 9);
    }

    #[test]
    fn test_null_move() {
        let m = Move::try_from("0000").expect("Should parse null move");
        assert!(m.is_null());
    }

    #[test]
    fn test_invalid_length() {
        assert!(Move::try_from("a0b").is_err()); // Too short
        assert!(Move::try_from("a0b1c").is_err()); // Too long
    }

    #[test]
    fn test_invalid_characters() {
        assert!(Move::try_from("j0b1").is_err()); // Source file out of range
        assert!(Move::try_from("a:b1").is_err()); // Source rank out of range
        assert!(Move::try_from("????").is_err()); // Completely wrong
    }

    #[test]
    fn test_equality() {
        let m1 = Move::try_from("a1c3").unwrap();
        let m2 = Move::try_from("a1c3").unwrap();
        let m3 = Move::try_from("c3a1").unwrap();

        assert_eq!(m1, m2);
        assert_ne!(m1, m3);
    }

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

    #[test]
    fn parses_single_word_name_and_value() {
        let tokens = "name Hash value 128"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = SetOptionParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.name, "hash");
        assert_eq!(parsed.value.as_deref(), Some("128"));
    }

    #[test]
    fn parses_multi_word_name_and_value() {
        let tokens = "name UCI Engine About value LiNgine test build"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = SetOptionParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.name, "uci engine about");
        assert_eq!(parsed.value.as_deref(), Some("LiNgine test build"));
    }

    #[test]
    fn parses_option_without_value() {
        let tokens = "name Clear Hash".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = SetOptionParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.name, "clear hash");
        assert_eq!(parsed.value, None);
    }

    #[test]
    fn parses_later() {
        let tokens = "later".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        assert!(matches!(parsed, RegisterParameters::Later));
    }

    #[test]
    fn parses_later_and_leaves_remaining_tokens() {
        let tokens = "later name John".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        assert!(matches!(parsed, RegisterParameters::Later));
        assert_eq!(iter.next(), Some(&"name"));
        assert_eq!(iter.next(), Some(&"John"));
    }

    #[test]
    fn parses_empty_input() {
        let tokens = "".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name, None);
                assert_eq!(code, None);
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_name_only_single_word() {
        let tokens = "name Alice".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name.as_deref(), Some("Alice"));
                assert_eq!(code, None);
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_name_only_multiple_words() {
        let tokens = "name Alice Bob Smith"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name.as_deref(), Some("Alice Bob Smith"));
                assert_eq!(code, None);
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_code_only() {
        let tokens = "code XYZ-123".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name, None);
                assert_eq!(code.as_deref(), Some("XYZ-123"));
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_name_and_code() {
        let tokens = "name Alice code XYZ-123"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name.as_deref(), Some("Alice"));
                assert_eq!(code.as_deref(), Some("XYZ-123"));
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_code_then_name() {
        let tokens = "code XYZ-123 name Bob Smith"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name.as_deref(), Some("Bob Smith"));
                assert_eq!(code.as_deref(), Some("XYZ-123"));
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_code_without_value() {
        let tokens = "code".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name, None);
                assert_eq!(code, None);
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn ignores_untracked_tokens_before_name() {
        let tokens = "hello world name Alice"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name.as_deref(), Some("Alice"));
                assert_eq!(code, None);
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_startpos_without_moves() {
        let tokens = "startpos".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = PositionParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, START_FEN);
        assert!(parsed.moves.is_empty());
    }

    #[test]
    fn parses_startpos_with_moves() {
        let tokens = "startpos moves a0a1 b0b1"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = PositionParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, START_FEN);
        assert_eq!(parsed.moves.len(), 2);
    }

    #[test]
    fn parses_fen_without_moves() {
        let tokens = "fen 9/9/9/9/9/9/9/9/9/9 w"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = PositionParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, "9/9/9/9/9/9/9/9/9/9 w");
        assert!(parsed.moves.is_empty());
    }

    #[test]
    fn parses_fen_with_moves() {
        let tokens = "fen 9/9/9/9/9/9/9/9/9/9 b moves a0a1"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = PositionParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, "9/9/9/9/9/9/9/9/9/9 b");
        assert_eq!(parsed.moves, &[Move::try_from("a0a1").unwrap()]);
    }

    #[test]
    fn encodes_move_bytes_consistently() {
        let mv = Move::try_from("b2c3").unwrap();
        // from: file 'b' = 1, rank '2' = 2
        // to:   file 'c' = 2, rank '3' = 3
        assert_eq!(mv.src_file(), 1);
        assert_eq!(mv.src_rank(), 2);
        assert_eq!(mv.dst_file(), 2);
        assert_eq!(mv.dst_rank(), 3);
    }

    #[test]
    fn uci_id_format() {
        let id = UciId {
            name: "Lingine".into(),
            author: "tuasananh".into(),
        };
        assert_eq!(id.to_string(), "id name Lingine\nid author tuasananh");
    }

    #[test]
    fn option_check_format() {
        let opt = UciOption::Check {
            name: "Ponder".into(),
            default: false,
        };
        assert_eq!(
            opt.to_string(),
            "option name Ponder type check default false"
        );
    }

    #[test]
    fn option_spin_format() {
        let opt = UciOption::Spin {
            name: "Hash".into(),
            default: 16,
            min: 1,
            max: 1024,
        };
        assert_eq!(
            opt.to_string(),
            "option name Hash type spin default 16 min 1 max 1024"
        );
    }

    #[test]
    fn option_combo_format() {
        let opt = UciOption::Combo {
            name: "Style".into(),
            default: "Normal".into(),
            vars: vec!["Solid".into(), "Normal".into(), "Risky".into()],
        };
        assert_eq!(
            opt.to_string(),
            "option name Style type combo default Normal var Solid var Normal var Risky"
        );
    }

    #[test]
    fn option_button_format() {
        let opt = UciOption::Button {
            name: "Clear Hash".into(),
        };
        assert_eq!(opt.to_string(), "option name Clear Hash type button");
    }

    #[test]
    fn option_str_format_empty() {
        let opt = UciOption::Str {
            name: "NalimovPath".into(),
            default: String::new(),
        };
        assert_eq!(
            opt.to_string(),
            "option name NalimovPath type string default <empty>"
        );
    }

    #[test]
    fn option_str_format_nonempty() {
        let opt = UciOption::Str {
            name: "NalimovPath".into(),
            default: "c:\\tb".into(),
        };
        assert_eq!(
            opt.to_string(),
            "option name NalimovPath type string default c:\\tb"
        );
    }

    #[test]
    fn score_cp_format() {
        let s = UciScoreBound {
            score: UciScore::Centipawns(214),
            bound: None,
        };
        assert_eq!(s.to_string(), "score cp 214");
    }

    #[test]
    fn score_mate_lowerbound_format() {
        let s = UciScoreBound {
            score: UciScore::Mate(3),
            bound: Some(Bound::Lower),
        };
        assert_eq!(s.to_string(), "score mate 3 lowerbound");
    }

    #[test]
    fn score_mated_format() {
        let s = UciScoreBound {
            score: UciScore::Mate(-2),
            bound: None,
        };
        assert_eq!(s.to_string(), "score mate -2");
    }

    #[test]
    fn uci_info_full_format() {
        let info = UciInfo {
            depth: Some(12),
            seldepth: Some(14),
            time: Some(Duration::from_millis(1242)),
            nodes: Some(123456),
            nps: Some(100000),
            score: Some(UciScoreBound {
                score: UciScore::Centipawns(214),
                bound: None,
            }),
            pv: Some(vec!["e2e4".into(), "e7e5".into(), "g1f3".into()]),
            ..UciInfo::new()
        };
        assert_eq!(
            info.to_string(),
            "info depth 12 seldepth 14 time 1242 nodes 123456 score cp 214 pv e2e4 e7e5 g1f3 nps 100000"
        );
    }

    #[test]
    fn uci_info_string_field_last() {
        let info = UciInfo {
            depth: Some(1),
            string: Some("hello world".into()),
            ..UciInfo::new()
        };
        let s = info.to_string();
        // string must appear last per spec
        assert!(s.ends_with("string hello world"));
    }

    #[test]
    fn bestmove_without_ponder() {
        let bm = BestMove {
            mv: "g1f3".into(),
            ponder: None,
        };
        assert_eq!(bm.to_string(), "bestmove g1f3");
    }

    #[test]
    fn bestmove_with_ponder() {
        let bm = BestMove {
            mv: "g1f3".into(),
            ponder: Some("d8f6".into()),
        };
        assert_eq!(bm.to_string(), "bestmove g1f3 ponder d8f6");
    }

    #[test]
    fn bestmove_null_move() {
        let bm = BestMove {
            mv: "0000".into(),
            ponder: None,
        };
        assert_eq!(bm.to_string(), "bestmove 0000");
    }
}
