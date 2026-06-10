use strum::EnumCount;
use thiserror::Error;

use crate::core::{
    Score,
    bitboard::Bitboard,
    types::{File, Move, Piece, PieceType, Rank, Side, Square},
};

use zobrist_table::*;
mod attacks;
mod do_move;
mod evaluate;
mod helpers;
mod rule_judge;
mod zobrist_table;

/// Represents search state parameters that must be saved on a stack,
/// allowing incremental undo_move restorations.
#[derive(Clone, Copy, Debug)]
pub struct StateInfo {
    /// The last move played to reach this state.
    pub last_move: Option<Move>,
    /// The piece captured during this move, or `None` if it was quiet.
    pub captured_piece: Option<Piece>,
    /// The prior Zobrist position hash value before the move occurred.
    pub zobrist: u64,
    /// Halfmove clock / 60-rule counter (increments on quiet moves, resets to 0
    /// on captures/pawn moves).
    pub sixtymove_clock: u16,
    /// Whether each color [Red, Black] was in check in this position state.
    pub in_check: [bool; Side::COUNT],
    /// Precalculated incremental middlegame score (from Red's perspective)
    pub mg_score: Score,
    /// Precalculated incremental endgame score (from Red's perspective)
    pub eg_score: Score,
    /// Precalculated incremental game phase
    pub phase: i32,
}

/// Encapsulates the complete game board representation, bitboards, turn
/// tracking, ply count, and incremental state histories for the engine.
#[derive(Clone)]
pub struct Position {
    /// Flat 90-square array mapping square index (0 to 89) to the Piece
    /// occupying it.
    board: [Option<Piece>; Square::COUNT],
    /// Precomputed bitboards showing piece placements grouped by `PieceType`.
    bitboard_by_type: [Bitboard; PieceType::COUNT],
    /// Precomputed bitboards showing piece placements grouped by `Color`.
    bitboard_by_color: [Bitboard; Side::COUNT],
    /// Active count of each piece category on the board.
    piece_count: [u8; Piece::COUNT],
    /// Current state of position
    state: StateInfo,
    /// Stack tracking previous move parameter histories for undoing moves.
    history: Vec<StateInfo>,
    /// Total moves played in the game so far (Red = 0, Black = 1, Red's
    /// next = 2, etc.).
    game_ply: u16,
    /// Palace coordinates of both players' Generals (Kings) for faster check
    /// detection.
    king_squares: [Square; Side::COUNT],
}

#[derive(Error, Debug)]
#[error("Failed to set position: {msg}")]
pub struct PositionSetError {
    msg: String,
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
}

impl Position {
    /// Standard Xiangqi starting position in FEN notation.
    pub const START_FEN: &str = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w";

    /// Initializes the position to the standard starting position by default.
    pub fn new() -> Self {
        Self::from_fen(Self::START_FEN).expect("Failed to initialize starting position")
    }

    /// Get a position that is the start position
    pub fn startpos() -> Self {
        Self::new()
    }

    /// Get the Zobrish Hash of the current position
    #[inline]
    pub fn zobrist_hash(&self) -> u64 {
        self.state.zobrist
    }

    /// Get the current side to make a move
    #[inline]
    pub fn side_to_move(&self) -> Side {
        Side::from_repr(self.game_ply as u8 & 1).unwrap()
    }

    /// Get the piece currently at [`square`]
    #[inline]
    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        self.board[square as usize]
    }

    /// Get the number of [`piece`] currently in the board
    #[inline]
    pub fn piece_count(&self, piece: Piece) -> u8 {
        self.piece_count[piece as usize]
    }

    /// Get the bitboard of the [`piece_type`], which represents the pieces of
    /// that type currently on the board
    #[inline]
    pub fn bitboard_by_type(&self, piece_type: PieceType) -> Bitboard {
        self.bitboard_by_type[piece_type as usize]
    }

    /// Get the bitboard of the side [`color`], which represents the pieces
    /// owned by [`color`] currently on the board
    #[inline]
    pub fn bitboard_by_color(&self, color: Side) -> Bitboard {
        self.bitboard_by_color[color as usize]
    }

    #[inline]
    pub fn bitboard_of(&self, color: Side, piece_type: PieceType) -> Bitboard {
        self.bitboard_by_color(color) & self.bitboard_by_type(piece_type)
    }

    #[inline]
    pub fn bitboard_occupied(&self) -> Bitboard {
        self.bitboard_by_color(Side::Red) | self.bitboard_by_color(Side::Black)
    }

    /// Checks whether or not [`square`] is empty (have no piece on it)
    #[inline]
    pub fn is_empty(&self, square: Square) -> bool {
        self.board[square as usize].is_none()
    }

    /// Get the square where the king of side [`color`] is currently at
    #[inline]
    pub fn king_square(&self, color: Side) -> Square {
        self.king_squares[color as usize]
    }

    /// Checks if the given player's King is currently in check.
    #[inline]
    pub fn is_in_check(&self, color: Side) -> bool {
        self.is_square_attacked(self.king_square(color), color.opposite())
    }

    /// Finds the last piece that was captured, or None if last move is not a
    /// capture move
    #[inline]
    pub fn last_captured_piece(&self) -> Option<Piece> {
        self.history
            .last()
            .map(|s| s.captured_piece)
            .unwrap_or(None)
    }

    /// Parses and initializes the position state from a standard FEN string.
    pub fn from_fen(fen: &str) -> Result<Self, PositionSetError> {
        let mut pos = Position {
            board: [None; Square::COUNT],
            bitboard_by_type: [Bitboard::new(); PieceType::COUNT],
            bitboard_by_color: [Bitboard::new(); Side::COUNT],
            piece_count: [0; Piece::COUNT],
            state: StateInfo {
                last_move: None,
                captured_piece: None,
                zobrist: 0,
                sixtymove_clock: 0,
                in_check: [false; Side::COUNT],
                mg_score: 0,
                eg_score: 0,
                phase: 0,
            },
            history: Vec::new(),
            game_ply: 0,
            king_squares: [Square::E0, Square::E9],
        };

        let tokens: Vec<&str> = fen.split_whitespace().collect();
        if tokens.is_empty() {
            return Err(PositionSetError {
                msg: "Empty FEN".to_string(),
            });
        }

        // 1. Parse piece placement ranks (10 ranks, separated by '/')
        let ranks: Vec<&str> = tokens[0].split('/').collect();
        if ranks.len() != 10 {
            return Err(PositionSetError {
                msg: format!("Expected 10 ranks, got {}", ranks.len()),
            });
        }

        for rank_idx in 0..10 {
            let rank = Rank::from_repr(9 - rank_idx).unwrap();
            let rank_str = ranks[rank_idx as usize];
            let mut file_idx = 0u8;

            for c in rank_str.chars() {
                if c.is_ascii_digit() {
                    let empty_squares = c.to_digit(10).unwrap() as u8;
                    file_idx += empty_squares;
                } else {
                    if file_idx >= 9 {
                        return Err(PositionSetError {
                            msg: "File index out of bounds".to_string(),
                        });
                    }
                    let file = File::from_repr(file_idx).unwrap();
                    let square = Square::from_file_rank(file, rank);
                    if let Some(piece) = Self::piece_from_char(c) {
                        pos.put_piece(square, Some(piece));
                    } else {
                        return Err(PositionSetError {
                            msg: format!("Unknown piece character: {}", c),
                        });
                    }
                    file_idx += 1;
                }
            }
            if file_idx != 9 {
                return Err(PositionSetError {
                    msg: format!("Invalid rank width: expected 9 files, got {}", file_idx),
                });
            }
        }

        // 2. Parse side to move ('w' or 'b')
        let side_to_move = if tokens.len() > 1 {
            match tokens[1] {
                "w" => Side::Red,
                "b" => Side::Black,
                _ => {
                    return Err(PositionSetError {
                        msg: format!("Invalid side to move: {}", tokens[1]),
                    });
                }
            }
        } else {
            Side::Red
        };

        if side_to_move == Side::Black {
            pos.state.zobrist ^= ZOBRIST.side;
        }

        let mut rule60 = 0;
        if tokens.len() > 4
            && let Ok(r60) = tokens[4].parse::<u16>()
        {
            rule60 = r60;
        }

        let mut fullmove = 1;
        if tokens.len() > 5
            && let Ok(fm) = tokens[5].parse::<u16>()
        {
            fullmove = fm;
        }
        pos.game_ply = (fullmove.saturating_sub(1) * 2) + (side_to_move as u16);

        pos.state.sixtymove_clock = rule60;
        pos.state.in_check = [pos.is_in_check(Side::Red), pos.is_in_check(Side::Black)];
        (pos.state.mg_score, pos.state.eg_score) = pos.compute_tapered_evaluation_scores();
        pos.state.phase = pos.calculate_board_phase();

        Ok(pos)
    }

    /// Prints a human-readable ASCII representation of the board state.
    pub fn print_board(&self) {
        println!("  +---------------------------+");
        for rank_idx in (0..10).rev() {
            print!("{} |", rank_idx);
            for file_idx in 0..9 {
                let file = File::from_repr(file_idx).unwrap();
                let rank = Rank::from_repr(rank_idx).unwrap();
                let square = Square::from_file_rank(file, rank);
                let piece_char = match self.board[square as usize] {
                    Some(Piece::RedRook) => 'R',
                    Some(Piece::RedKnight) => 'H',
                    Some(Piece::RedBishop) => 'E',
                    Some(Piece::RedAdvisor) => 'A',
                    Some(Piece::RedKing) => 'K',
                    Some(Piece::RedCannon) => 'C',
                    Some(Piece::RedPawn) => 'P',
                    Some(Piece::BlackRook) => 'r',
                    Some(Piece::BlackKnight) => 'h',
                    Some(Piece::BlackBishop) => 'e',
                    Some(Piece::BlackAdvisor) => 'a',
                    Some(Piece::BlackKing) => 'k',
                    Some(Piece::BlackCannon) => 'c',
                    Some(Piece::BlackPawn) => 'p',
                    None => '.',
                };
                print!(" {} ", piece_char);
            }
            println!("|");
        }
        println!("  +---------------------------+");
        println!("    a  b  c  d  e  f  g  h  i");
        println!("Side to move: {:?}", self.side_to_move());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        score,
        types::{Move, Square},
    };

    #[test]
    fn test_knight_leg_pin() {
        // Red King at E0, Red Advisor at F1, Black Knight at F2 (aligned to jump
        // onto E0)
        let pos = Position::from_fen("4k4/9/9/9/9/9/9/5h3/5A3/4K4 w - - 0 1").unwrap();
        // Since F1 blocks the Knight's jump, the Advisor is fully pinned and cannot
        // move away
        assert!(!pos.legal(Move::new(Square::F1, Square::E2)));
        assert!(!pos.legal(Move::new(Square::F1, Square::G2)));
        assert!(!pos.legal(Move::new(Square::F1, Square::E0)));
        assert!(!pos.legal(Move::new(Square::F1, Square::G0)));
    }

    #[test]
    fn test_rule_judge_insufficient_material() {
        // 1. Bare Kings
        let mut pos = Position::from_fen("4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1").unwrap();
        assert_eq!(pos.rule_judge(0), Some(score::DRAW));

        // 2. Kings + Bishops & Advisors (no attacking pieces)
        let mut pos = Position::from_fen("2b1kab2/9/9/9/9/9/9/9/9/2B1KAB2 w - - 0 1").unwrap();
        assert_eq!(pos.rule_judge(0), Some(score::DRAW));

        // 3. Kings + 1 Cannon, no Advisors/Bishops
        let mut pos = Position::from_fen("4k4/9/9/9/9/9/9/9/3C5/4K4 w - - 0 1").unwrap();
        assert_eq!(pos.rule_judge(0), Some(score::DRAW));
    }

    #[test]
    fn test_rule_judge_60_move_rule() {
        let mut pos = Position::from_fen("4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1").unwrap();

        // Play 120 plies of quiet King moves back and forth (60 full moves)
        let w_move1 = Move::new(Square::E0, Square::D0);
        let w_move2 = Move::new(Square::D0, Square::E0);
        let b_move1 = Move::new(Square::E9, Square::D9);
        let b_move2 = Move::new(Square::D9, Square::E9);

        for _ in 0..30 {
            pos.do_move(w_move1);
            pos.do_move(b_move1);
            pos.do_move(w_move2);
            pos.do_move(b_move2);
        }

        // rule60 counter should be exactly 120
        assert_eq!(pos.state.sixtymove_clock, 120);
        assert_eq!(pos.rule_judge(0), Some(score::DRAW));
    }

    #[test]
    fn test_print_board() {
        let pos = Position::new();
        pos.print_board();
    }
}
