use std::slice::Iter;

pub struct UCIMove(u32);
pub struct UCIPosition {
    fen: String,
    moves: Vec<UCIMove>
}

impl From<&mut Iter<'_, &str>> for UCIPosition {
    fn from(_value: &mut Iter<'_, &str>) -> Self {
        todo!("Actually parse for position")
    }
}