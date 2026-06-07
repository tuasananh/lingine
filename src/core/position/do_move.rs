use crate::{
    core::{Color, Move, Piece, PieceType, Position, Square, StateInfo, position::ZOBRIST},
    eval::{piece_material_value, piece_square_table_value},
};

impl Position {
    /// Safely introduces a piece onto a board square, updating the type/color
    /// bitboards, King Palace trackers, and XORing its random signature
    /// into the Zobrist hash.
    #[inline]
    pub fn put_piece(&mut self, square: Square, piece: Option<Piece>) {
        let square_idx = square as usize;
        // Clear the old piece
        if let Some(piece) = self.board[square_idx] {
            let pt = piece.piece_type();
            let color_idx = piece.color() as usize;
            self.bitboard_by_type[pt as usize].clear_bit(square);
            self.bitboard_by_color[color_idx].clear_bit(square);
            self.piece_count[piece as usize] -= 1;
            self.zobrist_hash ^= ZOBRIST.pieces[piece as usize][square as usize];
        }
        self.board[square as usize] = piece;
        if let Some(piece) = piece {
            let pt = piece.piece_type();
            let color_idx = piece.color() as usize;
            self.bitboard_by_type[pt as usize].set_bit(square);
            self.bitboard_by_color[color_idx].set_bit(square);
            self.piece_count[piece as usize] += 1;

            if pt == PieceType::King {
                self.king_squares[color_idx] = square;
            }

            self.zobrist_hash ^= ZOBRIST.pieces[piece as usize][square as usize];
        }
    }

    /// Plays a move on the board, saving prior ply parameter states onto the
    /// stack to support fast undo restores, and toggles active side.
    #[inline]
    pub fn do_move(&mut self, m: Move) {
        let from = m.from();
        let to = m.to();
        let piece = self.board[from as usize];
        let captured = self.board[to as usize];

        let last_state = self.history.last().expect("History stack is empty");
        let rule60 = last_state.rule60;
        let old_zobrist = self.zobrist_hash;
        let mut material_score = last_state.material_score;
        let mut piece_square_table_score = last_state.piece_square_table_score;

        let piece = piece.expect("Cannot do_move with no piece at the source square");

        match piece.color() {
            Color::White => {
                material_score -= piece_material_value(piece, from);
                piece_square_table_score -=
                    piece_square_table_value(piece.piece_type(), Color::White, from);
                material_score += piece_material_value(piece, to);
                piece_square_table_score +=
                    piece_square_table_value(piece.piece_type(), Color::White, to);
            }
            Color::Black => {
                material_score += piece_material_value(piece, from);
                piece_square_table_score +=
                    piece_square_table_value(piece.piece_type(), Color::Black, from);

                material_score -= piece_material_value(piece, to);
                piece_square_table_score -=
                    piece_square_table_value(piece.piece_type(), Color::Black, to);
            }
        }

        // 2. Remove captured piece from to (if any)
        if let Some(piece) = captured {
            match piece.color() {
                Color::White => {
                    material_score -= piece_material_value(piece, to);
                    piece_square_table_score -=
                        piece_square_table_value(piece.piece_type(), Color::White, to);
                }
                Color::Black => {
                    material_score += piece_material_value(piece, to);
                    piece_square_table_score +=
                        piece_square_table_value(piece.piece_type(), Color::Black, to);
                }
            }
        }

        self.put_piece(from, None);
        self.put_piece(to, Some(piece));

        // Update rule60 halfmove clock
        let new_rule60 = if piece.piece_type() == PieceType::Pawn || captured.is_some() {
            0
        } else {
            rule60 + 1
        };
        let in_check = [
            self.is_in_check(Color::White),
            self.is_in_check(Color::Black),
        ];

        // Push current state onto history stack
        self.history.push(StateInfo {
            last_move: m,
            captured_piece: captured,
            old_zobrist,
            rule60: new_rule60,
            in_check,
            material_score,
            piece_square_table_score,
        });

        // Toggle side to move
        self.zobrist_hash ^= ZOBRIST.side;
        self.side_to_move = self.side_to_move.opposite();
        self.game_ply += 1;
    }

    /// Restores the position to the exact state before the last move was
    /// played, popping details off the stack and re-toggling side to move.
    #[inline]
    pub fn undo_move(&mut self) {
        let state = self.history.pop().expect("No state in history to undo");
        let m = state.last_move;
        let from = m.from();
        let to = m.to();
        let piece = self.board[to as usize];

        self.put_piece(to, state.captured_piece);
        self.put_piece(from, piece);

        self.zobrist_hash = state.old_zobrist;
        self.side_to_move = self.side_to_move.opposite();
        self.game_ply -= 1;
    }
}
