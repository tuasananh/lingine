use crate::{
    bitboard::Bitboard,
    position::Position,
    types::{Color, MAX_MOVES, Move, MoveGenType, PieceType, Square},
};

/// Represents precomputed diagonal attack targets for a Bishop (Elephant).
/// In Xiangqi, Bishops move exactly 2 steps diagonally, and their jump is blocked
/// if the middle square (the "Elephant Eye") is occupied by ANY piece.
#[derive(Clone, Copy)]
struct BishopEntry {
    /// The 4 possible diagonal intermediate blocking squares (Elephant Eyes) around this square.
    eyes: [Option<Square>; 4],
    /// Precomputed valid target attack bitboards indexed by a 4-bit gathered blocker occupancy mask.
    attacks: [Bitboard; 16],
}

/// Represents precomputed attack targets for a quiet Knight (Horse) move.
/// Knights move 1 step orthogonally then 1 step diagonally. The move is blocked
/// if the intermediate orthogonal square (the "Horse Leg") is occupied.
#[derive(Clone, Copy)]
struct KnightEntry {
    /// The 4 intermediate orthogonal blocking squares (Horse Legs) around this square.
    eyes: [Option<Square>; 4],
    /// Precomputed valid target attack bitboards indexed by a 4-bit gathered blocker occupancy mask.
    attacks: [Bitboard; 16],
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
            if f - 1 >= 0 {
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
        if r - 1 >= 0 {
            mask |= 1 << ((r - 1) * 9 + f); // Forward move
        }
        if r <= 4 {
            // Crossed river (R0 to R4 are opponent's ranks)
            if f - 1 >= 0 {
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
        if r - 1 >= 0 {
            mask |= 1 << ((r - 1) * 9 + f); // Came from behind
        }
        if r >= 5 {
            if f - 1 >= 0 {
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
            if f - 1 >= 0 {
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
                    if let Some(l) = unique_legs[j] {
                        if l as u8 == leg_sq as u8 {
                            found = true;
                            break;
                        }
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
                        if let Some(l) = unique_legs[j] {
                            if l as u8 == leg_sq as u8 {
                                leg_idx = j;
                                found = true;
                                break;
                            }
                        }
                        j += 1;
                    }

                    if found {
                        if (occ_idx & (1 << leg_idx)) == 0 {
                            mask |= 1 << (from_r * 9 + from_f);
                        }
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
static KING_ATTACKS: [Bitboard; 90] = init_king_attacks();
static ADVISOR_ATTACKS: [Bitboard; 90] = init_advisor_attacks();
pub static PAWN_ATTACKS: [[Bitboard; 90]; 2] = init_pawn_attacks();
pub static PAWN_ATTACKS_TO: [[Bitboard; 90]; 2] = init_pawn_attacks_to();
static BISHOP_TABLE: [BishopEntry; 90] = init_bishop_table();
static KNIGHT_TABLE: [KnightEntry; 90] = init_knight_table();
pub static RANK_TABLE: [RankEntry; 9] = init_rank_table();
pub static FILE_TABLE: [FileEntry; 10] = init_file_table();
pub static KNIGHT_TO_TABLE: [KnightToEntry; 90] = init_knight_to_table();

/// Collects (gathers) the vertical file occupancy states into a 10-bit integer.
/// Every 9th bit in our `u128` bitboard represents the same file on successive ranks (R0 to R9).
/// Shifts, masks, and packs these bits dynamically in O(1) time without standard loops.
#[inline(always)]
pub fn gather_file_bits(bits: u128, f: usize) -> usize {
    let mut file_occ = 0;
    let occ = bits >> f;
    file_occ |= (occ & 1) as usize;
    file_occ |= (((occ >> 9) & 1) as usize) << 1;
    file_occ |= (((occ >> 18) & 1) as usize) << 2;
    file_occ |= (((occ >> 27) & 1) as usize) << 3;
    file_occ |= (((occ >> 36) & 1) as usize) << 4;
    file_occ |= (((occ >> 45) & 1) as usize) << 5;
    file_occ |= (((occ >> 54) & 1) as usize) << 6;
    file_occ |= (((occ >> 63) & 1) as usize) << 7;
    file_occ |= (((occ >> 72) & 1) as usize) << 8;
    file_occ |= (((occ >> 81) & 1) as usize) << 9;
    file_occ
}

/// Computes horizontal and vertical attack/move targets for a Rook (Chariot).
/// Combines precomputed `RANK_TABLE` and `FILE_TABLE` lookup masks.
#[inline(always)]
pub fn rook_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    let from_idx = square as usize;
    let f = from_idx % 9;
    let r = from_idx / 9;

    // 1. Rank attacks: Mask the 9 bits of the current rank (offset `r * 9`)
    let rank_occ = ((occupied.0 >> (r * 9)) & 0x1FF) as usize;
    let rank_attack_mask = RANK_TABLE[f].rook[rank_occ];
    let mut attack_bb = Bitboard((rank_attack_mask as u128) << (r * 9));

    // 2. File attacks: Gather the 10 bits of the current file
    let file_occ = gather_file_bits(occupied.0, f);
    let file_attack_mask = FILE_TABLE[r].rook[file_occ];
    let mut file_mask_bb = 0u128;
    let mut temp = file_attack_mask;
    while temp != 0 {
        let r_to = temp.trailing_zeros() as usize;
        temp &= temp - 1;
        file_mask_bb |= 1 << (r_to * 9 + f);
    }
    attack_bb.0 |= file_mask_bb;
    attack_bb
}

/// Computes horizontal and vertical quiet/leap capture masks for a Cannon.
#[inline(always)]
pub fn cannon_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    let from_idx = square as usize;
    let f = from_idx % 9;
    let r = from_idx / 9;

    // 1. Rank attacks: Mask the 9 bits of the rank
    let rank_occ = ((occupied.0 >> (r * 9)) & 0x1FF) as usize;
    let rank_attack_mask = RANK_TABLE[f].cannon[rank_occ];
    let mut attack_bb = Bitboard((rank_attack_mask as u128) << (r * 9));

    // 2. File attacks: Gather the 10 bits of the file
    let file_occ = gather_file_bits(occupied.0, f);
    let file_attack_mask = FILE_TABLE[r].cannon[file_occ];
    let mut file_mask_bb = 0u128;
    let mut temp = file_attack_mask;
    while temp != 0 {
        let r_to = temp.trailing_zeros() as usize;
        temp &= temp - 1;
        file_mask_bb |= 1 << (r_to * 9 + f);
    }
    attack_bb.0 |= file_mask_bb;
    attack_bb
}

/// Generates valid orthogonal moves for the General (King) inside the Palace.
fn generate_king_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    if let Some(from_sq) = pos.king_square(us) {
        let us_pieces = pos.bitboard_by_color(us);
        let mut target_bb = Bitboard(KING_ATTACKS[from_sq as usize].0 & !us_pieces.0);
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates diagonal moves for Advisors inside the Palace.
fn generate_advisor_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let mut advisors = pos.bitboard_by_type(PieceType::Advisor) & us_pieces;
    while let Some(from_sq) = advisors.pop_lsb() {
        let mut target_bb = Bitboard(ADVISOR_ATTACKS[from_sq as usize].0 & !us_pieces.0);
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates diagonal moves for Elephants (Bishops), checking diagonal blocker intermediate eyes.
fn generate_bishop_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let occupied = pos.bitboard_by_color(Color::White) | pos.bitboard_by_color(Color::Black);
    let mut bishops = pos.bitboard_by_type(PieceType::Bishop) & us_pieces;

    while let Some(from_sq) = bishops.pop_lsb() {
        let entry = &BISHOP_TABLE[from_sq as usize];
        let mut occ_idx = 0;

        let mut i = 0;
        while i < 4 {
            if let Some(eye_sq) = entry.eyes[i] {
                if occupied.is_occupied(eye_sq) {
                    occ_idx |= 1 << i;
                }
            }
            i += 1;
        }

        let mut target_bb = Bitboard(entry.attacks[occ_idx].0 & !us_pieces.0);
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates L-shaped moves for Horses (Knights), checking intermediate orthogonal blocker leg squares.
fn generate_knight_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let occupied = pos.bitboard_by_color(Color::White) | pos.bitboard_by_color(Color::Black);
    let mut knights = pos.bitboard_by_type(PieceType::Knight) & us_pieces;

    while let Some(from_sq) = knights.pop_lsb() {
        let entry = &KNIGHT_TABLE[from_sq as usize];
        let mut occ_idx = 0;

        let mut i = 0;
        while i < 4 {
            if let Some(eye_sq) = entry.eyes[i] {
                if occupied.is_occupied(eye_sq) {
                    occ_idx |= 1 << i;
                }
            }
            i += 1;
        }

        let mut target_bb = Bitboard(entry.attacks[occ_idx].0 & !us_pieces.0);
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates moves for Soldiers (Pawns) based on whether they have crossed the river or not.
fn generate_pawn_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let mut pawns = pos.bitboard_by_type(PieceType::Pawn) & us_pieces;
    let color_idx = if IS_WHITE { 0 } else { 1 };

    while let Some(from_sq) = pawns.pop_lsb() {
        let mut target_bb = Bitboard(PAWN_ATTACKS[color_idx][from_sq as usize].0 & !us_pieces.0);
        while let Some(to_sq) = target_bb.pop_lsb() {
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates horizontal and vertical sliding moves for Chariots (Rooks) in O(1) lookups.
fn generate_rook_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let us_pieces = pos.bitboard_by_color(us);
    let occupied = pos.bitboard_by_color(Color::White) | pos.bitboard_by_color(Color::Black);
    let mut rooks = pos.bitboard_by_type(PieceType::Rook) & us_pieces;

    while let Some(from_sq) = rooks.pop_lsb() {
        let from_idx = from_sq as usize;
        let f = from_idx % 9;
        let r = from_idx / 9;

        // 1. Rank attacks
        let rank_occ = ((occupied.0 >> (r * 9)) & 0x1FF) as usize;
        let us_rank_mask = ((us_pieces.0 >> (r * 9)) & 0x1FF) as u16;
        let mut rank_attack_mask = RANK_TABLE[f].rook[rank_occ] & !us_rank_mask;

        while rank_attack_mask != 0 {
            let f_to = rank_attack_mask.trailing_zeros() as usize;
            rank_attack_mask &= rank_attack_mask - 1;
            let to_sq = Square::from_repr((r * 9 + f_to) as u8).unwrap();
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }

        // 2. File attacks
        let file_occ = gather_file_bits(occupied.0, f);
        let us_file_mask = gather_file_bits(us_pieces.0, f) as u16;
        let mut file_attack_mask = FILE_TABLE[r].rook[file_occ] & !us_file_mask;

        while file_attack_mask != 0 {
            let r_to = file_attack_mask.trailing_zeros() as usize;
            file_attack_mask &= file_attack_mask - 1;
            let to_sq = Square::from_repr((r_to * 9 + f) as u8).unwrap();
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Generates horizontal and vertical moves/leap captures for Cannons in O(1) lookups.
fn generate_cannon_moves<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
    count: &mut usize,
) {
    let us = if IS_WHITE { Color::White } else { Color::Black };
    let them = us.opposite();
    let us_pieces = pos.bitboard_by_color(us);
    let them_pieces = pos.bitboard_by_color(them);
    let occupied = pos.bitboard_by_color(Color::White) | pos.bitboard_by_color(Color::Black);
    let mut cannons = pos.bitboard_by_type(PieceType::Cannon) & us_pieces;

    while let Some(from_sq) = cannons.pop_lsb() {
        let from_idx = from_sq as usize;
        let f = from_idx % 9;
        let r = from_idx / 9;

        // 1. Rank moves (horizontal quiet + leap captures)
        let rank_occ = ((occupied.0 >> (r * 9)) & 0x1FF) as usize;
        let them_rank_mask = ((them_pieces.0 >> (r * 9)) & 0x1FF) as u16;

        let rank_quiet_mask =
            RANK_TABLE[f].rook[rank_occ] & !((occupied.0 >> (r * 9)) & 0x1FF) as u16;
        let rank_capture_mask = RANK_TABLE[f].cannon[rank_occ] & them_rank_mask;

        let mut rank_attack_mask = rank_quiet_mask | rank_capture_mask;
        while rank_attack_mask != 0 {
            let f_to = rank_attack_mask.trailing_zeros() as usize;
            rank_attack_mask &= rank_attack_mask - 1;
            let to_sq = Square::from_repr((r * 9 + f_to) as u8).unwrap();
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }

        // 2. File moves (vertical quiet + leap captures)
        let file_occ = gather_file_bits(occupied.0, f);
        let them_file_mask = gather_file_bits(them_pieces.0, f) as u16;
        let occ_file_mask = gather_file_bits(occupied.0, f) as u16;

        let file_quiet_mask = FILE_TABLE[r].rook[file_occ] & !occ_file_mask;
        let file_capture_mask = FILE_TABLE[r].cannon[file_occ] & them_file_mask;

        let mut file_attack_mask = file_quiet_mask | file_capture_mask;
        while file_attack_mask != 0 {
            let r_to = file_attack_mask.trailing_zeros() as usize;
            file_attack_mask &= file_attack_mask - 1;
            let to_sq = Square::from_repr((r_to * 9 + f) as u8).unwrap();
            moves[*count] = Move::new(from_sq, to_sq);
            *count += 1;
        }
    }
}

/// Orchestrates move generators for all piece types, returning the total pseudo-legal move count.
fn generate_pseudo_legal<const IS_WHITE: bool>(
    pos: &Position,
    moves: &mut [Move; MAX_MOVES],
) -> usize {
    let mut count = 0;
    generate_king_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_advisor_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_bishop_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_knight_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_pawn_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_rook_moves::<IS_WHITE>(pos, moves, &mut count);
    generate_cannon_moves::<IS_WHITE>(pos, moves, &mut count);
    count
}

/// The main entry point for move generation.
/// Filters pseudo-legal moves into legal moves (e.g. by ensuring the King is not left in check)
/// and respects the target `MoveGenType` request (Legal, PseudoLegal, Quiets, Captures, Evasions).
pub fn generate_moves(
    pos: &Position,
    gen_type: MoveGenType,
    moves: &mut [Move; MAX_MOVES],
) -> usize {
    let color = pos.side_to_move();
    let mut count = match color {
        Color::White => generate_pseudo_legal::<true>(pos, moves),
        Color::Black => generate_pseudo_legal::<false>(pos, moves),
    };

    if gen_type == MoveGenType::PseudoLegal {
        return count;
    }

    let mut cur = 0;
    while cur < count {
        let m = moves[cur];
        let is_legal = pos.legal(m);
        let keep = if is_legal {
            match gen_type {
                MoveGenType::Legal | MoveGenType::Evasions => true,
                MoveGenType::Captures => !pos.is_empty(m.square_to()),
                MoveGenType::Quiets => pos.is_empty(m.square_to()),
                _ => false,
            }
        } else {
            false
        };

        if !keep {
            count -= 1;
            moves[cur] = moves[count]; // Swap with the last move in the list
        } else {
            cur += 1;
        }
    }

    count
}
