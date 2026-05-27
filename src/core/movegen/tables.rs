use crate::core::{bitboard::Bitboard, types::Square};

/// Represents precomputed diagonal attack targets for a Bishop (Elephant).
/// In Xiangqi, Bishops move exactly 2 steps diagonally, and their jump is blocked
/// if the middle square (the "Elephant Eye") is occupied by ANY piece.
#[derive(Clone, Copy)]
pub(super) struct BishopEntry {
    /// The 4 possible diagonal intermediate blocking squares (Elephant Eyes) around this square.
    pub(super) eyes: [Option<Square>; 4],
    /// Precomputed valid target attack bitboards indexed by a 4-bit gathered blocker occupancy mask.
    pub(super) attacks: [Bitboard; 16],
}

/// Represents precomputed attack targets for a quiet Knight (Horse) move.
/// Knights move 1 step orthogonally then 1 step diagonally. The move is blocked
/// if the intermediate orthogonal square (the "Horse Leg") is occupied.
#[derive(Clone, Copy)]
pub(super) struct KnightEntry {
    /// The 4 intermediate orthogonal blocking squares (Horse Legs) around this square.
    pub(super) eyes: [Option<Square>; 4],
    /// Precomputed valid target attack bitboards indexed by a 4-bit gathered blocker occupancy mask.
    pub(super) attacks: [Bitboard; 16],
}

/// Represents precomputed attack targets for Knight checking paths.
/// Used in `Position::checkers_to` to perform rapid, O(1) backward checks from the King.
/// Matches Knight attacks coming from up to 8 directions, which can share up to 6 unique Horse Legs.
#[derive(Clone, Copy)]
pub struct KnightToEntry {
    /// The 6 possible unique Horse Legs that can block any Knight's attack onto this square.
    pub eyes: [Option<Square>; 6],
    /// Precomputed attack origin bitboards indexed by a 6-bit gathered blocker occupancy mask (0 to 63).
    pub attacks: [Bitboard; 64],
}

/// Holds precomputed horizontal rank attack/move masks for Rooks and Cannons.
/// Indexed by the 9-bit rank occupancy state (0 to 511).
#[derive(Clone, Copy)]
pub struct RankEntry {
    /// Rook sliding quiet and capture targets. Stops sliding immediately upon hitting a blocker.
    pub rook: [u16; 512],
    /// Cannon quiet and leap capture targets. Skips quiet squares, jumps over exactly 1 screen,
    /// and targets the first piece behind it.
    pub cannon: [u16; 512],
}

/// Holds precomputed vertical file attack/move masks for Rooks and Cannons.
/// Indexed by the 10-bit file occupancy state (0 to 1023).
#[derive(Clone, Copy)]
pub struct FileEntry {
    /// Rook vertical sliding targets.
    pub rook: [u16; 1024],
    /// Cannon vertical sliding and leap capture targets.
    pub cannon: [u16; 1024],
}

/// Precomputes valid orthogonal moves for the General (King) inside the Palace.
/// Generals can only move 1 step orthogonally (up/down/left/right) and are strictly
/// confined to the 3x3 Palace.
const fn init_king_attacks() -> [Bitboard; 90] {
    let mut table = [Bitboard(0); 90];
    let mut from_idx = 0;
    while from_idx < 90 {
        let f = (from_idx % 9) as i8;
        let r = (from_idx / 9) as i8;
        let is_white_palace = (f >= 3 && f <= 5) && (r >= 0 && r <= 2);
        let is_black_palace = (f >= 3 && f <= 5) && (r >= 7 && r <= 9);
        if is_white_palace || is_black_palace {
            let king_dirs = [(0, 1), (0, -1), (1, 0), (-1, 0)];
            let mut i = 0;
            let mut mask = 0u128;
            while i < 4 {
                let (df, dr) = king_dirs[i];
                let nf = f + df;
                let nr = r + dr;
                let is_in_same_palace = if is_white_palace {
                    (nf >= 3 && nf <= 5) && (nr >= 0 && nr <= 2)
                } else {
                    (nf >= 3 && nf <= 5) && (nr >= 7 && nr <= 9)
                };
                if is_in_same_palace {
                    let to_idx = nr * 9 + nf;
                    mask |= 1 << to_idx;
                }
                i += 1;
            }
            table[from_idx as usize] = Bitboard(mask);
        }
        from_idx += 1;
    }
    table
}

/// Precomputes valid diagonal moves for Advisors inside the Palace.
/// Advisors move exactly 1 step diagonally and are strictly confined to the Palace.
/// There are exactly 5 valid Palace squares for an Advisor.
const fn init_advisor_attacks() -> [Bitboard; 90] {
    let mut table = [Bitboard(0); 90];
    let mut from_idx = 0;
    while from_idx < 90 {
        let f = (from_idx % 9) as i8;
        let r = (from_idx / 9) as i8;
        let is_white_palace = (f >= 3 && f <= 5) && (r >= 0 && r <= 2);
        let is_black_palace = (f >= 3 && f <= 5) && (r >= 7 && r <= 9);
        if is_white_palace || is_black_palace {
            let advisor_dirs = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
            let mut i = 0;
            let mut mask = 0u128;
            while i < 4 {
                let (df, dr) = advisor_dirs[i];
                let nf = f + df;
                let nr = r + dr;
                let is_in_same_palace = if is_white_palace {
                    (nf >= 3 && nf <= 5) && (nr >= 0 && nr <= 2)
                } else {
                    (nf >= 3 && nf <= 5) && (nr >= 7 && nr <= 9)
                };
                if is_in_same_palace {
                    let to_idx = nr * 9 + nf;
                    mask |= 1 << to_idx;
                }
                i += 1;
            }
            table[from_idx as usize] = Bitboard(mask);
        }
        from_idx += 1;
    }
    table
}

/// Precomputes Pawn attack masks for both White and Black Pawns.
///
/// * **Unpromoted (own side)**: Can only move exactly 1 step straight forward.
/// * **Promoted (crossed river)**: Can move 1 step straight forward OR 1 step horizontally (left/right).
const fn init_pawn_attacks() -> [[Bitboard; 90]; 2] {
    let mut table = [[Bitboard(0); 90]; 2];

    // Index 0: White Pawn
    let mut from_idx = 0;
    while from_idx < 90 {
        let f = (from_idx % 9) as i8;
        let r = (from_idx / 9) as i8;
        let mut mask = 0u128;
        if r + 1 < 10 {
            mask |= 1 << ((r + 1) * 9 + f); // Forward move
        }
        if r >= 5 {
            // Crossed river (R5 to R9 are opponent's ranks)
            if f > 0 {
                mask |= 1 << (r * 9 + f - 1); // Left sideways move
            }
            if f + 1 < 9 {
                mask |= 1 << (r * 9 + f + 1); // Right sideways move
            }
        }
        table[0][from_idx as usize] = Bitboard(mask);
        from_idx += 1;
    }

    // Index 1: Black Pawn
    let mut from_idx = 0;
    while from_idx < 90 {
        let f = (from_idx % 9) as i8;
        let r = (from_idx / 9) as i8;
        let mut mask = 0u128;
        if r > 0 {
            mask |= 1 << ((r - 1) * 9 + f); // Forward move
        }
        if r <= 4 {
            // Crossed river (R0 to R4 are opponent's ranks)
            if f > 0 {
                mask |= 1 << (r * 9 + f - 1); // Left sideways move
            }
            if f + 1 < 9 {
                mask |= 1 << (r * 9 + f + 1); // Right sideways move
            }
        }
        table[1][from_idx as usize] = Bitboard(mask);
        from_idx += 1;
    }

    table
}

/// Precomputes "reverse" Pawn attack lists.
/// Represents which source squares a Pawn of the given color could have come from
/// to attack the target square. Used in `Position::checkers_to` to check Pawn checks.
const fn init_pawn_attacks_to() -> [[Bitboard; 90]; 2] {
    let mut table = [[Bitboard(0); 90]; 2];

    // Index 0: White pawn attacking a square
    let mut square_idx = 0;
    while square_idx < 90 {
        let f = (square_idx % 9) as i8;
        let r = (square_idx / 9) as i8;
        let mut mask = 0u128;
        if r > 0 {
            mask |= 1 << ((r - 1) * 9 + f); // Came from behind
        }
        if r >= 5 {
            if f > 0 {
                mask |= 1 << (r * 9 + f - 1); // Came from left
            }
            if f + 1 < 9 {
                mask |= 1 << (r * 9 + f + 1); // Came from right
            }
        }
        table[0][square_idx as usize] = Bitboard(mask);
        square_idx += 1;
    }

    // Index 1: Black pawn attacking a square
    let mut square_idx = 0;
    while square_idx < 90 {
        let f = (square_idx % 9) as i8;
        let r = (square_idx / 9) as i8;
        let mut mask = 0u128;
        if r + 1 < 10 {
            mask |= 1 << ((r + 1) * 9 + f); // Came from in front
        }
        if r <= 4 {
            if f > 0 {
                mask |= 1 << (r * 9 + f - 1); // Came from left
            }
            if f + 1 < 9 {
                mask |= 1 << (r * 9 + f + 1); // Came from right
            }
        }
        table[1][square_idx as usize] = Bitboard(mask);
        square_idx += 1;
    }

    table
}

/// Precomputes Elephant (Bishop) jump entries.
/// Elephant jumps are 2-step diagonal leaps, confined to their own side of the board.
/// Intermediate blocking eyes (diagonally 1 step away) are checked.
const fn init_bishop_table() -> [BishopEntry; 90] {
    let mut table = [BishopEntry {
        eyes: [None; 4],
        attacks: [Bitboard(0); 16],
    }; 90];
    let mut from_idx = 0;
    while from_idx < 90 {
        let f = (from_idx % 9) as i8;
        let r = (from_idx / 9) as i8;
        let is_white_side = r <= 4;
        let elephant_jumps = [
            (2, 2, 1, 1),     // Up-Right jump, Blocker eye offset: (1, 1)
            (2, -2, 1, -1),   // Down-Right jump, Blocker eye offset: (1, -1)
            (-2, 2, -1, 1),   // Up-Left jump, Blocker eye offset: (-1, 1)
            (-2, -2, -1, -1), // Down-Left jump, Blocker eye offset: (-1, -1)
        ];

        let mut num_jumps = 0;
        let mut jump_targets = [0u8; 4];

        let mut i = 0;
        while i < 4 {
            let (df, dr, lf, lr) = elephant_jumps[i];
            let nf = f + df;
            let nr = r + dr;
            let is_same_side = if is_white_side {
                nr >= 0 && nr <= 4
            } else {
                nr >= 5 && nr <= 9
            };
            if nf >= 0 && nf < 9 && is_same_side {
                let to_idx = nr * 9 + nf;
                let eye_idx = (r + lr) * 9 + (f + lf);

                jump_targets[num_jumps] = to_idx as u8;
                table[from_idx as usize].eyes[num_jumps] =
                    Some(Square::from_repr(eye_idx as u8).unwrap());
                num_jumps += 1;
            }
            i += 1;
        }

        let mut occ_idx = 0;
        while occ_idx < 16 {
            let mut mask = 0u128;
            let mut j = 0;
            while j < num_jumps {
                // If the gathered bit is 0, the eye is empty, so the jump is unblocked!
                if (occ_idx & (1 << j)) == 0 {
                    mask |= 1 << jump_targets[j];
                }
                j += 1;
            }
            table[from_idx as usize].attacks[occ_idx] = Bitboard(mask);
            occ_idx += 1;
        }

        from_idx += 1;
    }
    table
}

/// Precomputes Horse (Knight) jumps and intermediate blocking Horse Legs.
/// Knights move 1 step orthogonally then 1 step diagonally. If the 1st orthogonal square
/// is occupied, the jump is blocked.
const fn init_knight_table() -> [KnightEntry; 90] {
    let mut table = [KnightEntry {
        eyes: [None; 4],
        attacks: [Bitboard(0); 16],
    }; 90];
    let mut from_idx = 0;
    while from_idx < 90 {
        let f = (from_idx % 9) as i8;
        let r = (from_idx / 9) as i8;

        let eye_offsets = [(0, 1), (0, -1), (1, 0), (-1, 0)]; // Up, Down, Right, Left legs
        let jump_offsets = [
            [(1, 2), (-1, 2)],   // Jumps corresponding to Up leg
            [(1, -2), (-1, -2)], // Jumps corresponding to Down leg
            [(2, 1), (2, -1)],   // Jumps corresponding to Right leg
            [(-2, 1), (-2, -1)], // Jumps corresponding to Left leg
        ];

        let mut num_eyes = 0;
        let mut eye_jump_targets = [[0u8; 2]; 4];
        let mut eye_jump_valid = [[false; 2]; 4];

        let mut i = 0;
        while i < 4 {
            let (ef, er) = eye_offsets[i];
            let eye_f = f + ef;
            let eye_r = r + er;
            if eye_f >= 0 && eye_f < 9 && eye_r >= 0 && eye_r < 10 {
                let eye_idx = eye_r * 9 + eye_f;
                table[from_idx as usize].eyes[num_eyes] =
                    Some(Square::from_repr(eye_idx as u8).unwrap());

                let mut j = 0;
                while j < 2 {
                    let (jf, jr) = jump_offsets[i][j];
                    let nf = f + jf;
                    let nr = r + jr;
                    if nf >= 0 && nf < 9 && nr >= 0 && nr < 10 {
                        eye_jump_targets[num_eyes][j] = (nr * 9 + nf) as u8;
                        eye_jump_valid[num_eyes][j] = true;
                    }
                    j += 1;
                }
                num_eyes += 1;
            }
            i += 1;
        }

        let mut occ_idx = 0;
        while occ_idx < 16 {
            let mut mask = 0u128;
            let mut e = 0;
            while e < num_eyes {
                // If the leg's occupancy bit is 0, the leg is empty, so both jumps are valid!
                if (occ_idx & (1 << e)) == 0 {
                    let mut j = 0;
                    while j < 2 {
                        if eye_jump_valid[e][j] {
                            mask |= 1 << eye_jump_targets[e][j];
                        }
                        j += 1;
                    }
                }
                e += 1;
            }
            table[from_idx as usize].attacks[occ_idx] = Bitboard(mask);
            occ_idx += 1;
        }

        from_idx += 1;
    }
    table
}

/// Precomputes sliding Rank occupancy attack masks.
/// A rank has 9 files, so the occupancy is a 9-bit number (0..511).
///
/// * **Rook**: Slides orthogonally, stopping on the first blocking piece (quiet on empty, capture on opponent).
/// * **Cannon**: Quiet moves are identical to Rook, but captures require jumping over exactly 1 blocking screen,
///   landing on the next piece.
const fn init_rank_table() -> [RankEntry; 9] {
    let mut table = [RankEntry {
        rook: [0; 512],
        cannon: [0; 512],
    }; 9];
    let mut f = 0;
    while f < 9 {
        let mut occ = 0;
        while occ < 512 {
            // 1. Rook horizontal attack generation
            let mut r_mask = 0u16;
            let mut temp_f = f - 1;
            while temp_f >= 0 {
                r_mask |= 1 << temp_f;
                if (occ & (1 << temp_f)) != 0 {
                    break; // Hit a blocking piece
                }
                temp_f -= 1;
            }
            let mut temp_f = f + 1;
            while temp_f < 9 {
                r_mask |= 1 << temp_f;
                if (occ & (1 << temp_f)) != 0 {
                    break; // Hit a blocking piece
                }
                temp_f += 1;
            }
            table[f as usize].rook[occ as usize] = r_mask;

            // 2. Cannon horizontal leap capture generation
            let mut c_mask = 0u16;
            let mut temp_f = f - 1;
            let mut screen = false; // Tracks if we have crossed exactly 1 screen
            while temp_f >= 0 {
                let occupied = (occ & (1 << temp_f)) != 0;
                if !screen {
                    if occupied {
                        screen = true; // Found the screen
                    }
                } else {
                    if occupied {
                        c_mask |= 1 << temp_f; // Found the target behind the screen!
                        break;
                    }
                }
                temp_f -= 1;
            }
            let mut temp_f = f + 1;
            let mut screen = false;
            while temp_f < 9 {
                let occupied = (occ & (1 << temp_f)) != 0;
                if !screen {
                    if occupied {
                        screen = true; // Found the screen
                    }
                } else {
                    if occupied {
                        c_mask |= 1 << temp_f; // Found the target behind the screen!
                        break;
                    }
                }
                temp_f += 1;
            }
            table[f as usize].cannon[occ as usize] = c_mask;

            occ += 1;
        }
        f += 1;
    }
    table
}

/// Precomputes sliding File occupancy attack masks.
/// A file has 10 ranks, so the occupancy is a 10-bit number (0..1023).
/// Generates vertical Rook attacks and Cannon leap capture targets.
const fn init_file_table() -> [FileEntry; 10] {
    let mut table = [FileEntry {
        rook: [0; 1024],
        cannon: [0; 1024],
    }; 10];
    let mut r = 0;
    while r < 10 {
        let mut occ = 0;
        while occ < 1024 {
            // 1. Rook vertical attack generation
            let mut r_mask = 0u16;
            let mut temp_r = r - 1;
            while temp_r >= 0 {
                r_mask |= 1 << temp_r;
                if (occ & (1 << temp_r)) != 0 {
                    break;
                }
                temp_r -= 1;
            }
            let mut temp_r = r + 1;
            while temp_r < 10 {
                r_mask |= 1 << temp_r;
                if (occ & (1 << temp_r)) != 0 {
                    break;
                }
                temp_r += 1;
            }
            table[r as usize].rook[occ as usize] = r_mask;

            // 2. Cannon vertical leap capture generation
            let mut c_mask = 0u16;
            let mut temp_r = r - 1;
            let mut screen = false;
            while temp_r >= 0 {
                let occupied = (occ & (1 << temp_r)) != 0;
                if !screen {
                    if occupied {
                        screen = true;
                    }
                } else {
                    if occupied {
                        c_mask |= 1 << temp_r;
                        break;
                    }
                }
                temp_r -= 1;
            }
            let mut temp_r = r + 1;
            let mut screen = false;
            while temp_r < 10 {
                let occupied = (occ & (1 << temp_r)) != 0;
                if !screen {
                    if occupied {
                        screen = true;
                    }
                } else {
                    if occupied {
                        c_mask |= 1 << temp_r;
                        break;
                    }
                }
                temp_r += 1;
            }
            table[r as usize].cannon[occ as usize] = c_mask;

            occ += 1;
        }
        r += 1;
    }
    table
}

/// Precomputes Knight backward attackers.
/// Gathers a 6-bit blocker index corresponding to the Horse Legs.
const fn init_knight_to_table() -> [KnightToEntry; 90] {
    let mut table = [KnightToEntry {
        eyes: [None; 6],
        attacks: [Bitboard(0); 64],
    }; 90];
    let mut from_idx = 0;
    while from_idx < 90 {
        let f = (from_idx % 9) as i8;
        let r = (from_idx / 9) as i8;

        let jump_offsets = [
            (1, 2),
            (-1, 2),
            (1, -2),
            (-1, -2),
            (2, 1),
            (2, -1),
            (-2, 1),
            (-2, -1),
        ];

        let mut unique_legs = [None; 6];
        let mut num_legs = 0;

        let mut i = 0;
        while i < 8 {
            let (df, dr) = jump_offsets[i];
            let from_f = f - df;
            let from_r = r - dr;
            if from_f >= 0 && from_f < 9 && from_r >= 0 && from_r < 10 {
                let leg_f = from_f
                    + if df == 2 {
                        1
                    } else if df == -2 {
                        -1
                    } else {
                        0
                    };
                let leg_r = from_r
                    + if dr == 2 {
                        1
                    } else if dr == -2 {
                        -1
                    } else {
                        0
                    };
                let leg_sq = Square::from_repr((leg_r * 9 + leg_f) as u8).unwrap();

                let mut found = false;
                let mut j = 0;
                while j < num_legs {
                    if let Some(l) = unique_legs[j]
                        && l as u8 == leg_sq as u8
                    {
                        found = true;
                        break;
                    }
                    j += 1;
                }
                if !found {
                    unique_legs[num_legs] = Some(leg_sq);
                    num_legs += 1;
                }
            }
            i += 1;
        }

        let mut j = 0;
        while j < num_legs {
            table[from_idx as usize].eyes[j] = unique_legs[j];
            j += 1;
        }

        let mut occ_idx = 0;
        while occ_idx < 64 {
            let mut mask = 0u128;

            let mut i = 0;
            while i < 8 {
                let (df, dr) = jump_offsets[i];
                let from_f = f - df;
                let from_r = r - dr;
                if from_f >= 0 && from_f < 9 && from_r >= 0 && from_r < 10 {
                    let leg_f = from_f
                        + if df == 2 {
                            1
                        } else if df == -2 {
                            -1
                        } else {
                            0
                        };
                    let leg_r = from_r
                        + if dr == 2 {
                            1
                        } else if dr == -2 {
                            -1
                        } else {
                            0
                        };
                    let leg_sq = Square::from_repr((leg_r * 9 + leg_f) as u8).unwrap();

                    let mut leg_idx = 0;
                    let mut found = false;
                    let mut j = 0;
                    while j < num_legs {
                        if let Some(l) = unique_legs[j]
                            && l as u8 == leg_sq as u8
                        {
                            leg_idx = j;
                            found = true;
                            break;
                        }
                        j += 1;
                    }

                    if found && (occ_idx & (1 << leg_idx)) == 0 {
                        mask |= 1 << (from_r * 9 + from_f);
                    }
                }
                i += 1;
            }
            table[from_idx as usize].attacks[occ_idx] = Bitboard(mask);
            occ_idx += 1;
        }

        from_idx += 1;
    }
    table
}

// Precomputed static lookup tables dissolved at compile-time to eliminate thread checks, lock contention,
// and atomic operations during perft search loops.
pub(super) static KING_ATTACKS: [Bitboard; 90] = init_king_attacks();
pub(super) static ADVISOR_ATTACKS: [Bitboard; 90] = init_advisor_attacks();
pub static PAWN_ATTACKS: [[Bitboard; 90]; 2] = init_pawn_attacks();
pub static PAWN_ATTACKS_TO: [[Bitboard; 90]; 2] = init_pawn_attacks_to();
pub(super) static BISHOP_TABLE: [BishopEntry; 90] = init_bishop_table();
pub(super) static KNIGHT_TABLE: [KnightEntry; 90] = init_knight_table();
pub static RANK_TABLE: [RankEntry; 9] = init_rank_table();
pub static FILE_TABLE: [FileEntry; 10] = init_file_table();
pub static KNIGHT_TO_TABLE: [KnightToEntry; 90] = init_knight_to_table();

const fn init_file_attacks_by_mask() -> [[Bitboard; 1024]; 9] {
    let mut table = [[Bitboard(0); 1024]; 9];
    let mut f = 0;
    while f < 9 {
        let mut mask = 0;
        while mask < 1024 {
            let mut bits = 0u128;
            let mut r = 0;
            while r < 10 {
                if (mask & (1 << r)) != 0 {
                    bits |= 1u128 << (r * 9 + f);
                }
                r += 1;
            }
            table[f as usize][mask as usize] = Bitboard(bits);
            mask += 1;
        }
        f += 1;
    }
    table
}

pub static FILE_ATTACKS_BY_MASK: [[Bitboard; 1024]; 9] = init_file_attacks_by_mask();
