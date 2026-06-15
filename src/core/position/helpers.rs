use crate::core::{Piece, Position};

impl Position {
    /// Maps standard algebraic piece FEN notation characters to their Piece
    /// enums.
    #[inline]
    pub(super) fn piece_from_char(c: char) -> Option<Piece> {
        match c {
            'R' => Some(Piece::RedRook),
            'H' | 'N' => Some(Piece::RedKnight),
            'E' | 'B' => Some(Piece::RedBishop),
            'A' => Some(Piece::RedAdvisor),
            'K' => Some(Piece::RedKing),
            'C' => Some(Piece::RedCannon),
            'P' => Some(Piece::RedPawn),
            'r' => Some(Piece::BlackRook),
            'h' | 'n' => Some(Piece::BlackKnight),
            'e' | 'b' => Some(Piece::BlackBishop),
            'a' => Some(Piece::BlackAdvisor),
            'k' => Some(Piece::BlackKing),
            'c' => Some(Piece::BlackCannon),
            'p' => Some(Piece::BlackPawn),
            _ => None,
        }
    }
}
