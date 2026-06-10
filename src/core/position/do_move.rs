use crate::{
    core::{Move, Piece, PieceType, Position, Side, Square, position::ZOBRIST},
    eval::{piece_material_value_tapered, piece_phase_weight, piece_square_table_value_tapered},
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
            self.state.zobrist ^= ZOBRIST.pieces[piece as usize][square as usize];
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

            self.state.zobrist ^= ZOBRIST.pieces[piece as usize][square as usize];
        }
    }

    /// Plays a move on the board, saving prior ply parameter states onto the
    /// stack to support fast undo restores, and toggles active side.
    #[inline]
    pub fn do_move(&mut self, m: Move) {
        self.game_ply += 1;
        self.history.push(self.state);
        self.state.zobrist ^= ZOBRIST.side;

        let from = m.from();
        let to = m.to();
        let piece =
            self.board[from as usize].expect("Cannot do_move with no piece at the source square");
        let captured = self.board[to as usize];

        let from_val = piece_material_value_tapered(piece, from);
        let to_val = piece_material_value_tapered(piece, to);
        let from_pst = piece_square_table_value_tapered(piece.piece_type(), piece.color(), from);
        let to_pst = piece_square_table_value_tapered(piece.piece_type(), piece.color(), to);

        let sgn = piece.color().sign();

        self.state.mg_score += sgn * (-from_val.mg - from_pst.mg + to_val.mg + to_pst.mg);
        self.state.eg_score += sgn * (-from_val.eg - from_pst.eg + to_val.eg + to_pst.eg);

        if let Some(cap) = captured {
            let cap_val = piece_material_value_tapered(cap, to);
            let cap_pst = piece_square_table_value_tapered(cap.piece_type(), cap.color(), to);
            let sgn = cap.color().sign();

            self.state.mg_score -= sgn * (cap_val.mg + cap_pst.mg);
            self.state.eg_score -= sgn * (cap_val.eg + cap_pst.eg);

            self.state.phase -= piece_phase_weight(cap.piece_type());
        }

        self.put_piece(from, None);
        self.put_piece(to, Some(piece));

        // Update rule60 halfmove clock
        self.state.sixtymove_clock = if piece.piece_type() == PieceType::Pawn || captured.is_some()
        {
            0
        } else {
            self.state.sixtymove_clock + 1
        };

        self.state.in_check = [self.is_in_check(Side::Red), self.is_in_check(Side::Black)];
        self.state.captured_piece = captured;
        self.state.last_move = Some(m);
    }

    /// Restores the position to the exact state before the last move was
    /// played, popping details off the stack and re-toggling side to move.
    #[inline]
    pub fn undo_move(&mut self, m: Move) {
        let from = m.from();
        let to = m.to();
        let piece = self.board[to as usize]
            .expect("Cannot undo_move with no piece at the destination square");

        self.put_piece(to, self.state.captured_piece);
        self.put_piece(from, Some(piece));

        let state = self.history.pop().expect("No state in history to undo");
        self.state = state;
        self.game_ply -= 1;
    }
}
