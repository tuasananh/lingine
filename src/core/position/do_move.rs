use crate::core::{Move, Piece, PieceType, Position, Side, Square, StateInfo, position::ZOBRIST};

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
        let sixtymove_clock = last_state.sixtymove_clock;
        let rule_repetition = last_state.rule_repetition;
        let old_zobrist = self.zobrist_hash;

        let mut mg_score = last_state.mg_score;
        let mut eg_score = last_state.eg_score;
        let mut phase = last_state.phase;

        let piece = piece.expect("Cannot do_move with no piece at the source square");

        let from_val = crate::eval::piece_material_value_tapered(piece, from);
        let to_val = crate::eval::piece_material_value_tapered(piece, to);
        let from_pst =
            crate::eval::piece_square_table_value_tapered(piece.piece_type(), piece.color(), from);
        let to_pst =
            crate::eval::piece_square_table_value_tapered(piece.piece_type(), piece.color(), to);

        match piece.color() {
            Side::Red => {
                mg_score = mg_score - from_val.mg - from_pst.mg + to_val.mg + to_pst.mg;
                eg_score = eg_score - from_val.eg - from_pst.eg + to_val.eg + to_pst.eg;
            }
            Side::Black => {
                mg_score = mg_score + from_val.mg + from_pst.mg - to_val.mg - to_pst.mg;
                eg_score = eg_score + from_val.eg + from_pst.eg - to_val.eg - to_pst.eg;
            }
        }

        if let Some(cap) = captured {
            let cap_val = crate::eval::piece_material_value_tapered(cap, to);
            let cap_pst =
                crate::eval::piece_square_table_value_tapered(cap.piece_type(), cap.color(), to);
            match cap.color() {
                Side::Red => {
                    mg_score -= cap_val.mg + cap_pst.mg;
                    eg_score -= cap_val.eg + cap_pst.eg;
                }
                Side::Black => {
                    mg_score += cap_val.mg + cap_pst.mg;
                    eg_score += cap_val.eg + cap_pst.eg;
                }
            }
            phase -= crate::eval::piece_phase_weight(cap.piece_type());
        }

        self.put_piece(from, None);
        self.put_piece(to, Some(piece));

        // Update rule60 halfmove clock
        let new_rule60 = if piece.piece_type() == PieceType::Pawn || captured.is_some() {
            0
        } else {
            sixtymove_clock + 1
        };

        let is_pawn_push = piece.piece_type() == PieceType::Pawn && to.rank() != from.rank();
        let is_irreversible = is_pawn_push || captured.is_some();
        let new_rule_repetition = if is_irreversible {
            0
        } else {
            rule_repetition + 1
        };

        let in_check = [self.is_in_check(Side::Red), self.is_in_check(Side::Black)];

        // Push current state onto history stack
        self.history.push(StateInfo {
            last_move: m,
            captured_piece: captured,
            old_zobrist,
            sixtymove_clock: new_rule60,
            rule_repetition: new_rule_repetition,
            in_check,
            mg_score,
            eg_score,
            phase,
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
