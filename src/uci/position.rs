use std::slice::Iter;

use anyhow::{Error, Result, ensure};

use crate::uci::r#move::UciMove;

/// The board position sent by the GUI via the `position` command.
///
/// Contains a FEN string describing the starting position and an ordered list
/// of moves applied on top of it. When the GUI sends `position startpos`, the
/// FEN is set to [`START_FEN`].
///
/// # Limitations
/// - `fen` is stored as a raw [`String`] and is **not validated** here.
///   FEN syntax errors (wrong piece counts, illegal side-to-move, etc.) will
///   only be caught when the real engine constructs its internal `Board` from
///   the FEN string inside `Engine::position()`.
/// - Each `UciMove` in `moves` is validated for correct notation (4 chars,
///   valid file/rank), but **not for legality**. The engine's `position()`
///   implementation must apply the moves one by one and return `Err` if any
///   is illegal.
#[derive(Clone, Debug)]
pub struct UciPosition {
    /// The starting position in Forsyth-Edwards Notation.
    pub fen: String,
    /// Moves applied after the FEN position, in order.
    pub moves: Vec<UciMove>,
}

/// The starting FEN for a standard Xiangqi game.
///
/// Used when the GUI sends `position startpos`.
const START_FEN: &str = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w";

impl TryFrom<&mut Iter<'_, &str>> for UciPosition {
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
            .map(|tok| UciMove::try_from(*tok))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { fen, moves })
    }
}

#[cfg(test)]
mod tests {
    use super::{UciMove, UciPosition};

    #[test]
    fn parses_startpos_without_moves() {
        let tokens = "startpos".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UciPosition::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, super::START_FEN);
        assert!(parsed.moves.is_empty());
    }

    #[test]
    fn parses_startpos_with_moves() {
        let tokens = "startpos moves a0a1 b0b1"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UciPosition::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, super::START_FEN);
        assert_eq!(parsed.moves.len(), 2);
    }

    #[test]
    fn parses_fen_without_moves() {
        let tokens = "fen 9/9/9/9/9/9/9/9/9/9 w"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UciPosition::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, "9/9/9/9/9/9/9/9/9/9 w");
        assert!(parsed.moves.is_empty());
    }

    #[test]
    fn parses_fen_with_moves() {
        let tokens = "fen 9/9/9/9/9/9/9/9/9/9 b moves a0a1"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UciPosition::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, "9/9/9/9/9/9/9/9/9/9 b");
        assert_eq!(parsed.moves, &[UciMove::try_from("a0a1").unwrap()]);
    }

    #[test]
    fn encodes_move_bytes_consistently() {
        let mv = UciMove::try_from("b2c3").unwrap();
        // from: file 'b' = 1, rank '2' = 2
        // to:   file 'c' = 2, rank '3' = 3
        assert_eq!(mv.src_file(), 1);
        assert_eq!(mv.src_rank(), 2);
        assert_eq!(mv.dst_file(), 2);
        assert_eq!(mv.dst_rank(), 3);
    }
}
