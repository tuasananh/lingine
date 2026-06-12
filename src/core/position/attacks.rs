use crate::core::{
    Bitboard, Move, PieceType, Position, Side, Square, cannon_captures, knight_attacks_to,
    movegen::squares_beyond, pawn_attacks_to, rook_attacks,
};

impl Position {
    /// Helper to check if three squares are orthogonally aligned.
    #[inline]
    fn aligned(&self, s1: Square, s2: Square, s3: Square) -> bool {
        let f1 = s1.file() as u8;
        let f2 = s2.file() as u8;
        let f3 = s3.file() as u8;
        if f1 == f2 && f2 == f3 {
            return true;
        }
        let r1 = s1.rank() as u8;
        let r2 = s2.rank() as u8;
        let r3 = s3.rank() as u8;
        if r1 == r2 && r2 == r3 {
            return true;
        }
        false
    }

    /// Validates if a pseudo-legal move `m` is fully legal (i.e. the King is
    /// not left in check).
    #[inline]
    pub fn legal(&self, m: Move) -> bool {
        let us = self.side_to_move();
        let from = m.from();
        let to = m.to();
        let moved_piece =
            self.board[from as usize].expect("No piece at the source square for legality check");
        let pt = moved_piece.piece_type();

        let occupied = (self.bitboard_occupied() ^ Bitboard::from(from)) | Bitboard::from(to);

        // If the moving piece is a King, check whether the destination square is
        // attacked by opponent
        if pt == PieceType::King {
            return self.checkers_to(to, occupied, us.opposite()).is_empty();
        }

        // If we don't need full check, the move is legal under the fast path:
        if !self.state.need_full_check {
            // A non-king move is legal if the piece is not pinned (blocker) OR:
            // - it is not a Cannon, or it is a Cannon but not a capture move
            // - and the move is aligned with the King
            if !self.state.blockers_for_king[us as usize].contains(from)
                || ((pt != PieceType::Cannon || !self.is_capture(m))
                    && self.aligned(from, to, self.king_square(us)))
            {
                return true;
            }
        }

        // Otherwise, run the fallback check: King must not be attacked after the move
        (self.checkers_to(self.king_square(us), occupied, us.opposite()) & !Bitboard::from(to))
            .is_empty()
    }

    /// Evaluates if playing the move `m` places the opponent's General in
    /// check.
    #[inline]
    pub fn gives_check(&self, m: Move) -> bool {
        let us = self.side_to_move();
        let them = us.opposite();
        let from = m.from();
        let to = m.to();
        let moved_piece =
            self.board[from as usize].expect("No piece at the source square for gives_check");
        let pt = moved_piece.piece_type();
        let ksq = self.king_square(them);

        // Direct check?
        if pt == PieceType::Cannon
            // The space between the cannon and the king is empty
            && self.state.check_squares[PieceType::Rook as usize].contains(from)
            // The movement is aligned in the same rank or file
            && self.aligned(from, to, ksq)
        {
            // The move is a capture and the cannon is moving away from the king
            // For example: (victim) .. X .. C .. K -> C .. X .. K -> Check!
            if self.is_capture(m) && squares_beyond(ksq, from).contains(to) {
                return true;
            }
        } else if self.state.check_squares[pt as usize].contains(to) {
            return true;
        }

        // Discovered check?
        // The move can be a discovered check if:
        // - The piece moved is a blocker
        // - And, either:
        //   - It moves out of the file/rank (this is obvious)
        //   - It stays in the same file/rank, but it captures another piece. This is
        //     because we can
        //   have something like C .. B1 .. B2 .. K -> B1 captures B2 and then we have a
        // check!
        if self.state.blockers_for_king[them as usize].contains(from)
            && (!self.aligned(from, to, ksq) || self.is_capture(m))
        {
            return true;
        }

        false
    }

    /// Checks whether a [`square`] is currently being attacked by [`attacker`]
    #[inline]
    pub fn is_square_attacked(&self, square: Square, attacker: Side) -> bool {
        let occupied = self.bitboard_by_color[Side::Red as usize]
            | self.bitboard_by_color[Side::Black as usize];
        !self.checkers_to(square, occupied, attacker).is_empty()
    }

    /// Identifies all opponent pieces of `attacker` color that attack the
    /// target `square` assuming the given board `occupied` bitboard.
    #[inline]
    pub(super) fn checkers_to(
        &self,
        square: Square,
        occupied: Bitboard,
        attacker: Side,
    ) -> Bitboard {
        let pawns = self.bitboard_by_type(PieceType::Pawn);
        let knights = self.bitboard_by_type(PieceType::Knight);
        let rooks = self.bitboard_by_type(PieceType::Rook);
        let cannons = self.bitboard_by_type(PieceType::Cannon);
        let king = self.bitboard_by_type(PieceType::King);
        let pawn_attackers = pawn_attacks_to(square, attacker) & pawns;
        let knight_attackers = knight_attacks_to(square, occupied) & knights;
        // Intersect with Rooks AND the King: under the Flying General rule, two Kings
        // facing each other on an open file counts as a check (treated as a
        // Rook attack).
        let rook_attackers = rook_attacks(square, occupied) & (rooks | king);
        let cannon_attackers = cannon_captures(square, occupied) & cannons;

        (pawn_attackers | knight_attackers | rook_attackers | cannon_attackers)
            & self.bitboard_by_color(attacker)
    }
}
