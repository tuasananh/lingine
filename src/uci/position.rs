use std::slice::Iter;

pub struct UCIMove(u32);

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

pub struct UCIPosition {
    fen: String,
    moves: Vec<UCIMove>
}

const START_FEN: &str = ""; 

impl From<&mut Iter<'_, &str>> for UCIPosition {
    fn from(value: &mut Iter<'_, &str>) -> Self {
        let next_token = *value.next().expect("Expect 'fen' or 'startpos', but got nothing");

        assert!(next_token == "fen" || next_token == "startpos", "Expect 'fen' or 'startpos', but got {}", next_token);

        let fen = if (next_token == "fen") {
            let mut val = value.next().expect("Expect fen string, got nothing").to_string();
            while let Some(tok) = value.next() && *tok != "moves" {
                val += " ";
                val += *tok;
            }
            val
        } else {
            // consumes next token, which should be moves
            if let Some(tok) = value.next() && *tok != "moves" {
                // this should not happen 
                panic!("Expect 'moves', got {}", tok);
            }
            START_FEN.to_string()
        };

        let moves: Vec<UCIMove> = value.map(|tok| UCIMove::from(*tok)).collect();

        Self {
            fen, 
            moves
        }
    }
}