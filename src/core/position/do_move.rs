use strum::EnumCount;

use crate::{
    core::{
        BETWEEN_BB, Bitboard, Move, Piece, PieceType, Position, Side, Square, cannon_attack_ray,
        knight_attacks_to, pawn_attacks_to, position::ZOBRIST, rook_attacks,
    },
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

        // Update rule60 halfmove clock. In Xiangqi, unlike chess, pawn moves
        // (especially sideways pawn moves after crossing the river) are reversible
        // and do not reset the 60-move rule counter. Thus, the clock only resets on
        // captures.
        self.state.sixtymove_clock = if captured.is_some() {
            0
        } else {
            self.state.sixtymove_clock + 1
        };

        self.set_check_info();
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

    /// Update blockers and pinners for king of color `c`
    fn update_blockers(&mut self, c: Side) {
        let ksq = self.king_square(c);
        let us = c;
        let them = c.opposite();

        self.state.blockers_for_king[us as usize] = Bitboard::new();
        self.state.pinners[them as usize] = Bitboard::new();

        let occupied = self.bitboard_occupied();
        let opponent_sliders = self.bitboard_of(them, PieceType::Rook)
            | self.bitboard_of(them, PieceType::Cannon)
            | self.bitboard_of(them, PieceType::King);
        let opponent_knights = self.bitboard_of(them, PieceType::Knight);

        // Empty board Rook attacks from ksq
        let empty_board_rook_attacks = rook_attacks(ksq, Bitboard::new());
        // Empty board Knight attacks to ksq
        let empty_board_knight_attacks = knight_attacks_to(ksq, Bitboard::new());

        let mut snipers = (empty_board_rook_attacks & opponent_sliders)
            | (empty_board_knight_attacks & opponent_knights);

        let occupancy = occupied ^ (snipers & !self.bitboard_of(them, PieceType::Cannon));

        while let Some(sniper_sq) = snipers.pop_lsb() {
            let is_cannon =
                self.board[sniper_sq as usize].unwrap().piece_type() == PieceType::Cannon;
            let b = BETWEEN_BB[ksq as usize][sniper_sq as usize]
                & if is_cannon {
                    occupied ^ Bitboard::from(sniper_sq)
                } else {
                    occupancy
                };
            let count = b.count_ones();
            if (!is_cannon && count == 1) || (is_cannon && count == 2) {
                self.state.blockers_for_king[us as usize] |= b;
                if !(b & self.bitboard_by_color(us)).is_empty() {
                    self.state.pinners[them as usize].set_bit(sniper_sq);
                }
            }
        }
    }

    /// Precalculate and store check-giving squares and blocker information for the current state.
    pub(super) fn set_check_info(&mut self) {
        let us = self.side_to_move();
        let them = us.opposite();

        self.update_blockers(Side::Red);
        self.update_blockers(Side::Black);

        let ksq = self.king_square(them);
        let occupied = self.bitboard_occupied();

        // checkers
        self.state.checkers = self.checkers_to(self.king_square(us), occupied, them);

        self.state.in_check = !self.state.checkers.is_empty();

        self.state.need_full_check = !self.state.checkers.is_empty()
            || !(rook_attacks(self.king_square(us), Bitboard::new())
                & self.bitboard_of(them, PieceType::Cannon))
            .is_empty();

        self.state.check_squares[PieceType::Pawn as usize] = pawn_attacks_to(ksq, us);
        self.state.check_squares[PieceType::Knight as usize] = knight_attacks_to(ksq, occupied);
        self.state.check_squares[PieceType::Cannon as usize] = cannon_attack_ray(ksq, occupied);
        self.state.check_squares[PieceType::Rook as usize] = rook_attacks(ksq, occupied);

        self.state.check_squares[PieceType::King as usize] = Bitboard::new();
        self.state.check_squares[PieceType::Advisor as usize] = Bitboard::new();
        self.state.check_squares[PieceType::Bishop as usize] = Bitboard::new();

        // hollow cannons
        let mut hollow_cannons = self.state.check_squares[PieceType::Rook as usize]
            & self.bitboard_of(us, PieceType::Cannon);
        if !hollow_cannons.is_empty() {
            let mut hollow_cannon_discover = Bitboard::new();
            while let Some(cannon_sq) = hollow_cannons.pop_lsb() {
                hollow_cannon_discover |= BETWEEN_BB[cannon_sq as usize][ksq as usize];
            }
            for pt in 0..PieceType::COUNT {
                if pt != PieceType::King as usize {
                    self.state.check_squares[pt] |= hollow_cannon_discover;
                }
            }
        }
    }
}
