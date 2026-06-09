use std::{fmt, time::Duration};

use crate::core::Score;

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
    Centipawns(Score),
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

#[cfg(test)]
mod tests {
    use super::*;

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
