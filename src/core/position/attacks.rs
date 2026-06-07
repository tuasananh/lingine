use crate::core::{
    Bitboard, KNIGHT_TO_TABLE, Move, PAWN_ATTACKS_TO, Piece, PieceType, Position, Side, Square,
    cannon_attacks, rook_attacks,
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
    ///
    /// This represents an extremely optimized, loop-free backward bitwise
    /// scanner. Rather than generating all moves for all opponent pieces,
    /// we shoot out rays and leaps backwards starting from `square`:
    ///
    /// 1. **Pawn Scanner**: Traces reverse Pawn attack positions using static
    ///    `PAWN_ATTACKS_TO`.
    /// 2. **Knight Scanner**: Gathers the 6 unique Horse Legs around `square`
    ///    into a 6-bit occupancy mask, looking up valid attack origin positions
    ///    in `KNIGHT_TO_TABLE`.
    /// 3. **Rook & King Scanner**: Traces orthogonal lines using precomputed
    ///    sliding rank/file masks. Under the "Flying General" rule in Xiangqi,
    ///    Kings cannot face each other directly along a file without
    ///    intervening pieces; this direct face is treated as a Rook check.
    /// 4. **Cannon Scanner**: Traces split rank/file leap capture paths using
    ///    precomputed Cannon tables.
    #[inline]
    pub(super) fn checkers_to(
        &self,
        square: Square,
        occupied: Bitboard,
        attacker: Side,
    ) -> Bitboard {
        // --- Isolate attacker's piece bitboards by intersecting piece-type and color
        // masks --- Each variable holds bits only for attacker-colored pieces
        // of that type.
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
        // White and Black Pawns attack in opposite directions, so each has its own
        // reverse-attack table.
        let them_color_idx = if attacker == Side::Red { 0 } else { 1 };
        // `PAWN_ATTACKS_TO[color][sq]` gives the set of squares FROM which a Pawn of
        // that color could have attacked `square`. AND with actual Pawn
        // positions to find real attackers.
        let pawn_attackers = PAWN_ATTACKS_TO[them_color_idx][square as usize] & opponent_pawns;

        // --- Knight scanner (Horse-Leg / blocking-pin aware) ---
        // Look up the precomputed entry for `square` in the reverse Knight attack
        // table. Each entry stores up to 6 "eye" squares (the leg-blocking
        // squares around `square`) and a 64-entry array of attack masks indexed
        // by a 6-bit occupancy key.
        let entry = &KNIGHT_TO_TABLE[square as usize];
        let mut occ_idx = 0; // will become a 6-bit mask of which eye squares are occupied
        let mut i = 0;
        while i < 6 {
            // For each potential eye square (the square a Knight must pass through on its
            // L-move)...
            if let Some(eye_sq) = entry.eyes[i] {
                // ...set bit `i` in occ_idx if that eye square is currently occupied (leg is
                // blocked).
                if occupied.is_occupied(eye_sq) {
                    occ_idx |= 1 << i;
                }
            }
            i += 1;
        }
        // Use the 6-bit occupancy key to look up which Knights can actually reach
        // `square` (only those whose leg is NOT blocked), then intersect with
        // real Knight positions.
        let knight_attackers = entry.attacks[occ_idx] & opponent_knights;

        // --- Rook & King scanner (orthogonal sliding rays + Flying General rule) ---
        // Compute all squares reachable by a Rook standing on `square` given
        // `occupied`. A Rook on the target square sees exactly the squares that
        // can send a Rook check.
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

        let them_color_idx = if attacker == Side::Red { 0 } else { 1 };
        let pawn_attackers = PAWN_ATTACKS_TO[them_color_idx][square as usize] & opponent_pawns;

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
        let knight_attackers = entry.attacks[occ_idx] & opponent_knights;

        let rook_atk = rook_attacks(square, occupied);
        let rook_attackers = rook_atk & (opponent_rooks | opponent_king);

        let cannon_atk = cannon_attacks(square, occupied);
        let cannon_attackers = cannon_atk & opponent_cannons;

        pawn_attackers | knight_attackers | rook_attackers | cannon_attackers
    }
}
