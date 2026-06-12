use strum::EnumCount;

use crate::core::{
    Bitboard, Move, MoveGenType, MoveList, Piece, PieceType, Score, Side, Square, cannon_captures,
    generate_moves, knight_attacks, pawn_attacks, rook_attacks, score,
};

impl super::Position {
    /// Calculates the chase information for a given color, returning a 16-bit
    /// mask of chased pieces.
    pub fn chased(&mut self, mover: Side, id_board: &[u8; Square::COUNT]) -> u16 {
        let mut chase = 0u16;
        let opponent = mover.opposite();
        let occupied = self.bitboard_by_color[Side::Red as usize]
            | self.bitboard_by_color[Side::Black as usize];

        // 1. Target pieces that can be chased (excluding King):
        // Rooks, Cannons, Knights, Advisors, Bishops of the opponent,
        // and crossed-river Pawns of the opponent.
        let mut targets_mask = self.bitboard_by_type[PieceType::Rook as usize]
            | self.bitboard_by_type[PieceType::Cannon as usize]
            | self.bitboard_by_type[PieceType::Knight as usize]
            | self.bitboard_by_type[PieceType::Advisor as usize]
            | self.bitboard_by_type[PieceType::Bishop as usize];

        // Add crossed-river pawns of the opponent
        let opp_pawns = self.bitboard_by_type[PieceType::Pawn as usize]
            & self.bitboard_by_color[opponent as usize];
        let opp_side_mask = Bitboard::side(mover); // The river-crossed zone is mover's side!
        targets_mask |= opp_pawns & opp_side_mask;

        // Filter targets to only include the opponent's pieces
        let targets = targets_mask & self.bitboard_by_color[opponent as usize];

        // 2. Chasing attackers:
        // Rooks, Cannons, Knights, and crossed-river Pawns of the mover.
        let mut attackers_mask = self.bitboard_by_type[PieceType::Rook as usize]
            | self.bitboard_by_type[PieceType::Cannon as usize]
            | self.bitboard_by_type[PieceType::Knight as usize];

        let my_pawns = self.bitboard_by_type[PieceType::Pawn as usize]
            & self.bitboard_by_color[mover as usize];
        let my_side_mask = Bitboard::side(opponent); // river-crossed zone is opponent's side!
        attackers_mask |= my_pawns & my_side_mask;

        // Filter attackers to only include mover's pieces
        let mut attackers = attackers_mask & self.bitboard_by_color[mover as usize];

        // 3. Scan all attackers to see if they attack any target
        while let Some(from) = attackers.pop_lsb() {
            let piece = self.board[from as usize]
                .expect("Attacker bitboard has no piece at the given square");
            let ptype = piece.piece_type();

            // Generate attacks from `from`
            let mut attacks = match ptype {
                PieceType::Rook => rook_attacks(from, occupied),
                PieceType::Cannon => cannon_captures(from, occupied),
                PieceType::Knight => knight_attacks(from, occupied),
                PieceType::Pawn => pawn_attacks(from, mover),
                _ => Bitboard::new(),
            };

            // Restrict attacks to only target the opponent's pieces that are valid targets
            attacks &= targets;

            while let Some(to) = attacks.pop_lsb() {
                // Verify if the move is legal (meaning the king of mover is not in check after
                // the move)
                if self.legal(Move::new(from, to)) {
                    let target_piece = self.board[to as usize]
                        .expect("Attack square must have a piece since it's in targets_mask");
                    let target_ptype = target_piece.piece_type();

                    // Relative value rules:

                    // Rule A: Attacks against stronger pieces
                    // Knight or Cannon attacking Rook -> chase

                    if target_ptype == PieceType::Rook
                        && (ptype == PieceType::Knight || ptype == PieceType::Cannon)
                    {
                        chase |= 1 << id_board[to as usize];
                        continue;
                    }

                    // Rule B: Attacks against potentially unprotected pieces
                    let mut true_chase = true;
                    let saved_ply = self.game_ply;

                    // Play move:
                    self.board[from as usize] = None;
                    self.board[to as usize] = Some(piece);

                    // Update bitboards
                    self.bitboard_by_color[mover as usize].clear_bit(from);
                    self.bitboard_by_color[mover as usize].set_bit(to);
                    self.bitboard_by_color[opponent as usize].clear_bit(to);

                    self.bitboard_by_type[ptype as usize].clear_bit(from);
                    self.bitboard_by_type[ptype as usize].set_bit(to);
                    self.bitboard_by_type[target_ptype as usize].clear_bit(to);

                    // We temporarily toggle side_to_move to opponent
                    self.game_ply ^= 1;

                    // Now see if any of the opponent's pieces can legally recapture at `to`
                    let recaptured_occupied = self.bitboard_by_color[Side::Red as usize]
                        | self.bitboard_by_color[Side::Black as usize];

                    let mut recapturers = self.checkers_to(to, recaptured_occupied, opponent);
                    while let Some(s) = recapturers.pop_lsb() {
                        if self.legal(Move::new(s, to)) {
                            true_chase = false;
                            break;
                        }
                    }

                    // Restore board and bitboards:
                    self.board[from as usize] = Some(piece);
                    self.board[to as usize] = Some(target_piece);

                    self.bitboard_by_color[mover as usize].set_bit(from);
                    self.bitboard_by_color[mover as usize].clear_bit(to);
                    self.bitboard_by_color[opponent as usize].set_bit(to);

                    self.bitboard_by_type[ptype as usize].set_bit(from);
                    self.bitboard_by_type[ptype as usize].clear_bit(to);
                    self.bitboard_by_type[target_ptype as usize].set_bit(to);

                    self.game_ply = saved_ply;

                    if true_chase {
                        // Exclude mutual/symmetric attacks except pins
                        if ptype == target_ptype {
                            // If same type (e.g. Rook attacking Rook):
                            // Check if the opponent's piece cannot legally capture back.
                            self.game_ply ^= 1;
                            let can_opp_capture_back = self.legal(Move::new(to, from));
                            self.game_ply = saved_ply;

                            if !can_opp_capture_back {
                                chase |= 1 << id_board[to as usize];
                            }
                        } else {
                            chase |= 1 << id_board[to as usize];
                        }
                    }
                }
            }
        }

        chase
    }

    /// Detects chases from state st - d to state st on a rollback clone of
    /// self.
    pub fn detect_chases(&mut self, d: usize, ply: u8) -> Score {
        let n = self.history.len();
        if n < d {
            return score::ZERO; // Draw
        }

        // Grant each piece on board a unique ID for each side
        let mut white_id = 0;
        let mut black_id = 0;
        let mut id_board = [0u8; Square::COUNT];
        for sq_idx in 0..Square::COUNT {
            let sq = Square::from_repr(sq_idx as u8).unwrap();
            if let Some(piece) = self.board[sq as usize] {
                if piece.color() == Side::Red {
                    id_board[sq as usize] = white_id;
                    white_id += 1;
                } else {
                    id_board[sq as usize] = black_id;
                    black_id += 1;
                }
            }
        }

        let us = self.side_to_move();
        let opponent = us.opposite();

        // Rollback until we reached st - d
        let mut chase = [0xFFFFu16, 0xFFFFu16];

        for _ in 0..d {
            if self.state.in_check {
                return score::DRAW; // Draw
            }

            let opposing_chase_mask = chase[self.side_to_move().opposite() as usize];
            if opposing_chase_mask == 0 {
                let our_chase_mask = chase[self.side_to_move() as usize];
                if our_chase_mask == 0 {
                    break;
                }

                // Just undo move without computing chase diff
                let m = self
                    .state
                    .last_move
                    .expect("Rollback state must have a last move");
                self.undo_move(m);
            } else {
                let mover = self.side_to_move();
                let after = self.chased(mover.opposite(), &id_board);
                let m = self
                    .state
                    .last_move
                    .expect("Rollback state must have a last move");
                self.undo_move(m);
                let before = self.chased(self.side_to_move(), &id_board);

                chase[self.side_to_move() as usize] &= after & !before;
            }
        }

        let us_chasing = chase[us as usize] != 0;
        let them_chasing = chase[opponent as usize] != 0;

        if us_chasing && them_chasing {
            score::DRAW // Mutual chase -> draw
        } else if us_chasing {
            score::mated_in(ply) // We perpetually chase -> we lose
        } else if them_chasing {
            score::mate_in(ply) // Opponent perpetually chases -> we win
        } else {
            score::DRAW // Normal draw
        }
    }

    /// Evaluates if the game has ended due to 60-move rule, insufficient
    /// material, or loops (normal draws, perpetual checking, or perpetual
    /// chasing).
    ///
    /// This function is based on Pikafish's implementation
    pub fn rule_judge(&mut self, ply: u8) -> Option<Score> {
        // 1. 60-Move Rule (120 Plies since last pawn advance or capture)
        let rule60 = self.state.sixtymove_clock;
        const RULE60_PLIES_THRESHOLD: u16 = 120;
        if rule60 >= RULE60_PLIES_THRESHOLD {
            return Some(score::DRAW);
        }

        // 2. Insufficient Material Draw
        // If all Pawns are gone, check if remaining major pieces are capable of
        // checkmating
        if self.piece_count(Piece::RedPawn) + self.piece_count(Piece::BlackPawn) == 0 {
            let red_cannon = self.piece_count(Piece::RedCannon);
            let red_majors =
                self.piece_count(Piece::RedRook) + red_cannon + self.piece_count(Piece::RedKnight);
            let black_cannon = self.piece_count(Piece::BlackCannon);
            let black_majors = self.piece_count(Piece::BlackRook)
                + black_cannon
                + self.piece_count(Piece::BlackKnight);

            let cannons = red_cannon + black_cannon;
            let majors = red_majors + black_majors;

            if majors == 0 {
                // No Rooks, Cannons, or Knights remain on either side -> direct draw
                return Some(score::DRAW);
            }

            let mut is_mate_draw = false;

            if cannons == 1 && majors == 1 {
                // One cannon left on the board, and no other major pieces -> direct draw
                let (our_advisor, our_bishop, their_advisor) = if red_cannon == 1 {
                    (Piece::RedAdvisor, Piece::RedBishop, Piece::BlackAdvisor)
                } else {
                    (Piece::BlackAdvisor, Piece::BlackBishop, Piece::RedAdvisor)
                };

                if self.piece_count(our_advisor) == 0 {
                    let their_advisors = self.piece_count(their_advisor);
                    if their_advisors == 0 {
                        return Some(score::DRAW);
                    }
                    if their_advisors == 1 {
                        if self.piece_count(our_bishop) == 0 {
                            return Some(score::DRAW);
                        } else {
                            is_mate_draw = true;
                        }
                    } else if self.piece_count(our_bishop) == 0 {
                        is_mate_draw = true;
                    }
                }
            } else if red_cannon == 1
                && black_cannon == 1
                && red_majors == 1
                && black_majors == 1
                && self.piece_count(Piece::RedAdvisor) + self.piece_count(Piece::BlackAdvisor) == 0
            {
                // Two cannons on the board, exactly one for each side (i.e. neither side has
                // a second cannon or another major piece like a rook or knight), and no
                // advisors left on the board.
                if self.piece_count(Piece::RedBishop) + self.piece_count(Piece::BlackBishop) == 0 {
                    return Some(score::DRAW);
                } else {
                    is_mate_draw = true;
                }
            }

            if is_mate_draw {
                let mut moves = MoveList::new();
                generate_moves(self, MoveGenType::Legal, &mut moves);
                if moves.is_empty() {
                    return Some(score::mated_in(ply));
                }
                let mut new_moves = MoveList::new();
                for m in moves {
                    self.do_move(m);
                    new_moves.clear();
                    generate_moves(self, MoveGenType::Legal, &mut new_moves);
                    if new_moves.is_empty() {
                        // The position is winning, so we let the main search continue
                        self.undo_move(m);
                        return None;
                    }
                    self.undo_move(m);
                }
                return Some(score::DRAW);
            }
        }

        // 3. Repetition & Perpetual Check/Chase Loops
        let current_hash = self.state.zobrist;
        let rule_repetition = self.state.sixtymove_clock;
        let rule_repetition_val = rule_repetition as usize;
        let n = self.history.len();
        let max_back = rule_repetition_val.min(n);

        // Repetitions must occur on the same side's turn, so we scan back in steps of 2
        // plies.
        let mut i = 4;
        while i <= max_back {
            if self.history[n - i].zobrist == current_hash {
                // Repetition loop detected!
                let mut us_perpetual_check = true;
                let mut them_perpetual_check = true;

                let us = self.side_to_move();

                // Scan all intermediate plies in the loop (from `n - i` to `n - 1`)
                for j in (n - i)..n {
                    let player_who_moved =
                        Side::from_repr((self.game_ply - (n - j) as u16) as u8 & 1).unwrap();
                    let state_after = if j + 1 < n {
                        &self.history[j + 1]
                    } else {
                        &self.state
                    };

                    if player_who_moved == us {
                        if !state_after.in_check {
                            us_perpetual_check = false;
                        }
                    } else {
                        if !state_after.in_check {
                            them_perpetual_check = false;
                        }
                    }
                }

                if us_perpetual_check || them_perpetual_check {
                    if us_perpetual_check && them_perpetual_check {
                        return Some(score::DRAW); // Both check perpetually -> draw
                    } else if us_perpetual_check {
                        return Some(score::mated_in(ply)); // We check perpetually -> we lose
                    } else {
                        return Some(score::mate_in(ply)); // Opponent checks perpetually -> they lose
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
