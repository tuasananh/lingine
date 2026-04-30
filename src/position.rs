use anyhow::Result;
use strum::EnumCount;
use thiserror::Error;

use crate::{
    bitboard::Bitboard,
    types::{BloomFilter, Color, File, Move, Piece, PieceType, Rank, Square},
};

struct StateInfo {}

pub struct Position {
    board: [Piece; Square::COUNT],
    bitboard_by_type: [Bitboard; PieceType::COUNT],
    bitboard_by_color: [Bitboard; Color::COUNT],

    piece_count: [u8; Piece::COUNT],
    history: Vec<StateInfo>,
    game_ply: u16,
    side_to_move: Color,

    filter: BloomFilter,

    id_board: [u8; Square::COUNT],
}

#[derive(Error, Debug)]
#[error("Failed to set position: {msg}")]
pub struct PositionSetError {
    msg: String,
}

impl Position {
    pub fn new() -> Self {
        todo!();
    }

    pub fn set(&mut self, fen: &str) -> Result<(), PositionSetError> {
        let mut num_pieces = 0;
        let file = File::FA;
        let rank = Rank::R9;
        let mut iter = fen.bytes();
        loop {
            if false {
                break;
            }
            let token = iter.next();
            if token.is_none() {
                return Err(PositionSetError {
                    msg: "Unexpected end of FEN string".to_string(),
                });
            }
        }
        todo!();
    }

    pub fn do_move(&mut self, m: Move) {
        todo!();
    }

    pub fn undo_move(&mut self, m: Move) {
        todo!();
    }

    pub fn is_empty(&self, square: Square) -> bool {
        todo!()
    }

    pub fn gives_check(&self, m: Move) -> bool {
        todo!();
    }
}
