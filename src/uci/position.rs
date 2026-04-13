use std::slice::Iter;

#[derive(Clone, Debug)]
pub struct UCIMove(u32);

impl UCIMove {
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl PartialEq for UCIMove {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for UCIMove {}

impl From<&str> for UCIMove {
    fn from(value: &str) -> Self {
        assert!(value.len() == 4, "Expect move to be of 4 characters");
        let value = value.as_bytes();
        let r1 = value[0] - 'a' as u8;
        let f1 = value[1] - '0' as u8;
        let r2 = value[2] - 'a' as u8;
        let f2 = value[3] - '0' as u8;
        Self((r1 as u32) | ((f1 as u32) << 8) | ((r2 as u32) << 16) | ((f2 as u32) << 24))
    }
}

#[derive(Clone, Debug)]
pub struct UCIPosition {
    fen: String,
    moves: Vec<UCIMove>,
}

const START_FEN: &str = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w";

impl From<&mut Iter<'_, &str>> for UCIPosition {
    fn from(value: &mut Iter<'_, &str>) -> Self {
        let next_token = *value
            .next()
            .expect("Expect 'fen' or 'startpos', but got nothing");

        assert!(
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
            // consumes next token, which should be moves
            if let Some(tok) = value.next()
                && *tok != "moves"
            {
                // this should not happen
                panic!("Expect 'moves', got {}", tok);
            }
            START_FEN.to_string()
        };

        let moves: Vec<UCIMove> = value.map(|tok| UCIMove::from(*tok)).collect();

        Self { fen, moves }
    }
}

#[cfg(test)]
mod tests {
    use super::{UCIMove, UCIPosition};

    #[test]
    fn parses_startpos_without_moves() {
        let tokens = "startpos".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UCIPosition::from(&mut iter);

        assert_eq!(parsed.fen, super::START_FEN);
        assert!(parsed.moves.is_empty());
    }

    #[test]
    fn parses_startpos_with_moves() {
        let tokens = "startpos moves a0a1 b0b1"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UCIPosition::from(&mut iter);

        assert_eq!(parsed.fen, super::START_FEN);
        assert_eq!(parsed.moves.len(), 2);
    }

    #[test]
    fn parses_fen_without_moves() {
        let tokens = "fen 9/9/9/9/9/9/9/9/9/9 w"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UCIPosition::from(&mut iter);

        assert_eq!(parsed.fen, "9/9/9/9/9/9/9/9/9/9 w");
        assert!(parsed.moves.is_empty());
    }

    #[test]
    fn parses_fen_with_moves() {
        let tokens = "fen 9/9/9/9/9/9/9/9/9/9 b moves a0a1"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UCIPosition::from(&mut iter);

        assert_eq!(parsed.fen, "9/9/9/9/9/9/9/9/9/9 b");
        assert_eq!(parsed.moves, &[UCIMove::from("a0a1")]);
    }

    #[test]
    fn encodes_move_bytes_consistently() {
        let mv = UCIMove::from("b2c3");
        let expected = (1_u32) | (2_u32 << 8) | (2_u32 << 16) | (3_u32 << 24);
        assert_eq!(mv.as_u32(), expected);
    }
}
