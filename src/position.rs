use anyhow::Result;
use strum::EnumCount;
use thiserror::Error;
use std::sync::OnceLock;

use crate::{
    bitboard::Bitboard,
    types::{BloomFilter, Color, File, Move, Piece, PieceType, Rank, Square},
};

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state
    }
}

struct ZobristTable {
    pieces: [[u64; 90]; 16],
    side: u64,
}

static ZOBRIST: OnceLock<ZobristTable> = OnceLock::new();

impl ZobristTable {
    fn init() -> Self {
        let mut prng = Lcg::new(1070372);
        let mut pieces = [[0u64; 90]; 16];
        for p in 0..16 {
            for sq in 0..90 {
                pieces[p][sq] = prng.next();
            }
        }
        let side = prng.next();
        Self { pieces, side }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StateInfo {
    pub captured_piece: Piece,
    pub old_zobrist: u64,
    pub rule50: u16,
}

#[derive(Clone)]
pub struct Position {
    board: [Piece; Square::COUNT],
    bitboard_by_type: [Bitboard; PieceType::COUNT],
    bitboard_by_color: [Bitboard; Color::COUNT],

    piece_count: [u8; 16],
    history: Vec<StateInfo>,
    game_ply: u16,
    side_to_move: Color,

    filter: BloomFilter,

    id_board: [u8; Square::COUNT],
    pub zobrist_hash: u64,
}

#[derive(Error, Debug)]
#[error("Failed to set position: {msg}")]
pub struct PositionSetError {
    msg: String,
}

impl Position {
    pub fn new() -> Self {
        let mut pos = Self {
            board: [Piece::NoPiece; Square::COUNT],
            bitboard_by_type: [Bitboard::new(); PieceType::COUNT],
            bitboard_by_color: [Bitboard::new(); Color::COUNT],
            piece_count: [0; 16],
            history: Vec::new(),
            game_ply: 0,
            side_to_move: Color::White,
            filter: BloomFilter::new(),
            id_board: [0; Square::COUNT],
            zobrist_hash: 0,
        };
        // Setup initial empty history state
        pos.history.push(StateInfo {
            captured_piece: Piece::NoPiece,
            old_zobrist: 0,
            rule50: 0,
        });
        pos
    }

    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    pub fn piece_at(&self, square: Square) -> Piece {
        self.board[square as usize]
    }

    pub fn piece_count(&self, piece: Piece) -> u8 {
        self.piece_count[piece as usize]
    }

    pub fn bitboard_by_type(&self, pt: PieceType) -> Bitboard {
        self.bitboard_by_type[pt as usize]
    }

    pub fn bitboard_by_color(&self, color: Color) -> Bitboard {
        self.bitboard_by_color[color as usize]
    }

    pub fn put_piece(&mut self, piece: Piece, square: Square) {
        self.board[square as usize] = piece;
        if piece != Piece::NoPiece {
            let pt = piece.piece_type();
            let c = piece.color().unwrap();
            self.bitboard_by_type[pt as usize].set_bit(square);
            self.bitboard_by_color[c as usize].set_bit(square);
            self.piece_count[piece as usize] += 1;
            let table = ZOBRIST.get_or_init(ZobristTable::init);
            self.zobrist_hash ^= table.pieces[piece as usize][square as usize];
        }
    }

    pub fn remove_piece(&mut self, square: Square) -> Piece {
        let piece = self.board[square as usize];
        if piece != Piece::NoPiece {
            let pt = piece.piece_type();
            let c = piece.color().unwrap();
            self.bitboard_by_type[pt as usize].clear_bit(square);
            self.bitboard_by_color[c as usize].clear_bit(square);
            self.piece_count[piece as usize] -= 1;
            let table = ZOBRIST.get_or_init(ZobristTable::init);
            self.zobrist_hash ^= table.pieces[piece as usize][square as usize];
            self.board[square as usize] = Piece::NoPiece;
        }
        piece
    }

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

    pub fn set(&mut self, fen: &str) -> Result<(), PositionSetError> {
        self.board = [Piece::NoPiece; Square::COUNT];
        self.bitboard_by_type = [Bitboard::new(); PieceType::COUNT];
        self.bitboard_by_color = [Bitboard::new(); Color::COUNT];
        self.piece_count = [0; 16];
        self.history.clear();
        self.game_ply = 0;
        self.zobrist_hash = 0;

        let tokens: Vec<&str> = fen.split_whitespace().collect();
        if tokens.is_empty() {
            return Err(PositionSetError {
                msg: "Empty FEN".to_string(),
            });
        }

        // 1. Piece placement
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
                        self.put_piece(piece, square);
                    } else {
                        return Err(PositionSetError {
                            msg: format!("Unknown piece character: {}", c),
                        });
                    }
                    file_idx += 1;
                }
            }
        }

        // 2. Active color
        if tokens.len() > 1 {
            self.side_to_move = match tokens[1] {
                "w" => Color::White,
                "b" => Color::Black,
                _ => {
                    return Err(PositionSetError {
                        msg: format!("Invalid side to move: {}", tokens[1]),
                    })
                }
            };
            if self.side_to_move == Color::Black {
                let table = ZOBRIST.get_or_init(ZobristTable::init);
                self.zobrist_hash ^= table.side;
            }
        } else {
            self.side_to_move = Color::White;
        }

        let mut rule50 = 0;
        if tokens.len() > 4 {
            if let Ok(r50) = tokens[4].parse::<u16>() {
                rule50 = r50;
            }
        }

        let mut fullmove = 1;
        if tokens.len() > 5 {
            if let Ok(fm) = tokens[5].parse::<u16>() {
                fullmove = fm;
            }
        }
        self.game_ply = (fullmove.saturating_sub(1) * 2) + (self.side_to_move as u16);

        self.history.push(StateInfo {
            captured_piece: Piece::NoPiece,
            old_zobrist: self.zobrist_hash,
            rule50,
        });

        Ok(())
    }

    pub fn do_move(&mut self, m: Move) {
        let from = m.square_from();
        let to = m.square_to();
        let piece = self.board[from as usize];
        let captured = self.board[to as usize];

        let rule50 = self.history.last().map(|s| s.rule50).unwrap_or(0);
        let old_zobrist = self.zobrist_hash;

        // Push current state onto stack before modifying
        self.history.push(StateInfo {
            captured_piece: captured,
            old_zobrist,
            rule50,
        });

        self.remove_piece(from);
        if captured != Piece::NoPiece {
            self.remove_piece(to);
        }
        self.put_piece(piece, to);

        // Update halfmove clock / rule50
        let new_rule50 = if piece.piece_type() == PieceType::Pawn || captured != Piece::NoPiece {
            0
        } else {
            rule50 + 1
        };
        if let Some(last) = self.history.last_mut() {
            last.rule50 = new_rule50;
        }

        // Toggle side to move
        let table = ZOBRIST.get_or_init(ZobristTable::init);
        self.zobrist_hash ^= table.side;
        self.side_to_move = self.side_to_move.opposite();
        self.game_ply += 1;
    }

    pub fn undo_move(&mut self, m: Move) {
        let from = m.square_from();
        let to = m.square_to();
        let piece = self.board[to as usize];

        // Pop last state
        let state = self.history.pop().expect("No state in history to undo");

        self.remove_piece(to);
        self.put_piece(piece, from);
        if state.captured_piece != Piece::NoPiece {
            self.put_piece(state.captured_piece, to);
        }

        self.zobrist_hash = state.old_zobrist;
        self.side_to_move = self.side_to_move.opposite();
        self.game_ply -= 1;
    }

    pub fn is_empty(&self, square: Square) -> bool {
        self.board[square as usize] == Piece::NoPiece
    }

    pub fn king_square(&self, color: Color) -> Option<Square> {
        let king_piece = match color {
            Color::White => Piece::WhiteKing,
            Color::Black => Piece::BlackKing,
        };
        for sq in 0..90 {
            if self.board[sq] == king_piece {
                return Some(Square::from_repr(sq as u8).unwrap());
            }
        }
        None
    }

    pub fn gives_check(&self, m: Move) -> bool {
        let mut temp_pos = self.clone();
        temp_pos.do_move(m);
        let opp_king_square = temp_pos.king_square(temp_pos.side_to_move);
        if let Some(sq) = opp_king_square {
            temp_pos.is_square_attacked(sq, temp_pos.side_to_move.opposite())
        } else {
            false
        }
    }

    pub fn is_in_check(&self, color: Color) -> bool {
        if let Some(sq) = self.king_square(color) {
            self.is_square_attacked(sq, color.opposite())
        } else {
            false
        }
    }

    pub fn is_square_attacked(&self, square: Square, attacker: Color) -> bool {
        let f = (square as i8) % 9;
        let r = (square as i8) / 9;

        // 1. Orthogonal attacks: Rook & Cannon & Flying General (Attacking King)
        let directions = [(0, 1), (0, -1), (1, 0), (-1, 0)];
        for &(df, dr) in &directions {
            let mut nf = f;
            let mut nr = r;
            let mut screen_count = 0;
            loop {
                nf += df;
                nr += dr;
                if nf < 0 || nf >= 9 || nr < 0 || nr >= 10 {
                    break;
                }
                let sq = Square::from_file_rank(File::from_repr(nf as u8).unwrap(), Rank::from_repr(nr as u8).unwrap());
                let piece = self.board[sq as usize];
                if piece != Piece::NoPiece {
                    if screen_count == 0 {
                        if piece.color() == Some(attacker) {
                            let pt = piece.piece_type();
                            if pt == PieceType::Rook {
                                return true;
                            }
                            if pt == PieceType::King && df == 0 {
                                // Flying General rule: king on the same file with no pieces in between
                                return true;
                            }
                        }
                        screen_count += 1;
                    } else if screen_count == 1 {
                        if piece.color() == Some(attacker) && piece.piece_type() == PieceType::Cannon {
                            return true;
                        }
                        break;
                    }
                }
            }
        }

        // 2. Horse (Knight) attacks
        let horse_jumps = [
            (1, 2, 0, 1),   // target (+1, +2), leg (0, +1)
            (-1, 2, 0, 1),  // target (-1, +2), leg (0, +1)
            (1, -2, 0, -1),  // target (+1, -2), leg (0, -1)
            (-1, -2, 0, -1), // target (-1, -2), leg (0, -1)
            (2, 1, 1, 0),   // target (+2, +1), leg (+1, 0)
            (2, -1, 1, 0),  // target (+2, -1), leg (+1, 0)
            (-2, 1, -1, 0),  // target (-2, +1), leg (-1, 0)
            (-2, -1, -1, 0), // target (-2, -1), leg (-1, 0)
        ];
        for &(df, dr, lf, lr) in &horse_jumps {
            let target_f = f + df;
            let target_r = r + dr;
            if target_f >= 0 && target_f < 9 && target_r >= 0 && target_r < 10 {
                let leg_f = f + lf;
                let leg_r = r + lr;
                let leg_sq = Square::from_file_rank(File::from_repr(leg_f as u8).unwrap(), Rank::from_repr(leg_r as u8).unwrap());
                if self.board[leg_sq as usize] == Piece::NoPiece {
                    let target_sq = Square::from_file_rank(File::from_repr(target_f as u8).unwrap(), Rank::from_repr(target_r as u8).unwrap());
                    let piece = self.board[target_sq as usize];
                    if piece.color() == Some(attacker) && piece.piece_type() == PieceType::Knight {
                        return true;
                    }
                }
            }
        }

        // 3. Elephant (Bishop) attacks
        let elephant_jumps = [
            (2, 2, 1, 1),
            (2, -2, 1, -1),
            (-2, 2, -1, 1),
            (-2, -2, -1, -1),
        ];
        for &(df, dr, lf, lr) in &elephant_jumps {
            let target_f = f + df;
            let target_r = r + dr;
            if target_f >= 0 && target_f < 9 && target_r >= 0 && target_r < 10 {
                let mid_f = f + lf;
                let mid_r = r + lr;
                let mid_sq = Square::from_file_rank(File::from_repr(mid_f as u8).unwrap(), Rank::from_repr(mid_r as u8).unwrap());
                if self.board[mid_sq as usize] == Piece::NoPiece {
                    let target_sq = Square::from_file_rank(File::from_repr(target_f as u8).unwrap(), Rank::from_repr(target_r as u8).unwrap());
                    let piece = self.board[target_sq as usize];
                    if piece.color() == Some(attacker) && piece.piece_type() == PieceType::Bishop {
                        return true;
                    }
                }
            }
        }

        // 4. Advisor attacks
        let advisor_jumps = [
            (1, 1), (1, -1), (-1, 1), (-1, -1)
        ];
        for &(df, dr) in &advisor_jumps {
            let target_f = f + df;
            let target_r = r + dr;
            if target_f >= 0 && target_f < 9 && target_r >= 0 && target_r < 10 {
                let target_sq = Square::from_file_rank(File::from_repr(target_f as u8).unwrap(), Rank::from_repr(target_r as u8).unwrap());
                let piece = self.board[target_sq as usize];
                if piece.color() == Some(attacker) && piece.piece_type() == PieceType::Advisor {
                    return true;
                }
            }
        }

        // 5. King attacks (orthogonal 1 step)
        let king_jumps = [
            (0, 1), (0, -1), (1, 0), (-1, 0)
        ];
        for &(df, dr) in &king_jumps {
            let target_f = f + df;
            let target_r = r + dr;
            if target_f >= 0 && target_f < 9 && target_r >= 0 && target_r < 10 {
                let target_sq = Square::from_file_rank(File::from_repr(target_f as u8).unwrap(), Rank::from_repr(target_r as u8).unwrap());
                let piece = self.board[target_sq as usize];
                if piece.color() == Some(attacker) && piece.piece_type() == PieceType::King {
                    return true;
                }
            }
        }

        // 6. Pawn attacks
        match attacker {
            Color::White => {
                if r - 1 >= 0 {
                    let pawn_sq = Square::from_file_rank(File::from_repr(f as u8).unwrap(), Rank::from_repr((r - 1) as u8).unwrap());
                    let piece = self.board[pawn_sq as usize];
                    if piece == Piece::WhitePawn {
                        return true;
                    }
                }
                if r >= 5 {
                    for df in &[-1, 1] {
                        let target_f = f + df;
                        if target_f >= 0 && target_f < 9 {
                            let pawn_sq = Square::from_file_rank(File::from_repr(target_f as u8).unwrap(), Rank::from_repr(r as u8).unwrap());
                            let piece = self.board[pawn_sq as usize];
                            if piece == Piece::WhitePawn {
                                return true;
                            }
                        }
                    }
                }
            }
            Color::Black => {
                if r + 1 < 10 {
                    let pawn_sq = Square::from_file_rank(File::from_repr(f as u8).unwrap(), Rank::from_repr((r + 1) as u8).unwrap());
                    let piece = self.board[pawn_sq as usize];
                    if piece == Piece::BlackPawn {
                        return true;
                    }
                }
                if r <= 4 {
                    for df in &[-1, 1] {
                        let target_f = f + df;
                        if target_f >= 0 && target_f < 9 {
                            let pawn_sq = Square::from_file_rank(File::from_repr(target_f as u8).unwrap(), Rank::from_repr(r as u8).unwrap());
                            let piece = self.board[pawn_sq as usize];
                            if piece == Piece::BlackPawn {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }
}
