use std::slice::Iter;

use anyhow::{Error, Result, ensure};

use crate::uci::r#move::Move;

#[derive(Clone, Debug)]
pub struct Position {
    pub fen: String,
    pub moves: Vec<Move>,
}

const START_FEN: &str = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w";

impl TryFrom<&mut Iter<'_, &str>> for Position {
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
            let mut val = value
                .next()
                .expect("Expect fen string, got nothing")
                .to_string();
            while let Some(tok) = value.next()
                && *tok != "moves"
            {
                val += " ";
                val += *tok;
            }
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

#[cfg(test)]
mod tests {
    use super::{Move, Position};

    #[test]
    fn parses_startpos_without_moves() {
        let tokens = "startpos".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = Position::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, super::START_FEN);
        assert!(parsed.moves.is_empty());
    }

    #[test]
    fn parses_startpos_with_moves() {
        let tokens = "startpos moves a0a1 b0b1"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = Position::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, super::START_FEN);
        assert_eq!(parsed.moves.len(), 2);
    }

    #[test]
    fn parses_fen_without_moves() {
        let tokens = "fen 9/9/9/9/9/9/9/9/9/9 w"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = Position::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, "9/9/9/9/9/9/9/9/9/9 w");
        assert!(parsed.moves.is_empty());
    }

    #[test]
    fn parses_fen_with_moves() {
        let tokens = "fen 9/9/9/9/9/9/9/9/9/9 b moves a0a1"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = Position::try_from(&mut iter).unwrap();

        assert_eq!(parsed.fen, "9/9/9/9/9/9/9/9/9/9 b");
        assert_eq!(parsed.moves, &[Move::try_from("a0a1").unwrap()]);
    }

    #[test]
    fn encodes_move_bytes_consistently() {
        let mv = Move::try_from("b2c3").unwrap();
        let expected = (1_u32) | (2_u32 << 8) | (2_u32 << 16) | (3_u32 << 24);
        assert_eq!(mv.as_u32(), expected);
    }
}
