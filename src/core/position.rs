use std::sync::OnceLock;
use strum::EnumCount;
use thiserror::Error;

use crate::{
    core::{
        bitboard::Bitboard,
        movegen::{KNIGHT_TO_TABLE, PAWN_ATTACKS_TO, cannon_attacks, rook_attacks},
        types::{BloomFilter, Color, File, Move, Piece, PieceType, Rank, Square},
    },
    eval::piece_square_table::{piece_material_value, piece_square_table_value},
};

/// A fast Linear Congruential Generator (LCG) used to generate pseudo-random numbers
/// for Zobrist position hashing.
/// Uses standard Knuth MMIX LCG values:
/// * **Multiplier**: `6364136223846793005`
/// * **Increment**: `1442695040888963407`
struct Lcg {
    state: u64,
}

impl Lcg {
    /// Creates a new LCG starting from a given seed.
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    /// Returns the next 64-bit pseudo-random number in the sequence.
    fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }
}

/// Holds Zobrist random numbers used for fast, incremental position hashing.
/// Positional hashes are updated via XOR operations during do_move/undo_move,
/// entirely avoiding full-board hash recalculations.
struct ZobristTable {
    /// Random keys for every piece type on every one of the 90 squares:
    /// Categorized as `pieces[piece_index][square_index]`.
    pieces: [[u64; 90]; 16],
    /// XOR'ed into the hash if it is Black's turn to move.
    side: u64,
}

static ZOBRIST: OnceLock<ZobristTable> = OnceLock::new();

impl ZobristTable {
    /// Initializes the Zobrist key matrix using our deterministic LCG starting from seed `1070372`.
    fn init() -> Self {
        let mut prng = Lcg::new(1070372);
        let mut pieces = [[0u64; 90]; 16];
        for piece in &mut pieces {
            for square in piece {
                *square = prng.next();
            }
        }
        let side = prng.next();
        Self { pieces, side }
    }
}

/// Represents search state parameters that must be saved on a stack,
/// allowing incremental undo_move restorations.
#[derive(Clone, Copy, Debug)]
pub struct StateInfo {
    /// The exact move executed to reach this state.
    pub last_move: Move,
    /// The piece captured during this move, or `Piece::None` if it was quiet.
    pub captured_piece: Piece,
    /// The prior Zobrist position hash value before the move occurred.
    pub old_zobrist: u64,
    /// Halfmove clock / 60-rule counter (increments on quiet moves, resets to 0 on captures/pawn moves).
    pub rule60: u16,
    /// Whether each color [White, Black] was in check in this position state.
    pub in_check: [bool; 2],
    /// Precalculated incremental material score (from White's perspective)
    pub material_score: i32,
    /// Precalculated incremental piece-square table positional score (from White's perspective)
    pub piece_square_table_score: i32,
}

/// Encapsulates the complete game board representation, bitboards, turn tracking, ply count,
/// and incremental state histories for the engine.
#[derive(Clone)]
pub struct Position {
    /// Flat 90-square array mapping square index (0 to 89) to the Piece occupying it.
    board: [Piece; Square::COUNT],
    /// Precomputed bitboards showing piece placements grouped by `PieceType`.
    bitboard_by_type: [Bitboard; PieceType::COUNT],
    /// Precomputed bitboards showing piece placements grouped by `Color`.
    bitboard_by_color: [Bitboard; Color::COUNT],

    /// Active count of each piece category on the board.
    piece_count: [u8; 16],
    /// Stack tracking previous move parameter histories for undoing moves.
    history: Vec<StateInfo>,
    /// Total moves played in the game so far (White = 0, Black = 1, White's next = 2, etc.).
    game_ply: u16,
    /// The player active to play next.
    side_to_move: Color,

    #[allow(dead_code)]
    filter: BloomFilter,

    #[allow(dead_code)]
    id_board: [u8; Square::COUNT],
    /// Current transposition hash of the board position.
    pub zobrist_hash: u64,
    /// Palace coordinates of both players' Generals (Kings) to expedite check scans.
    king_squares: [Square; 2],
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
    /// Constructs a clean, completely empty board position state.
    pub fn new() -> Self {
        let mut pos = Self {
            board: [Piece::None; Square::COUNT],
            bitboard_by_type: [Bitboard::new(); PieceType::COUNT],
            bitboard_by_color: [Bitboard::new(); Color::COUNT],
            piece_count: [0; 16],
            history: Vec::new(),
            game_ply: 0,
            side_to_move: Color::White,
            filter: BloomFilter::new(),
            id_board: [0; Square::COUNT],
            zobrist_hash: 0,
            king_squares: [Square::E0, Square::E9],
        };
        // Setup initial empty history state
        pos.history.push(StateInfo {
            last_move: Move::none(),
            captured_piece: Piece::None,
            old_zobrist: 0,
            rule60: 0,
            in_check: [false, false],
            material_score: 0,
            piece_square_table_score: 0,
        });
        pos
    }

    #[inline(always)]
    pub fn material_score(&self) -> i32 {
        self.history.last().map(|s| s.material_score).unwrap_or(0)
    }

    #[inline(always)]
    pub fn piece_square_table_score(&self) -> i32 {
        self.history
            .last()
            .map(|s| s.piece_square_table_score)
            .unwrap_or(0)
    }

    /// Computes the complete material and Piece-Square Table scores from scratch.
    pub fn compute_evaluation_scores(&self) -> (i32, i32) {
        let mut material_score = 0;
        let mut piece_square_table_score = 0;
        for sq_idx in 0..Square::COUNT {
            let sq = Square::from_repr(sq_idx as u8).unwrap();
            let piece = self.board[sq_idx];
            if piece != Piece::None {
                let color = piece.color().unwrap();
                let val = piece_material_value(piece, sq);
                let pst = piece_square_table_value(piece.piece_type(), color, sq);
                if color == Color::White {
                    material_score += val;
                    piece_square_table_score += pst;
                } else {
                    material_score -= val;
                    piece_square_table_score -= pst;
                }
            }
        }
        (material_score, piece_square_table_score)
    }

    #[inline(always)]
    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    #[inline(always)]
    pub fn piece_at(&self, square: Square) -> Piece {
        self.board[square as usize]
    }

    #[inline(always)]
    pub fn piece_count(&self, piece: Piece) -> u8 {
        self.piece_count[piece as usize]
    }

    #[inline(always)]
    pub fn bitboard_by_type(&self, pt: PieceType) -> Bitboard {
        self.bitboard_by_type[pt as usize]
    }

    #[inline(always)]
    pub fn bitboard_by_color(&self, color: Color) -> Bitboard {
        self.bitboard_by_color[color as usize]
    }

    /// Safely introduces a piece onto a board square, updating the type/color bitboards,
    /// King Palace trackers, and XORing its random signature into the Zobrist hash.
    pub fn put_piece(&mut self, piece: Piece, square: Square) {
        self.board[square as usize] = piece;
        if piece != Piece::None {
            let pt = piece.piece_type();
            let c = piece.color().unwrap();
            self.bitboard_by_type[pt as usize].set_bit(square);
            self.bitboard_by_color[c as usize].set_bit(square);
            self.piece_count[piece as usize] += 1;

            if pt == PieceType::King {
                self.king_squares[c as usize] = square;
            }

            let table = ZOBRIST.get_or_init(ZobristTable::init);
            self.zobrist_hash ^= table.pieces[piece as usize][square as usize];
        }
    }

    /// Removes a piece from a board square, cleaning up placement bitboards,
    /// Zobrist position signatures, and returning the removed piece.
    pub fn remove_piece(&mut self, square: Square) -> Piece {
        let piece = self.board[square as usize];
        if piece != Piece::None {
            let pt = piece.piece_type();
            let c = piece.color().unwrap();
            self.bitboard_by_type[pt as usize].clear_bit(square);
            self.bitboard_by_color[c as usize].clear_bit(square);
            self.piece_count[piece as usize] -= 1;
            let table = ZOBRIST.get_or_init(ZobristTable::init);
            self.zobrist_hash ^= table.pieces[piece as usize][square as usize];
            self.board[square as usize] = Piece::None;
        }
        piece
    }

    /// Maps standard algebraic piece FEN notation characters to their Piece enums.
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

    /// Parses and initializes the position state from a standard FEN string.
    pub fn set(&mut self, fen: &str) -> Result<(), PositionSetError> {
        self.board = [Piece::None; Square::COUNT];
        self.bitboard_by_type = [Bitboard::new(); PieceType::COUNT];
        self.bitboard_by_color = [Bitboard::new(); Color::COUNT];
        self.piece_count = [0; 16];
        self.king_squares = [Square::E0, Square::E9];
        self.history.clear();
        self.game_ply = 0;
        self.zobrist_hash = 0;

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
                        self.put_piece(piece, square);
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
        if tokens.len() > 1 {
            self.side_to_move = match tokens[1] {
                "w" => Color::White,
                "b" => Color::Black,
                _ => {
                    return Err(PositionSetError {
                        msg: format!("Invalid side to move: {}", tokens[1]),
                    });
                }
            };
            if self.side_to_move == Color::Black {
                let table = ZOBRIST.get_or_init(ZobristTable::init);
                self.zobrist_hash ^= table.side;
            }
        } else {
            self.side_to_move = Color::White;
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
        self.game_ply = (fullmove.saturating_sub(1) * 2) + (self.side_to_move as u16);

        let in_check = [
            self.is_in_check(Color::White),
            self.is_in_check(Color::Black),
        ];
        let (material_score, piece_square_table_score) = self.compute_evaluation_scores();
        self.history.push(StateInfo {
            last_move: Move::none(),
            captured_piece: Piece::None,
            old_zobrist: self.zobrist_hash,
            rule60,
            in_check,
            material_score,
            piece_square_table_score,
        });

        Ok(())
    }

    /// Plays a move on the board, saving prior ply parameter states onto the stack
    /// to support fast undo restores, and toggles active side.
    pub fn do_move(&mut self, m: Move) {
        let from = m.square_from();
        let to = m.square_to();
        let piece = self.board[from as usize];
        let captured = self.board[to as usize];

        let last_state = self.history.last().expect("History stack is empty");
        let rule60 = last_state.rule60;
        let old_zobrist = self.zobrist_hash;
        let mut material_score = last_state.material_score;
        let mut piece_square_table_score = last_state.piece_square_table_score;

        // 1. Remove piece from from
        if piece.color() == Some(Color::White) {
            material_score -= crate::eval::piece_square_table::piece_material_value(piece, from);
            piece_square_table_score -= crate::eval::piece_square_table::piece_square_table_value(
                piece.piece_type(),
                Color::White,
                from,
            );
        } else if piece.color() == Some(Color::Black) {
            material_score += crate::eval::piece_square_table::piece_material_value(piece, from);
            piece_square_table_score += crate::eval::piece_square_table::piece_square_table_value(
                piece.piece_type(),
                Color::Black,
                from,
            );
        }

        // 2. Remove captured piece from to (if any)
        if captured != Piece::None {
            if captured.color() == Some(Color::White) {
                material_score -=
                    crate::eval::piece_square_table::piece_material_value(captured, to);
                piece_square_table_score -=
                    crate::eval::piece_square_table::piece_square_table_value(
                        captured.piece_type(),
                        Color::White,
                        to,
                    );
            } else if captured.color() == Some(Color::Black) {
                material_score +=
                    crate::eval::piece_square_table::piece_material_value(captured, to);
                piece_square_table_score +=
                    crate::eval::piece_square_table::piece_square_table_value(
                        captured.piece_type(),
                        Color::Black,
                        to,
                    );
            }
        }

        // 3. Put piece at to
        if piece.color() == Some(Color::White) {
            material_score += crate::eval::piece_square_table::piece_material_value(piece, to);
            piece_square_table_score += crate::eval::piece_square_table::piece_square_table_value(
                piece.piece_type(),
                Color::White,
                to,
            );
        } else if piece.color() == Some(Color::Black) {
            material_score -= crate::eval::piece_square_table::piece_material_value(piece, to);
            piece_square_table_score -= crate::eval::piece_square_table::piece_square_table_value(
                piece.piece_type(),
                Color::Black,
                to,
            );
        }

        // Push current state onto history stack
        self.history.push(StateInfo {
            last_move: m,
            captured_piece: captured,
            old_zobrist,
            rule60,
            in_check: [false, false],
            material_score,
            piece_square_table_score,
        });

        self.remove_piece(from);
        if captured != Piece::None {
            self.remove_piece(to);
        }
        self.put_piece(piece, to);

        // Update rule60 halfmove clock
        let new_rule60 = if piece.piece_type() == PieceType::Pawn || captured != Piece::None {
            0
        } else {
            rule60 + 1
        };
        let in_check = [
            self.is_in_check(Color::White),
            self.is_in_check(Color::Black),
        ];
        if let Some(last) = self.history.last_mut() {
            last.rule60 = new_rule60;
            last.in_check = in_check;
        }

        // Toggle side to move
        let table = ZOBRIST.get_or_init(ZobristTable::init);
        self.zobrist_hash ^= table.side;
        self.side_to_move = self.side_to_move.opposite();
        self.game_ply += 1;
    }

    /// Restores the position to the exact state before the last move was played,
    /// popping details off the stack and re-toggling side to move.
    pub fn undo_move(&mut self, m: Move) {
        let from = m.square_from();
        let to = m.square_to();
        let piece = self.board[to as usize];

        let state = self.history.pop().expect("No state in history to undo");

        self.remove_piece(to);
        self.put_piece(piece, from);
        if state.captured_piece != Piece::None {
            self.put_piece(state.captured_piece, to);
        }

        self.zobrist_hash = state.old_zobrist;
        self.side_to_move = self.side_to_move.opposite();
        self.game_ply -= 1;
    }

    #[inline(always)]
    pub fn is_empty(&self, square: Square) -> bool {
        self.board[square as usize] == Piece::None
    }

    #[inline(always)]
    pub fn king_square(&self, color: Color) -> Option<Square> {
        Some(self.king_squares[color as usize])
    }

    /// Identifies all opponent pieces of `attacker` color that attack the target `square`
    /// assuming the given board `occupied` bitboard.
    ///
    /// This represents an extremely optimized, loop-free backward bitwise scanner.
    /// Rather than generating all moves for all opponent pieces, we shoot out rays and leaps
    /// backwards starting from `square`:
    ///
    /// 1. **Pawn Scanner**: Traces reverse Pawn attack positions using static `PAWN_ATTACKS_TO`.
    /// 2. **Knight Scanner**: Gathers the 6 unique Horse Legs around `square` into a 6-bit occupancy mask,
    ///    looking up valid attack origin positions in `KNIGHT_TO_TABLE`.
    /// 3. **Rook & King Scanner**: Traces orthogonal lines using precomputed sliding rank/file masks.
    ///    Under the "Flying General" rule in Xiangqi, Kings cannot face each other directly along a file
    ///    without intervening pieces; this direct face is treated as a Rook check.
    /// 4. **Cannon Scanner**: Traces split rank/file leap capture paths using precomputed Cannon tables.
    #[inline(always)]
    fn checkers_to(&self, square: Square, occupied: Bitboard, attacker: Color) -> Bitboard {
        // --- Isolate attacker's piece bitboards by intersecting piece-type and color masks ---
        // Each variable holds bits only for attacker-colored pieces of that type.
        let opponent_pawns = self.bitboard_by_type[PieceType::Pawn as usize]
            & self.bitboard_by_color[attacker as usize]; // attacker's Pawns
        let opponent_knights = self.bitboard_by_type[PieceType::Knight as usize]
            & self.bitboard_by_color[attacker as usize]; // attacker's Knights
        let opponent_rooks = self.bitboard_by_type[PieceType::Rook as usize]
            & self.bitboard_by_color[attacker as usize]; // attacker's Rooks
        let opponent_cannons = self.bitboard_by_type[PieceType::Cannon as usize]
            & self.bitboard_by_color[attacker as usize]; // attacker's Cannons
        let opponent_king = self.bitboard_by_type[PieceType::King as usize]
            & self.bitboard_by_color[attacker as usize]; // attacker's King (Flying General rule)

        // --- Pawn scanner ---
        // Map attacker color to a table index (0 = White, 1 = Black).
        // White and Black Pawns attack in opposite directions, so each has its own reverse-attack table.
        let them_color_idx = if attacker == Color::White { 0 } else { 1 };
        // `PAWN_ATTACKS_TO[color][sq]` gives the set of squares FROM which a Pawn of that color
        // could have attacked `square`. AND with actual Pawn positions to find real attackers.
        let pawn_attackers = PAWN_ATTACKS_TO[them_color_idx][square as usize].0 & opponent_pawns.0;

        // --- Knight scanner (Horse-Leg / blocking-pin aware) ---
        // Look up the precomputed entry for `square` in the reverse Knight attack table.
        // Each entry stores up to 6 "eye" squares (the leg-blocking squares around `square`)
        // and a 64-entry array of attack masks indexed by a 6-bit occupancy key.
        let entry = &KNIGHT_TO_TABLE[square as usize];
        let mut occ_idx = 0; // will become a 6-bit mask of which eye squares are occupied
        let mut i = 0;
        while i < 6 {
            // For each potential eye square (the square a Knight must pass through on its L-move)...
            if let Some(eye_sq) = entry.eyes[i] {
                // ...set bit `i` in occ_idx if that eye square is currently occupied (leg is blocked).
                if occupied.is_occupied(eye_sq) {
                    occ_idx |= 1 << i;
                }
            }
            i += 1;
        }
        // Use the 6-bit occupancy key to look up which Knights can actually reach `square`
        // (only those whose leg is NOT blocked), then intersect with real Knight positions.
        let knight_attackers = entry.attacks[occ_idx].0 & opponent_knights.0;

        // --- Rook & King scanner (orthogonal sliding rays + Flying General rule) ---
        // Compute all squares reachable by a Rook standing on `square` given `occupied`.
        // A Rook on the target square sees exactly the squares that can send a Rook check.
        let rook_atk = rook_attacks(square, occupied);
        // Intersect with attacker Rooks AND the attacker King: under the Flying General rule,
        // two Kings facing each other on an open file counts as a check (treated as a Rook attack).
        let rook_attackers = rook_atk.0 & (opponent_rooks.0 | opponent_king.0);

        // --- Cannon scanner (platform-leap captures) ---
        // Compute Cannon attack squares from `square`: Cannons capture by leaping over exactly
        // one intervening piece (the "platform"). `cannon_attacks` returns squares that have
        // exactly one piece between them and `square` along a rank or file.
        let cannon_atk = cannon_attacks(square, occupied);
        // Intersect with attacker Cannons only (Kings/Rooks cannot leap over platforms).
        let cannon_attackers = cannon_atk.0 & opponent_cannons.0;

        // Union all four attacker bitboards into a single result: every square occupied by
        // an attacker-colored piece that can reach `square` under the current board occupancy.
        Bitboard(pawn_attackers | knight_attackers | rook_attackers | cannon_attackers)
    }

    /// Evaluates backward checkers attacking `square` after simulating a specific piece move.
    /// Overrides positions without modifying active board structures.
    #[inline(always)]
    fn checkers_to_after_move(
        &self,
        square: Square,
        occupied: Bitboard,
        attacker: Color,
        from: Square,
        to: Square,
        moved_piece: Piece,
    ) -> Bitboard {
        let captured = self.board[to as usize];
        let was_captured = captured != Piece::None && captured.color() == Some(attacker);

        let mut opponent_pawns = self.bitboard_by_type[PieceType::Pawn as usize]
            & self.bitboard_by_color[attacker as usize];
        let mut opponent_knights = self.bitboard_by_type[PieceType::Knight as usize]
            & self.bitboard_by_color[attacker as usize];
        let mut opponent_rooks = self.bitboard_by_type[PieceType::Rook as usize]
            & self.bitboard_by_color[attacker as usize];
        let mut opponent_cannons = self.bitboard_by_type[PieceType::Cannon as usize]
            & self.bitboard_by_color[attacker as usize];
        let mut opponent_king = self.bitboard_by_type[PieceType::King as usize]
            & self.bitboard_by_color[attacker as usize];

        if was_captured {
            let captured_pt = captured.piece_type();
            match captured_pt {
                PieceType::Pawn => opponent_pawns.clear_bit(to),
                PieceType::Knight => opponent_knights.clear_bit(to),
                PieceType::Rook => opponent_rooks.clear_bit(to),
                PieceType::Cannon => opponent_cannons.clear_bit(to),
                PieceType::King => opponent_king.clear_bit(to),
                _ => {}
            }
        }

        let moved_by_attacker = moved_piece.color() == Some(attacker);
        if moved_by_attacker {
            let pt = moved_piece.piece_type();
            match pt {
                PieceType::Pawn => {
                    opponent_pawns.clear_bit(from);
                    opponent_pawns.set_bit(to);
                }
                PieceType::Knight => {
                    opponent_knights.clear_bit(from);
                    opponent_knights.set_bit(to);
                }
                PieceType::Rook => {
                    opponent_rooks.clear_bit(from);
                    opponent_rooks.set_bit(to);
                }
                PieceType::Cannon => {
                    opponent_cannons.clear_bit(from);
                    opponent_cannons.set_bit(to);
                }
                PieceType::King => {
                    opponent_king.clear_bit(from);
                    opponent_king.set_bit(to);
                }
                _ => {}
            }
        }

        let them_color_idx = if attacker == Color::White { 0 } else { 1 };
        let pawn_attackers = PAWN_ATTACKS_TO[them_color_idx][square as usize].0 & opponent_pawns.0;

        let entry = &KNIGHT_TO_TABLE[square as usize];
        let mut occ_idx = 0;
        let mut i = 0;
        while i < 6 {
            if let Some(eye_sq) = entry.eyes[i]
                && occupied.is_occupied(eye_sq)
            {
                occ_idx |= 1 << i;
            }
            i += 1;
        }
        let knight_attackers = entry.attacks[occ_idx].0 & opponent_knights.0;

        let rook_atk = rook_attacks(square, occupied);
        let rook_attackers = rook_atk.0 & (opponent_rooks.0 | opponent_king.0);

        let cannon_atk = cannon_attacks(square, occupied);
        let cannon_attackers = cannon_atk.0 & opponent_cannons.0;

        Bitboard(pawn_attackers | knight_attackers | rook_attackers | cannon_attackers)
    }

    /// Evaluates if playing the move `m` places the opponent's General in check.
    /// Runs a simulation update of `occupied` bitboards and calculates checkers pointing to the General.
    #[inline(always)]
    pub fn gives_check(&self, m: Move) -> bool {
        let us = self.side_to_move;
        let them = us.opposite();
        let from = m.square_from();
        let to = m.square_to();
        let moved_piece = self.board[from as usize];
        let them_king_sq = self.king_square(them).unwrap();

        let mut occupied = self.bitboard_by_color[Color::White as usize]
            | self.bitboard_by_color[Color::Black as usize];
        occupied.clear_bit(from);
        occupied.set_bit(to);

        self.checkers_to_after_move(them_king_sq, occupied, us, from, to, moved_piece)
            .0
            != 0
    }

    /// Checks if the given player's King is currently in check.
    #[inline(always)]
    pub fn is_in_check(&self, color: Color) -> bool {
        if let Some(sq) = self.king_square(color) {
            self.is_square_attacked(sq, color.opposite())
        } else {
            false
        }
    }

    #[inline(always)]
    pub fn last_captured_piece(&self) -> Piece {
        self.history
            .last()
            .map(|s| s.captured_piece)
            .unwrap_or(Piece::None)
    }

    /// Validates if a pseudo-legal move `m` is fully legal (i.e. the King is not left in check).
    #[inline(always)]
    pub fn legal(&self, m: Move) -> bool {
        let us = self.side_to_move;
        let from = m.square_from();
        let to = m.square_to();
        let moved_piece = self.board[from as usize];

        let king_sq = if moved_piece.piece_type() == PieceType::King {
            to
        } else {
            self.king_square(us).unwrap()
        };

        let mut occupied = self.bitboard_by_color[Color::White as usize]
            | self.bitboard_by_color[Color::Black as usize];
        occupied.clear_bit(from);
        occupied.set_bit(to);

        self.checkers_to_after_move(king_sq, occupied, us.opposite(), from, to, moved_piece)
            .0
            == 0
    }

    #[inline(always)]
    pub fn is_square_attacked(&self, square: Square, attacker: Color) -> bool {
        let occupied = self.bitboard_by_color[Color::White as usize]
            | self.bitboard_by_color[Color::Black as usize];
        self.checkers_to(square, occupied, attacker).0 != 0
    }

    #[inline(always)]
    pub fn is_square_attacked_after_move(
        &self,
        square: Square,
        attacker: Color,
        from: Square,
        to: Square,
        moved_piece: Piece,
    ) -> bool {
        let mut occupied = self.bitboard_by_color[Color::White as usize]
            | self.bitboard_by_color[Color::Black as usize];
        occupied.clear_bit(from);
        occupied.set_bit(to);

        self.checkers_to_after_move(square, occupied, attacker, from, to, moved_piece)
            .0
            != 0
    }

    /// Calculates the chase information for a given color, returning a 16-bit mask of chased pieces.
    pub fn chased(&mut self, mover: Color) -> u16 {
        use crate::core::movegen::{KNIGHT_TABLE, PAWN_ATTACKS};

        let mut chase = 0u16;
        let opponent = mover.opposite();
        let occupied = self.bitboard_by_color[Color::White as usize]
            | self.bitboard_by_color[Color::Black as usize];

        // 1. Target pieces that can be chased (excluding King):
        // Rooks, Cannons, Knights, Advisors, Bishops of the opponent,
        // and crossed-river Pawns of the opponent.
        let mut targets_mask = self.bitboard_by_type[PieceType::Rook as usize].0
            | self.bitboard_by_type[PieceType::Cannon as usize].0
            | self.bitboard_by_type[PieceType::Knight as usize].0
            | self.bitboard_by_type[PieceType::Advisor as usize].0
            | self.bitboard_by_type[PieceType::Bishop as usize].0;

        // Add crossed-river pawns of the opponent
        let opp_pawns = self.bitboard_by_type[PieceType::Pawn as usize].0
            & self.bitboard_by_color[opponent as usize].0;
        let opp_side_mask = Bitboard::side(mover).0; // The river-crossed zone is mover's side!
        targets_mask |= opp_pawns & opp_side_mask;

        // Filter targets to only include the opponent's pieces
        let targets = Bitboard(targets_mask & self.bitboard_by_color[opponent as usize].0);

        // 2. Chasing attackers:
        // Rooks, Cannons, Knights, and crossed-river Pawns of the mover.
        let mut attackers_mask = self.bitboard_by_type[PieceType::Rook as usize].0
            | self.bitboard_by_type[PieceType::Cannon as usize].0
            | self.bitboard_by_type[PieceType::Knight as usize].0;

        let my_pawns = self.bitboard_by_type[PieceType::Pawn as usize].0
            & self.bitboard_by_color[mover as usize].0;
        let my_side_mask = Bitboard::side(opponent).0; // river-crossed zone is opponent's side!
        attackers_mask |= my_pawns & my_side_mask;

        // Filter attackers to only include mover's pieces
        let mut attackers = Bitboard(attackers_mask & self.bitboard_by_color[mover as usize].0);

        // 3. Scan all attackers to see if they attack any target
        while let Some(from) = attackers.pop_lsb() {
            let piece = self.board[from as usize];
            let ptype = piece.piece_type();

            // Generate attacks from `from`
            let mut attacks = match ptype {
                PieceType::Rook => rook_attacks(from, occupied),
                PieceType::Cannon => cannon_attacks(from, occupied),
                PieceType::Knight => {
                    let entry = &KNIGHT_TABLE[from as usize];
                    let mut occ_idx = 0;
                    for i in 0..4 {
                        if let Some(eye_sq) = entry.eyes[i]
                            && occupied.is_occupied(eye_sq)
                        {
                            occ_idx |= 1 << i;
                        }
                    }
                    entry.attacks[occ_idx]
                }
                PieceType::Pawn => {
                    let color_idx = if mover == Color::White { 0 } else { 1 };
                    PAWN_ATTACKS[color_idx][from as usize]
                }
                _ => Bitboard::new(),
            };

            // Restrict attacks to only target the opponent's pieces that are valid targets
            attacks &= targets;

            while let Some(to) = attacks.pop_lsb() {
                let m = Move::new(from, to);

                // Verify if the move is legal (meaning the king of mover is not in check after the move)
                if self.legal(m) {
                    let target_piece = self.board[to as usize];
                    let target_ptype = target_piece.piece_type();

                    // Relative value rules:

                    // Rule A: Attacks against stronger pieces
                    // Knight or Cannon attacking Rook -> chase
                    if (ptype == PieceType::Knight || ptype == PieceType::Cannon)
                        && target_ptype == PieceType::Rook
                    {
                        chase |= 1 << self.id_board[to as usize];
                        continue;
                    }

                    // Rule B: Attacks against potentially unprotected pieces
                    let mut true_chase = true;
                    let saved_side = self.side_to_move;

                    let old_type_from = self.board[from as usize];
                    let old_type_to = self.board[to as usize];

                    // Play move:
                    self.board[from as usize] = Piece::None;
                    self.board[to as usize] = old_type_from;

                    // Update bitboards
                    self.bitboard_by_color[mover as usize].clear_bit(from);
                    self.bitboard_by_color[mover as usize].set_bit(to);
                    self.bitboard_by_color[opponent as usize].clear_bit(to);

                    self.bitboard_by_type[ptype as usize].clear_bit(from);
                    self.bitboard_by_type[ptype as usize].set_bit(to);
                    self.bitboard_by_type[target_ptype as usize].clear_bit(to);

                    // We temporarily toggle side_to_move to opponent
                    self.side_to_move = opponent;

                    // Now see if any of the opponent's pieces can legally recapture at `to`
                    let recaptured_occupied = self.bitboard_by_color[Color::White as usize]
                        | self.bitboard_by_color[Color::Black as usize];

                    let mut recapturers = self.checkers_to(to, recaptured_occupied, opponent);
                    while let Some(s) = recapturers.pop_lsb() {
                        if self.legal(Move::new(s, to)) {
                            true_chase = false;
                            break;
                        }
                    }

                    // Restore board and bitboards:
                    self.board[from as usize] = old_type_from;
                    self.board[to as usize] = old_type_to;

                    self.bitboard_by_color[mover as usize].set_bit(from);
                    self.bitboard_by_color[mover as usize].clear_bit(to);
                    if old_type_to != Piece::None {
                        self.bitboard_by_color[opponent as usize].set_bit(to);
                    }

                    self.bitboard_by_type[ptype as usize].set_bit(from);
                    self.bitboard_by_type[ptype as usize].clear_bit(to);
                    if old_type_to != Piece::None {
                        self.bitboard_by_type[target_ptype as usize].set_bit(to);
                    }

                    self.side_to_move = saved_side;

                    if true_chase {
                        // Exclude mutual/symmetric attacks except pins
                        if ptype == target_ptype {
                            // If same type (e.g. Rook attacking Rook):
                            // Check if the opponent's piece cannot legally capture back.
                            self.side_to_move = opponent;
                            let can_opp_capture_back = self.legal(Move::new(to, from));
                            self.side_to_move = saved_side;

                            if !can_opp_capture_back {
                                chase |= 1 << self.id_board[to as usize];
                            }
                        } else {
                            chase |= 1 << self.id_board[to as usize];
                        }
                    }
                }
            }
        }

        chase
    }

    /// Detects chases from state st - d to state st on a rollback clone of self.
    pub fn detect_chases(&mut self, d: usize, ply: i32) -> i32 {
        let n = self.history.len();
        if n < d {
            return 0; // Draw
        }

        // Grant each piece on board a unique ID for each side
        let mut white_id = 0;
        let mut black_id = 0;
        self.id_board = [0; Square::COUNT];
        for sq_idx in 0..Square::COUNT {
            let sq = Square::from_repr(sq_idx as u8).unwrap();
            let piece = self.board[sq as usize];
            if piece != Piece::None {
                if piece.color() == Some(Color::White) {
                    self.id_board[sq as usize] = white_id;
                    white_id += 1;
                } else {
                    self.id_board[sq as usize] = black_id;
                    black_id += 1;
                }
            }
        }

        let us = self.side_to_move;
        let opponent = us.opposite();

        // Rollback until we reached st - d
        let mut chase = [0xFFFFu16, 0xFFFFu16];

        // Save the moves in the loop so we can undo them one by one
        let mut moves_in_loop = Vec::with_capacity(d);
        for i in 0..d {
            moves_in_loop.push(self.history[n - 1 - i].last_move);
        }

        for m in moves_in_loop {
            let state = self.history.last().unwrap();

            // Under Xiangqi rules, if the current side to move is in check, it overrides chase or is a draw.
            let side_to_move_idx = self.side_to_move as usize;
            if state.in_check[side_to_move_idx] {
                return 0; // Draw
            }

            let opposing_chase_mask = chase[self.side_to_move.opposite() as usize];
            if opposing_chase_mask == 0 {
                let our_chase_mask = chase[self.side_to_move as usize];
                if our_chase_mask == 0 {
                    break;
                }

                // Just undo move without computing chase diff
                self.undo_move(m);
            } else {
                let after = self.chased(self.side_to_move.opposite());
                self.undo_move(m);
                let before = self.chased(self.side_to_move);

                chase[self.side_to_move as usize] &= after & !before;
            }
        }

        let us_chasing = chase[us as usize] != 0;
        let them_chasing = chase[opponent as usize] != 0;

        if us_chasing && them_chasing {
            0 // Mutual chase -> draw
        } else if us_chasing {
            -100_000 + ply // We perpetually chase -> we lose
        } else if them_chasing {
            100_000 - ply // Opponent perpetually chases -> we win
        } else {
            0 // Normal draw
        }
    }

    /// Evaluates if the game has ended due to 60-move rule, insufficient material,
    /// or loops (normal draws, perpetual checking, or perpetual chasing).
    ///
    /// Returns:
    /// - `Some(0)` for a draw.
    /// - `Some(-100_000 + ply)` if the side to move is penalized (loses).
    /// - `Some(100_000 - ply)` if the opposing side is penalized (loses).
    /// - `None` if the game continues.
    pub fn rule_judge(&self, ply: i32) -> Option<i32> {
        let n = self.history.len();
        if n == 0 {
            return None;
        }

        // 1. 60-Move Rule (120 Plies since last pawn advance or capture)
        let rule60 = self.history.last().map(|s| s.rule60).unwrap_or(0);
        if rule60 >= 120 {
            return Some(0);
        }

        // 2. Insufficient Material Draw
        // If all Pawns are gone, check if remaining major pieces are capable of checkmating
        if self.piece_count(Piece::WhitePawn) == 0 && self.piece_count(Piece::BlackPawn) == 0 {
            let white_majors = self.piece_count(Piece::WhiteRook) as u32
                + self.piece_count(Piece::WhiteCannon) as u32
                + self.piece_count(Piece::WhiteKnight) as u32;
            let black_majors = self.piece_count(Piece::BlackRook) as u32
                + self.piece_count(Piece::BlackCannon) as u32
                + self.piece_count(Piece::BlackKnight) as u32;

            if white_majors == 0 && black_majors == 0 {
                // No Rooks, Cannons, or Knights remain on either side -> direct draw
                return Some(0);
            }

            // Exactly one Cannon left on the entire board, and no Advisors left
            let total_cannons =
                self.piece_count(Piece::WhiteCannon) + self.piece_count(Piece::BlackCannon);
            let total_advisors =
                self.piece_count(Piece::WhiteAdvisor) + self.piece_count(Piece::BlackAdvisor);
            if white_majors + black_majors == total_cannons as u32
                && total_cannons == 1
                && total_advisors == 0
            {
                return Some(0);
            }
        }

        // 3. Repetition & Perpetual Check/Chase Loops
        let current_hash = self.zobrist_hash;
        let rule60_val = rule60 as usize;
        let max_back = rule60_val.min(n - 1);

        // Repetitions must occur on the same side's turn, so we scan back in steps of 2 plies.
        let mut i = 4;
        while i <= max_back {
            if self.history[n - i].old_zobrist == current_hash {
                // Repetition loop detected!
                let mut us_perpetual_check = true;
                let mut them_perpetual_check = true;

                let us = self.side_to_move;
                let opponent = us.opposite();

                let initial_game_ply = self.game_ply - (n as u16 - 1);

                // Scan all intermediate plies in the loop (from `n - i` to `n - 1`)
                for j in (n - i)..n {
                    let state = &self.history[j];
                    let player_who_moved = if (initial_game_ply + (j as u16) - 1).is_multiple_of(2)
                    {
                        Color::White
                    } else {
                        Color::Black
                    };

                    if player_who_moved == us {
                        if !state.in_check[opponent as usize] {
                            us_perpetual_check = false;
                        }
                    } else {
                        if !state.in_check[us as usize] {
                            them_perpetual_check = false;
                        }
                    }
                }

                if us_perpetual_check || them_perpetual_check {
                    if us_perpetual_check && them_perpetual_check {
                        return Some(0); // Both check perpetually -> draw
                    } else if us_perpetual_check {
                        return Some(-100_000 + ply); // We check perpetually -> we lose
                    } else {
                        return Some(100_000 - ply); // Opponent checks perpetually -> they lose
                    }
                } else {
                    // No perpetual check, check perpetual chase
                    let mut rollback = self.clone();
                    let result = rollback.detect_chases(i, ply);
                    return Some(result);
                }
            }
            i += 2;
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Move, Square};

    #[test]
    fn test_knight_leg_pin() {
        let mut pos = Position::new();
        // White King at E0, White Advisor at F1, Black Knight at F2 (aligned to jump onto E0)
        pos.set("4k4/9/9/9/9/9/9/5h3/5A3/4K4 w - - 0 1").unwrap();
        // Since F1 blocks the Knight's jump, the Advisor is fully pinned and cannot move away
        assert!(!pos.legal(Move::new(Square::F1, Square::E2)));
        assert!(!pos.legal(Move::new(Square::F1, Square::G2)));
        assert!(!pos.legal(Move::new(Square::F1, Square::E0)));
        assert!(!pos.legal(Move::new(Square::F1, Square::G0)));
    }

    #[test]
    fn test_rule_judge_insufficient_material() {
        let mut pos = Position::new();
        // 1. Bare Kings
        pos.set("4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1").unwrap();
        assert_eq!(pos.rule_judge(0), Some(0));

        // 2. Kings + Bishops & Advisors (no attacking pieces)
        pos.set("2b1kab2/9/9/9/9/9/9/9/9/2B1KAB2 w - - 0 1")
            .unwrap();
        assert_eq!(pos.rule_judge(0), Some(0));

        // 3. Kings + 1 Cannon, no Advisors/Bishops
        pos.set("4k4/9/9/9/9/9/9/9/3C5/4K4 w - - 0 1").unwrap();
        assert_eq!(pos.rule_judge(0), Some(0));
    }

    #[test]
    fn test_rule_judge_60_move_rule() {
        let mut pos = Position::new();
        pos.set("4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1").unwrap();

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
        assert_eq!(pos.history.last().unwrap().rule60, 120);
        assert_eq!(pos.rule_judge(0), Some(0));
    }
}
