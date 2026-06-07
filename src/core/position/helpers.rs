use crate::core::{Piece, Position};

impl Position {
    /// Maps standard algebraic piece FEN notation characters to their Piece
    /// enums.
    #[inline]
    pub fn piece_from_char(c: char) -> Option<Piece> {
        match c {
            'R' => Some(Piece::WhiteRook),
            'H' | 'N' => Some(Piece::WhiteKnight),
            'E' | 'B' => Some(Piece::WhiteBishop),
            'A' => Some(Piece::WhiteAdvisor),
            'K' => Some(Piece::WhiteKing),
            'C' => Some(Piece::WhiteCannon),
            'P' => Some(Piece::WhitePawn),
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
