use anyhow::{Error, Result, anyhow, bail, ensure};

use std::{num::NonZeroU32, time::Duration};

use crate::core::Position;

/// A parsed UCI command received from the GUI on stdin.
///
/// [`EngineCommand::parse`] converts a raw input line into a typed command.
/// Blank lines and unrecognised commands produce `Ok(None)` (per spec: silently
/// ignored). Structurally invalid known commands (e.g. a malformed FEN string
/// in `position`) produce `Err`.
pub enum EngineCommand {
    /// `uci` — GUI asks for engine identity and options.
    Uci,
    /// `debug on|off` — enable or disable verbose `info string` output.
    ///
    /// Only produced when the keyword is followed by exactly `"on"` or `"off"`;
    /// a missing or unrecognised value silently yields `Ok(None)`.
    Debug(bool),
    /// `isready` — GUI asks whether the engine is ready; handler replies
    /// `readyok`.
    IsReady,
    /// `setoption name <name> [value <value>]` — change a configuration option.
    SetOption(SetOptionParameters),
    /// `register later|name <n> code <c>` — engine registration
    /// (copy-protection).
    Register(RegisterParameters),
    /// `ucinewgame` — the next position will be from a different game; reset
    /// state.
    NewGame,
    /// `position startpos|fen <fen> [moves <mv>…]` — set the current board
    /// position.
    Position(PositionParameters),
    /// `go <params>` — start searching the current position.
    ///
    /// The `stop` field of [`GoParameters`] is left at its `Default` value
    /// here (a fresh, unshared `Arc<AtomicBool>`). The engine actor injects
    /// the real shared flag just before calling `engine.go()` so that Thread A
    /// can interrupt the search via `stop_flag.store(true)`.
    Go(GoParameters),
    /// `stop` — interrupt the running search as soon as possible.
    ///
    /// Thread A sets the shared `stop_flag` atomically *before* enqueuing this
    /// command, so the engine's search loop can observe the signal without
    /// waiting for the command queue to drain.
    Stop,
    /// `ponderhit` — the opponent played the ponder move; switch to normal
    /// search.
    ///
    /// # Limitation
    /// Thread B is blocked inside `engine.go()` while searching, so this
    /// command queues but is not processed until the search returns. There is
    /// no plumbing to handle it mid-search yet.
    PonderHit,
    /// `quit` — terminate the engine process.
    Quit,
}

impl EngineCommand {
    /// Parse one line of UCI input into a command.
    ///
    /// Returns [`None`] for blank lines and unknown commands (silently
    /// ignored per the UCI spec), as well as structurally invalid
    /// known commands.
    /// Returns [`Some(cmd)`] for valid, fully-parsed commands.
    pub fn parse(line: &str) -> Result<Self> {
        let tokens: Vec<&str> = line.split_whitespace().collect();

        match tokens.as_slice() {
            ["uci"] => Ok(Self::Uci),
            ["debug", "on"] => Ok(Self::Debug(true)),
            ["debug", "off"] => Ok(Self::Debug(false)),
            ["isready"] => Ok(Self::IsReady),
            ["setoption", tokens @ ..] => {
                SetOptionParameters::try_from(tokens).map(Self::SetOption)
            }
            ["register", tokens @ ..] => RegisterParameters::try_from(tokens).map(Self::Register),
            ["ucinewgame"] => Ok(Self::NewGame),
            ["position", tokens @ ..] => PositionParameters::try_from(tokens).map(Self::Position),
            ["go", tokens @ ..] => GoParameters::try_from(tokens).map(Self::Go),
            ["stop"] => Ok(Self::Stop),
            ["ponderhit"] => Ok(Self::PonderHit),
            ["quit"] => Ok(Self::Quit),
            // Unknown command — UCI spec says ignore silently.
            _ => bail!("Unknown command"),
        }
    }
}

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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Time remaining for Red.
    pub wtime: Option<Duration>,
    /// Time remaining for Black.
    pub btime: Option<Duration>,
    /// Red's increment added after each move.
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
}

impl TryFrom<&[&str]> for GoParameters {
    type Error = Error;
    fn try_from(tokens: &[&str]) -> Result<Self> {
        let mut res = Self::default();
        let mut i = 0;

        while i < tokens.len() {
            let tok = tokens[i];
            i += 1;
            match tok {
                "searchmoves" => {
                    let mut searchmoves = Vec::new();
                    while i < tokens.len() {
                        let next_tok = tokens[i];
                        match next_tok {
                            "ponder" | "wtime" | "btime" | "winc" | "binc" | "movestogo"
                            | "depth" | "nodes" | "mate" | "movetime" | "infinite" | "perft" => {
                                break;
                            }
                            _ => {
                                i += 1;
                                searchmoves.push(next_tok.try_into()?);
                            }
                        }
                    }
                    res.searchmoves = Some(searchmoves);
                }
                "ponder" => {
                    res.ponder = true;
                }
                "wtime" => {
                    ensure!(i < tokens.len(), "missing value for 'wtime'");
                    let val = tokens[i];
                    i += 1;
                    let ms = val
                        .parse::<u64>()
                        .map_err(|_| anyhow!("invalid 'wtime' value: {val}"))?;
                    res.wtime = Some(Duration::from_millis(ms));
                }
                "btime" => {
                    ensure!(i < tokens.len(), "missing value for 'btime'");
                    let val = tokens[i];
                    i += 1;
                    let ms = val
                        .parse::<u64>()
                        .map_err(|_| anyhow!("invalid 'btime' value: {val}"))?;
                    res.btime = Some(Duration::from_millis(ms));
                }
                "winc" => {
                    ensure!(i < tokens.len(), "missing value for 'winc'");
                    let val = tokens[i];
                    i += 1;
                    let ms = val
                        .parse::<u64>()
                        .map_err(|_| anyhow!("invalid 'winc' value: {val}"))?;
                    res.winc = Some(Duration::from_millis(ms));
                }
                "binc" => {
                    ensure!(i < tokens.len(), "missing value for 'binc'");
                    let val = tokens[i];
                    i += 1;
                    let ms = val
                        .parse::<u64>()
                        .map_err(|_| anyhow!("invalid 'binc' value: {val}"))?;
                    res.binc = Some(Duration::from_millis(ms));
                }
                "movestogo" => {
                    ensure!(i < tokens.len(), "missing value for 'movestogo'");
                    let val = tokens[i];
                    i += 1;
                    let n = val
                        .parse::<u32>()
                        .map_err(|_| anyhow!("invalid 'movestogo' value: {val}"))?;
                    res.movestogo =
                        Some(NonZeroU32::new(n).ok_or_else(|| anyhow!("'movestogo' must be > 0"))?);
                }
                "depth" => {
                    ensure!(i < tokens.len(), "missing value for 'depth'");
                    let val = tokens[i];
                    i += 1;
                    res.depth = Some(
                        val.parse::<u32>()
                            .map_err(|_| anyhow!("invalid 'depth' value: {val}"))?,
                    );
                }
                "nodes" => {
                    ensure!(i < tokens.len(), "missing value for 'nodes'");
                    let val = tokens[i];
                    i += 1;
                    res.nodes = Some(
                        val.parse::<u64>()
                            .map_err(|_| anyhow!("invalid 'nodes' value: {val}"))?,
                    );
                }
                "mate" => {
                    ensure!(i < tokens.len(), "missing value for 'mate'");
                    let val = tokens[i];
                    i += 1;
                    res.mate = Some(
                        val.parse::<u32>()
                            .map_err(|_| anyhow!("invalid 'mate' value: {val}"))?,
                    );
                }
                "movetime" => {
                    ensure!(i < tokens.len(), "missing value for 'movetime'");
                    let val = tokens[i];
                    i += 1;
                    let ms = val
                        .parse::<u64>()
                        .map_err(|_| anyhow!("invalid 'movetime' value: {val}"))?;
                    res.movetime = Some(Duration::from_millis(ms));
                }
                "infinite" => {
                    res.infinite = true;
                }
                "perft" => {
                    ensure!(i < tokens.len(), "missing value for 'perft'");
                    let val = tokens[i];
                    i += 1;
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

impl TryFrom<&[&str]> for SetOptionParameters {
    type Error = Error;

    fn try_from(tokens: &[&str]) -> Result<Self> {
        ensure!(
            !tokens.is_empty() && tokens[0] == "name",
            "Expected 'name' keyword"
        );

        let mut name_parts = Vec::new();
        let mut value_parts = Vec::new();
        let mut parsing_value = false;

        for &tok in &tokens[1..] {
            if tok == "value" {
                parsing_value = true;
                continue;
            }
            if parsing_value {
                value_parts.push(tok);
            } else {
                name_parts.push(tok);
            }
        }

        ensure!(!name_parts.is_empty(), "Option name cannot be empty");

        let name = name_parts.join(" ").to_lowercase();
        let value = if parsing_value {
            Some(value_parts.join(" "))
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

impl TryFrom<&[&str]> for RegisterParameters {
    type Error = Error;

    fn try_from(tokens: &[&str]) -> Result<Self> {
        if tokens.first() == Some(&"later") {
            return Ok(RegisterParameters::Later);
        }

        let mut name_tokens = Vec::new();
        let mut code = None;
        let mut parsing_name = false;
        let mut i = 0;

        while i < tokens.len() {
            let token = tokens[i];
            i += 1;
            match token {
                "name" => {
                    parsing_name = true;
                }
                "code" => {
                    parsing_name = false;
                    if i < tokens.len() {
                        code = Some(tokens[i].to_string());
                        i += 1;
                    }
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
            fen: Position::START_FEN.to_string(),
            moves: Vec::new(),
        }
    }
}

impl TryFrom<&[&str]> for PositionParameters {
    type Error = Error;
    fn try_from(tokens: &[&str]) -> Result<Self> {
        ensure!(
            !tokens.is_empty(),
            "Expect 'fen' or 'startpos', but got nothing"
        );
        let next_token = tokens[0];
        ensure!(
            next_token == "fen" || next_token == "startpos",
            "Expect 'fen' or 'startpos', but got {}",
            next_token
        );

        let mut fen = String::new();
        let mut moves = Vec::new();
        let mut i = 1;

        if next_token == "fen" {
            while i < tokens.len() {
                let tok = tokens[i];
                if tok == "moves" {
                    i += 1;
                    break;
                }
                if !fen.is_empty() {
                    fen += " ";
                }
                fen += tok;
                i += 1;
            }
            ensure!(!fen.is_empty(), "Expect fen string, got nothing");
        } else {
            // startpos
            while i < tokens.len() {
                if tokens[i] == "moves" {
                    i += 1;
                    break;
                }
                i += 1;
            }
            fen = Position::START_FEN.to_string();
        }

        while i < tokens.len() {
            moves.push(Move::try_from(tokens[i])?);
            i += 1;
        }

        Ok(Self { fen, moves })
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use crate::core::Position;

    use super::*;

    #[test]
    fn test_parse_simple_commands() {
        assert!(EngineCommand::parse("").is_err());
        assert!(EngineCommand::parse("   ").is_err());
        assert!(EngineCommand::parse("unknown_command").is_err());

        assert!(matches!(
            EngineCommand::parse("uci"),
            Ok(EngineCommand::Uci)
        ));
        assert!(matches!(
            EngineCommand::parse("isready"),
            Ok(EngineCommand::IsReady)
        ));
        assert!(matches!(
            EngineCommand::parse("ucinewgame"),
            Ok(EngineCommand::NewGame)
        ));
        assert!(matches!(
            EngineCommand::parse("stop"),
            Ok(EngineCommand::Stop)
        ));
        assert!(matches!(
            EngineCommand::parse("ponderhit"),
            Ok(EngineCommand::PonderHit)
        ));
        assert!(matches!(
            EngineCommand::parse("quit"),
            Ok(EngineCommand::Quit)
        ));
    }

    #[test]
    fn test_parse_debug() {
        assert!(matches!(
            EngineCommand::parse("debug on"),
            Ok(EngineCommand::Debug(true))
        ));
        assert!(matches!(
            EngineCommand::parse("debug off"),
            Ok(EngineCommand::Debug(false))
        ));
        assert!(EngineCommand::parse("debug").is_err());
        assert!(EngineCommand::parse("debug foo").is_err());
    }

    #[test]
    fn test_parse_setoption() {
        let cmd = EngineCommand::parse("setoption name Hash value 16").unwrap();
        if let EngineCommand::SetOption(params) = cmd {
            assert_eq!(params.name, "hash");
            assert_eq!(params.value, Some("16".to_string()));
        } else {
            panic!("Expected SetOption");
        }

        // Invalid setoption format should fail
        assert!(EngineCommand::parse("setoption").is_err());
    }

    #[test]
    fn test_parse_register() {
        let cmd = EngineCommand::parse("register later").unwrap();
        assert!(matches!(
            cmd,
            EngineCommand::Register(RegisterParameters::Later)
        ));
    }

    #[test]
    fn test_parse_position() {
        let cmd = EngineCommand::parse("position startpos").unwrap();
        if let EngineCommand::Position(pos) = cmd {
            assert_eq!(pos.fen, Position::START_FEN);
            assert!(pos.moves.is_empty());
        } else {
            panic!("Expected Position");
        }
    }

    #[test]
    fn test_parse_go() {
        let cmd = EngineCommand::parse("go depth 5").unwrap();
        if let EngineCommand::Go(params) = cmd {
            assert_eq!(params.depth, Some(5));
        } else {
            panic!("Expected Go");
        }
    }

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

        let parsed = GoParameters::try_from(tokens.as_slice()).unwrap();

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

        let parsed = GoParameters::try_from(tokens.as_slice()).unwrap();

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

        let parsed = GoParameters::try_from(tokens.as_slice()).unwrap();

        assert_eq!(parsed.depth, Some(3));
    }

    #[test]
    fn returns_error_on_missing_wtime_value() {
        let tokens = "wtime".split_whitespace().collect::<Vec<&str>>();
        assert!(GoParameters::try_from(tokens.as_slice()).is_err());
    }

    #[test]
    fn returns_error_on_invalid_depth_value() {
        let tokens = "depth abc".split_whitespace().collect::<Vec<&str>>();
        assert!(GoParameters::try_from(tokens.as_slice()).is_err());
    }

    #[test]
    fn returns_error_on_zero_movestogo() {
        let tokens = "movestogo 0".split_whitespace().collect::<Vec<&str>>();
        assert!(GoParameters::try_from(tokens.as_slice()).is_err());
    }

    #[test]
    fn parses_single_word_name_and_value() {
        let tokens = "name Hash value 128"
            .split_whitespace()
            .collect::<Vec<&str>>();

        let parsed = SetOptionParameters::try_from(tokens.as_slice()).unwrap();

        assert_eq!(parsed.name, "hash");
        assert_eq!(parsed.value.as_deref(), Some("128"));
    }

    #[test]
    fn parses_multi_word_name_and_value() {
        let tokens = "name UCI Engine About value LiNgine test build"
            .split_whitespace()
            .collect::<Vec<&str>>();

        let parsed = SetOptionParameters::try_from(tokens.as_slice()).unwrap();

        assert_eq!(parsed.name, "uci engine about");
        assert_eq!(parsed.value.as_deref(), Some("LiNgine test build"));
    }

    #[test]
    fn parses_option_without_value() {
        let tokens = "name Clear Hash".split_whitespace().collect::<Vec<&str>>();

        let parsed = SetOptionParameters::try_from(tokens.as_slice()).unwrap();

        assert_eq!(parsed.name, "clear hash");
        assert_eq!(parsed.value, None);
    }

    #[test]
    fn parses_later() {
        let tokens = "later".split_whitespace().collect::<Vec<&str>>();

        let parsed = RegisterParameters::try_from(tokens.as_slice()).unwrap();

        assert!(matches!(parsed, RegisterParameters::Later));
    }

    #[test]
    fn parses_later_and_ignores_remaining_tokens() {
        let tokens = "later name John".split_whitespace().collect::<Vec<&str>>();

        let parsed = RegisterParameters::try_from(tokens.as_slice()).unwrap();

        assert!(matches!(parsed, RegisterParameters::Later));
    }

    #[test]
    fn parses_empty_input() {
        let tokens = "".split_whitespace().collect::<Vec<&str>>();

        let parsed = RegisterParameters::try_from(tokens.as_slice()).unwrap();

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

        let parsed = RegisterParameters::try_from(tokens.as_slice()).unwrap();

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

        let parsed = RegisterParameters::try_from(tokens.as_slice()).unwrap();

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

        let parsed = RegisterParameters::try_from(tokens.as_slice()).unwrap();

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

        let parsed = RegisterParameters::try_from(tokens.as_slice()).unwrap();

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

        let parsed = RegisterParameters::try_from(tokens.as_slice()).unwrap();

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

        let parsed = RegisterParameters::try_from(tokens.as_slice()).unwrap();

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

        let parsed = RegisterParameters::try_from(tokens.as_slice()).unwrap();

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

        let parsed = PositionParameters::try_from(tokens.as_slice()).unwrap();

        assert_eq!(parsed.fen, Position::START_FEN);
        assert!(parsed.moves.is_empty());
    }

    #[test]
    fn parses_startpos_with_moves() {
        let tokens = "startpos moves a0a1 b0b1"
            .split_whitespace()
            .collect::<Vec<&str>>();

        let parsed = PositionParameters::try_from(tokens.as_slice()).unwrap();

        assert_eq!(parsed.fen, Position::START_FEN);
        assert_eq!(parsed.moves.len(), 2);
    }

    #[test]
    fn parses_fen_without_moves() {
        let tokens = "fen 9/9/9/9/9/9/9/9/9/9 w"
            .split_whitespace()
            .collect::<Vec<&str>>();

        let parsed = PositionParameters::try_from(tokens.as_slice()).unwrap();

        assert_eq!(parsed.fen, "9/9/9/9/9/9/9/9/9/9 w");
        assert!(parsed.moves.is_empty());
    }

    #[test]
    fn parses_fen_with_moves() {
        let tokens = "fen 9/9/9/9/9/9/9/9/9/9 b moves a0a1"
            .split_whitespace()
            .collect::<Vec<&str>>();

        let parsed = PositionParameters::try_from(tokens.as_slice()).unwrap();

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
}
