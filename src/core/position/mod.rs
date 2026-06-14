use crate::core::{Bitboard, File, Move, PackedScore, Piece, PieceType, Rank, Side, Square};
use anyhow::{Result, bail};
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
struct StateInfo {
    /// The last move played to reach this state.
    pub last_move: Option<Move>,
    /// The piece captured during this move, or `None` if it was quiet.
    pub captured_piece: Option<Piece>,
    /// The prior Zobrist position hash value before the move occurred.
    pub zobrist: u64,
    /// Halfmove clock / 60-rule counter (increments on quiet moves, resets to 0
    /// on captures/pawn moves).
    pub sixtymove_clock: u16,
    /// Whether the side to move was in check in this position state.
    pub in_check: bool,
    /// Precalculated incremental mid- and end-game score (from Red's
    /// perspective)
    pub score: PackedScore,
    /// Precalculated incremental game phase
    pub phase: u8,
    /// Checker pieces checking the King of the side to move.
    pub checkers: Bitboard,
    /// Pinned pieces (blockers) for both sides' kings.
    pub blockers_for_king: [Bitboard; Side::COUNT],
    /// The slider pieces pinning other pieces for both sides' kings.
    pub pinners: [Bitboard; Side::COUNT],
    /// Check squares for each piece type of the side to move (against the
    /// opponent king).
    pub check_squares: [Bitboard; PieceType::COUNT],
    /// Whether we need a full check validation.
    pub need_full_check: bool,
    /// Number of plies played since the last null move.
    pub plies_since_null: u16,
}

/// Encapsulates the complete game board representation, bitboards, turn
/// tracking, ply count, and incremental state histories for the engine.
#[derive(Clone)]
pub struct Position {
    /// Flat 90-square array mapping square index (0 to 89) to the Piece
    /// occupying it.
    board: [Option<Piece>; Square::COUNT], // 1 * 90 = 90 bytes
    /// Precomputed bitboards showing piece placements grouped by `PieceType`.
    bitboard_by_type: [Bitboard; PieceType::COUNT], // 7 * 16 = 112 bytes
    /// Precomputed bitboards showing piece placements grouped by `Color`.
    bitboard_by_color: [Bitboard; Side::COUNT], // 2 * 16 = 32 bytes
    /// Active count of each piece category on the board.
    piece_count: [u8; Piece::COUNT], // 15 bytes
    /// Current state of position
    state: StateInfo, // 224 bytes
    /// Stack tracking previous move parameter histories for undoing moves.
    history: Vec<StateInfo>, // ptr (8) + size (8) + cap(8) = 24 bytes
    /// Total moves played in the game so far (Red = 0, Black = 1, Red's
    /// next = 2, etc.).
    game_ply: u16, // 2 bytes
    /// Palace coordinates of both players' Generals (Kings) for faster check
    /// detection.
    king_squares: [Square; Side::COUNT], // 2 bytes
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
}

impl Position {
    /// Standard Xiangqi starting position in FEN notation.
    pub(crate) const START_FEN: &str =
        "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w";

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
    pub fn hash(&self) -> u64 {
        self.state.zobrist
    }

    /// Get the current side to make a move
    #[inline]
    pub fn side_to_move(&self) -> Side {
        unsafe { std::mem::transmute(self.game_ply as u8 & 1) }
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

    #[inline]
    pub fn piece_type_count(&self, piece_type: PieceType) -> u8 {
        self.piece_count(piece_type.to_piece(Side::Red))
            + self.piece_count(piece_type.to_piece(Side::Black))
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

    /// Checks whether or not [`mv`] is a capture move.
    #[inline]
    pub fn is_capture(&self, mv: Move) -> bool {
        !self.is_empty(mv.to())
    }

    /// Checks whether or not [`mv`] is a quiet move (no capture).
    #[inline]
    pub fn is_quiet(&self, mv: Move) -> bool {
        self.is_empty(mv.to())
    }

    /// Get the square where the king of side [`color`] is currently at
    #[inline]
    pub fn king_square(&self, color: Side) -> Square {
        self.king_squares[color as usize]
    }

    /// Checks if the given player's King is currently in check.
    ///
    /// Mainly used for tests.
    #[inline]
    pub fn is_side_in_check(&self, color: Side) -> bool {
        if color == self.side_to_move() {
            self.state.in_check
        } else {
            self.is_square_attacked(self.king_square(color), color.opposite())
        }
    }

    /// Checks if the player to move is currently in check.
    #[inline]
    pub fn is_in_check(&self) -> bool {
        self.state.in_check
    }

    /// Parses and initializes the position state from a standard FEN string.
    pub fn from_fen(fen: &str) -> Result<Self> {
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
                in_check: false,
                score: PackedScore::ZERO,
                phase: 0,
                checkers: Bitboard::new(),
                blockers_for_king: [Bitboard::new(); Side::COUNT],
                pinners: [Bitboard::new(); Side::COUNT],
                check_squares: [Bitboard::new(); PieceType::COUNT],
                need_full_check: false,
                plies_since_null: 0,
            },
            history: Vec::new(),
            game_ply: 0,
            king_squares: [Square::E0, Square::E9],
        };

        let tokens: Vec<&str> = fen.split_whitespace().collect();
        if tokens.is_empty() {
            bail!("Empty fen");
        }

        // 1. Parse piece placement ranks (10 ranks, separated by '/')
        let ranks: Vec<&str> = tokens[0].split('/').collect();
        if ranks.len() != 10 {
            bail!("Invalid number of ranks, expected 10, got {}", ranks.len());
        }

        for rank in Rank::all().rev() {
            let rank_idx = rank as usize;
            let rank_str = ranks[9 - rank_idx];
            let mut file_idx = 0u8;

            for c in rank_str.chars() {
                if c.is_ascii_digit() {
                    let empty_squares = c.to_digit(10).unwrap() as u8;
                    file_idx += empty_squares;
                } else {
                    if file_idx >= 9 {
                        bail!("File index out of bounds in rank {}", rank_idx);
                    }
                    let file = unsafe { File::from_repr_unchecked(file_idx) };
                    let square = Square::from_file_rank(file, rank);
                    if let Some(piece) = Self::piece_from_char(c) {
                        pos.put_piece(square, Some(piece));
                    } else {
                        bail!("Unknown piece character {}", c);
                    }
                    file_idx += 1;
                }
            }
            if file_idx != 9 {
                bail!(
                    "Invalid number of files for rank {}, expected 9, got {}",
                    rank_idx,
                    file_idx
                );
            }
        }

        // 2. Parse side to move ('w' or 'b')
        let side_to_move = if tokens.len() > 1 {
            match tokens[1] {
                "w" => Side::Red,
                "b" => Side::Black,
                _ => {
                    bail!("Invalid side to move: {}", tokens[1]);
                }
            }
        } else {
            Side::Red
        };

        if side_to_move == Side::Black {
            pos.state.zobrist ^= ZOBRIST.side;
        }

        let rule60 = tokens
            .get(4)
            .and_then(|x| x.parse::<u16>().ok())
            .unwrap_or(0);

        let fullmove = tokens
            .get(5)
            .and_then(|x| x.parse::<u16>().ok())
            .unwrap_or(1);

        pos.game_ply = (fullmove.saturating_sub(1) * 2) + (side_to_move as u16);

        pos.state.sixtymove_clock = rule60;
        pos.state.score = pos.tapered_score_from_scratch();
        pos.state.phase = pos.calculate_board_phase();
        pos.set_check_info();

        Ok(pos)
    }

    /// Prints a human-readable ASCII representation of the board state.
    pub fn print_board(&self) {
        println!("  +---------------------------+");
        for rank in Rank::all().rev() {
            print!("{} |", rank as usize);
            for file in File::all() {
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

    #[test]
    fn test_debug_gives_check() {
        let fen = "CRH1k1e2/3ca4/4ea3/9/2hr5/9/9/4E4/4A4/4KA3 w - - 0 1";
        let mut pos = Position::from_fen(fen).unwrap();

        fn debug_helper(pos: &mut Position, depth: u32, history: &mut Vec<Move>) {
            if depth == 0 {
                return;
            }
            let mut moves = crate::core::MoveList::new();
            crate::core::generate_moves(pos, crate::core::MoveGenType::Legal, &mut moves);

            for &m in &moves {
                let actual = pos.gives_check(m);
                pos.do_move(m);
                let expected = pos.is_side_in_check(pos.side_to_move());
                if actual != expected {
                    let us = pos.side_to_move();
                    let them = us.opposite();
                    let ksq = pos.king_square(them);
                    println!("DEBUG INFO FOR MISMATCH:");
                    println!("Move: {}", m);
                    println!("Side to move: {:?}", us);
                    println!("King square of opponent: {:?}", ksq);
                    println!("Occupied bitboard:\n{}", pos.bitboard_occupied());
                    println!(
                        "Rook check squares:\n{}",
                        pos.state.check_squares[PieceType::Rook as usize]
                    );
                    println!(
                        "Cannon check squares:\n{}",
                        pos.state.check_squares[PieceType::Cannon as usize]
                    );
                    pos.undo_move(m);
                    pos.print_board();
                    let history_strs: Vec<String> =
                        history.iter().map(|mv| format!("{}", mv)).collect();
                    panic!(
                        "Mismatch for move {} with history {:?}:\nactual (gives_check): {}\nexpected (is_in_check): {}",
                        m, history_strs, actual, expected
                    );
                }
                history.push(m);
                debug_helper(pos, depth - 1, history);
                history.pop();
                pos.undo_move(m);
            }
        }

        let mut history = Vec::new();
        debug_helper(&mut pos, 3, &mut history);
    }

    #[test]
    fn verify_gives_check_and_legal_on_all_positions() {
        let txt = include_str!("../../bin/perft_positions.txt");
        let mut fens = Vec::new();
        for segment in txt.split("---") {
            let segment = segment.trim();
            if segment.is_empty() {
                continue;
            }
            for line in segment.lines() {
                let line = line.trim();
                if line.starts_with("fen:") {
                    let fen = line.strip_prefix("fen:").unwrap().trim();
                    fens.push(fen);
                }
            }
        }

        for (idx, fen) in fens.iter().enumerate() {
            let mut pos = Position::from_fen(fen).unwrap();
            let mut history = Vec::new();
            verify_helper(&mut pos, 4, &mut history, fen, idx + 1);
        }
    }

    fn verify_helper(
        pos: &mut Position,
        depth: u32,
        history: &mut Vec<Move>,
        initial_fen: &str,
        pos_idx: usize,
    ) {
        if depth == 0 {
            return;
        }
        let mut moves = crate::core::MoveList::new();
        crate::core::generate_moves(pos, crate::core::MoveGenType::PseudoLegal, &mut moves);

        for &m in &moves {
            let actual_legal = pos.legal(m);

            let us = pos.side_to_move();
            let mut temp_pos = pos.clone();
            temp_pos.do_move(m);
            let expected_legal = !temp_pos.is_side_in_check(us);

            if actual_legal != expected_legal {
                println!("LEGALITY MISMATCH!");
                println!("Initial FEN of Position {}: {}", pos_idx, initial_fen);
                println!("Move: {}", m);
                println!("History: {:?}", history);
                println!(
                    "actual_legal: {}, expected_legal: {}",
                    actual_legal, expected_legal
                );
                pos.print_board();
                panic!("Legality mismatch");
            }

            if actual_legal {
                let actual_check = pos.gives_check(m);
                let expected_check = temp_pos.is_side_in_check(temp_pos.side_to_move());

                if actual_check != expected_check {
                    println!("GIVES_CHECK MISMATCH!");
                    println!("Initial FEN of Position {}: {}", pos_idx, initial_fen);
                    println!("Move: {}", m);
                    println!("History: {:?}", history);
                    println!(
                        "actual_check: {}, expected_check: {}",
                        actual_check, expected_check
                    );
                    pos.print_board();
                    panic!("Gives check mismatch");
                }

                history.push(m);
                verify_helper(&mut temp_pos, depth - 1, history, initial_fen, pos_idx);
                history.pop();
            }
        }
    }

    #[test]
    fn test_regression_cannon_gives_check() {
        // Reproduces gives_check mismatch on Cannon move (a9a8 checking King on e8
        // through d8 blocker)
        let fen = "CRH1k1e2/3ca4/4ea3/9/2hr5/9/9/4E4/4A4/4KA3 w - - 0 1";
        let mut pos = Position::from_fen(fen).unwrap();

        // Red plays c9e8 (Red Knight H captures Black Advisor on e8)
        let m1 = Move::new(Square::C9, Square::E8);
        assert!(pos.legal(m1));
        pos.do_move(m1);

        // Black plays e9e8 (Black King captures Red Knight on e8)
        let m2 = Move::new(Square::E9, Square::E8);
        assert!(pos.legal(m2));
        pos.do_move(m2);

        // Red plays a9a8 (Red Cannon a9 to a8, checking Black King e8 via Black Cannon
        // d8)
        let m3 = Move::new(Square::A9, Square::A8);
        assert!(pos.legal(m3));
        assert!(pos.gives_check(m3), "Cannon move a9a8 should give check");
    }

    #[test]
    fn test_regression_knight_leg_pin_legality() {
        // Reproduces Knight leg blocker pin: Red Cannon at e2 blocks the leg of Black
        // Knight f2 jumping to d1. Moving Red Cannon e2 to a2 (or any
        // non-blocking square) is illegal because it exposes the Red King d1 to check.
        let fen = "4ka3/4a4/9/9/4H4/p8/9/4C3c/7h1/2EK5 w - - 0 1";
        let mut pos = Position::from_fen(fen).unwrap();

        // Setup the specific position by playing quiet/legal moves
        // Red moves Red King from d0 to d1 (d0d1)
        let m1 = Move::new(Square::D0, Square::D1);
        pos.do_move(m1);

        // Black moves Black Knight from h1 to f2 (h1f2)
        let m2 = Move::new(Square::H1, Square::F2);
        pos.do_move(m2);

        // Red tries to move Red Cannon from e2 to a2 (e2a2)
        let m3 = Move::new(Square::E2, Square::A2);
        assert!(
            !pos.legal(m3),
            "e2a2 should be illegal because it unblocks Black Knight leg check"
        );
    }

    #[test]
    fn test_regression_cannon_pin_capture_legality() {
        // Reproduces Cannon pin capture: Red Cannon at e2 pins Black Pawn at e5 and Red
        // Pawn at e4. Black Pawn e5 captures Red Pawn e4. This is illegal
        // because it leaves only 1 blocker (e4), exposing the Black King e9 to
        // check by the Red Cannon e2.
        let fen = "rheakaehr/9/1c5c1/p1p3p1p/4p4/4P4/P1P3P1P/4C2C1/9/RHEAKAEHR b - - 0 1";
        let pos = Position::from_fen(fen).unwrap();

        let m = Move::new(Square::E5, Square::E4);
        assert!(
            !pos.legal(m),
            "e5e4 should be illegal because it leaves only 1 blocker under Cannon pin"
        );
    }

    #[test]
    fn test_null_moves() {
        let mut pos = Position::new();
        let initial_hash = pos.hash();
        let initial_side = pos.side_to_move();
        let initial_ply = pos.game_ply;

        // Verify attacking pieces check
        assert!(pos.has_attacking_pieces(Side::Red));
        assert!(pos.has_attacking_pieces(Side::Black));
        assert_eq!(pos.state.plies_since_null, 0);

        // Make a normal move
        let w_move = Move::new(Square::E0, Square::D0);
        pos.do_move(w_move);
        assert_eq!(pos.state.plies_since_null, 1);

        // Make null move
        pos.do_null_move();
        assert_ne!(pos.hash(), initial_hash);
        assert_eq!(pos.side_to_move(), initial_side);
        assert_eq!(pos.game_ply, initial_ply + 2);
        assert_eq!(pos.state.plies_since_null, 0);

        // Undo null move
        pos.undo_null_move();
        assert_eq!(pos.state.plies_since_null, 1);

        // Undo normal move
        pos.undo_move(w_move);
        assert_eq!(pos.hash(), initial_hash);
        assert_eq!(pos.side_to_move(), initial_side);
        assert_eq!(pos.game_ply, initial_ply);
        assert_eq!(pos.state.plies_since_null, 0);
    }
}
