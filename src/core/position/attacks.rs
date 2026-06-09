use crate::core::{
    Bitboard, Move, Piece, PieceType, Position, Side, Square, cannon_attacks, knight_attacks_to,
    pawn_attacks_to, rook_attacks,
};

impl Position {
    /// Validates if a pseudo-legal move `m` is fully legal (i.e. the King is
    /// not left in check).
    #[inline]
    pub fn legal(&self, m: Move) -> bool {
        let us = self.side_to_move;
        let from = m.from();
        let to = m.to();
        let moved_piece =
            self.board[from as usize].expect("No piece at the source square for legality check");

        let king_sq = if moved_piece.piece_type() == PieceType::King {
            to
        } else {
            self.king_square(us)
        };

        let mut occupied = self.bitboard_by_color[Side::Red as usize]
            | self.bitboard_by_color[Side::Black as usize];
        occupied.clear_bit(from);
        occupied.set_bit(to);

        self.checkers_to_after_move(king_sq, occupied, us.opposite(), from, to, moved_piece)
            .is_empty()
    }

    /// Evaluates if playing the move `m` places the opponent's General in
    /// check. Runs a simulation update of `occupied` bitboards and
    /// calculates checkers pointing to the General.
    #[inline]
    pub fn gives_check(&self, m: Move) -> bool {
        let us = self.side_to_move;
        let them = us.opposite();
        let from = m.from();
        let to = m.to();
        let moved_piece =
            self.board[from as usize].expect("No piece at the source square for gives_check");
        let them_king_sq = self.king_square(them);

        let mut occupied = self.bitboard_by_color[Side::Red as usize]
            | self.bitboard_by_color[Side::Black as usize];
        occupied.clear_bit(from);
        occupied.set_bit(to);

        !self
            .checkers_to_after_move(them_king_sq, occupied, us, from, to, moved_piece)
            .is_empty()
    }

    /// Checks whether a [`square`] is currently being attacked by [`attacker`]
    #[inline]
    pub fn is_square_attacked(&self, square: Square, attacker: Side) -> bool {
        let occupied = self.bitboard_by_color[Side::Red as usize]
            | self.bitboard_by_color[Side::Black as usize];
        !self.checkers_to(square, occupied, attacker).is_empty()
    }

    /// Checks whether a [`square`] is currently being attacked by [`attacker`]
    /// after doing a move
    #[inline]
    pub fn is_square_attacked_after_move(
        &self,
        square: Square,
        attacker: Side,
        from: Square,
        to: Square,
        moved_piece: Piece,
    ) -> bool {
        let mut occupied = self.bitboard_by_color[Side::Red as usize]
            | self.bitboard_by_color[Side::Black as usize];
        occupied.clear_bit(from);
        occupied.set_bit(to);

        !self
            .checkers_to_after_move(square, occupied, attacker, from, to, moved_piece)
            .is_empty()
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
        let opponent_pawns = self.bitboard_of(attacker, PieceType::Pawn);
        let opponent_knights = self.bitboard_of(attacker, PieceType::Knight);
        let opponent_rooks = self.bitboard_of(attacker, PieceType::Rook);
        let opponent_cannons = self.bitboard_of(attacker, PieceType::Cannon);
        let opponent_king = self.bitboard_of(attacker, PieceType::King);

        let pawn_attackers = pawn_attacks_to(square, attacker) & opponent_pawns;
        let knight_attackers = knight_attacks_to(square, occupied) & opponent_knights;
        let rook_atk = rook_attacks(square, occupied);

        // Intersect with attacker Rooks AND the attacker King: under the Flying General
        // rule, two Kings facing each other on an open file counts as a check
        // (treated as a Rook attack).
        let rook_attackers = rook_atk & (opponent_rooks | opponent_king);

        // --- Cannon scanner (platform-leap captures) ---
        // Compute Cannon attack squares from `square`: Cannons capture by leaping over
        // exactly one intervening piece (the "platform"). `cannon_attacks`
        // returns squares that have exactly one piece between them and `square`
        // along a rank or file.
        let cannon_atk = cannon_attacks(square, occupied);
        // Intersect with attacker Cannons only (Kings/Rooks cannot leap over
        // platforms).
        let cannon_attackers = cannon_atk & opponent_cannons;

        // Union all four attacker bitboards into a single result: every square occupied
        // by an attacker-colored piece that can reach `square` under the
        // current board occupancy.
        pawn_attackers | knight_attackers | rook_attackers | cannon_attackers
    }

    /// Evaluates backward checkers attacking `square` after simulating a
    /// specific piece move. Overrides positions without modifying active
    /// board structures.
    #[inline]
    pub(super) fn checkers_to_after_move(
        &self,
        square: Square,
        occupied: Bitboard,
        attacker: Side,
        from: Square,
        to: Square,
        moved_piece: Piece,
    ) -> Bitboard {
        let captured = self.board[to as usize];
        let mut opponent_pawns = self.bitboard_of(attacker, PieceType::Pawn);
        let mut opponent_knights = self.bitboard_of(attacker, PieceType::Knight);
        let mut opponent_rooks = self.bitboard_of(attacker, PieceType::Rook);
        let mut opponent_cannons = self.bitboard_of(attacker, PieceType::Cannon);
        let mut opponent_king = self.bitboard_of(attacker, PieceType::King);

        if let Some(captured) = captured
            && captured.color() == attacker
        {
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

        let moved_by_attacker = moved_piece.color() == attacker;
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

        let pawn_attackers = pawn_attacks_to(square, attacker) & opponent_pawns;
        let knight_attackers = knight_attacks_to(square, occupied) & opponent_knights;
        let rook_atk = rook_attacks(square, occupied);
        let rook_attackers = rook_atk & (opponent_rooks | opponent_king);
        let cannon_atk = cannon_attacks(square, occupied);
        let cannon_attackers = cannon_atk & opponent_cannons;

        pawn_attackers | knight_attackers | rook_attackers | cannon_attackers
    }
}
