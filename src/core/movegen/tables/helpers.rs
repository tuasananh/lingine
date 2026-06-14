use crate::core::{File, Rank, Square};
use strum::EnumCount;

const RANK_STRIDE: i8 = File::COUNT as i8;

/// Returns the bitmask of valid destinations from `from_sq` for a piece that
/// steps in `dirs` and must stay within the same palace half.
pub(super) const fn palace_step_attacks(from_sq: Square, dirs: &[(i8, i8); 4]) -> u128 {
    let f = from_sq.file() as i8;
    let r = from_sq.rank() as i8;

    let in_white = (f >= 3 && f <= 5) && (r >= 0 && r <= 2);
    let in_black = (f >= 3 && f <= 5) && (r >= 7 && r <= 9);
    if !in_white && !in_black {
        return 0;
    }

    let mut mask = 0u128;
    let mut i = 0;
    while i < 4 {
        let (df, dr) = dirs[i];
        let nf = f + df;
        let nr = r + dr;
        let in_same = if in_white {
            (nf >= 3 && nf <= 5) && (nr >= 0 && nr <= 2)
        } else {
            (nf >= 3 && nf <= 5) && (nr >= 7 && nr <= 9)
        };
        if in_same {
            mask |= 1 << (nr * RANK_STRIDE + nf);
        }
        i += 1;
    }
    mask
}

/// Builds the pawn attack mask for a single square.
///
/// A pawn always attacks one square in its `forward_dr` direction (+1 = up for
/// Red, -1 = down for Black). After crossing the river (`promoted == true`) it
/// also attacks the two sideways squares on its current rank.
#[inline(always)]
pub(super) const fn pawn_attacks_from(f: i8, r: i8, forward_dr: i8, promoted: bool) -> u128 {
    let mut mask = 0u128;
    // Forward square (always present if on board).
    let nr = r + forward_dr;
    if nr >= 0 && nr < Rank::COUNT as i8 {
        mask |= 1 << (nr * RANK_STRIDE + f);
    }
    // Sideways squares (only for promoted pawns that have crossed the river).
    if promoted {
        if f > 0 {
            mask |= 1 << (r * RANK_STRIDE + f - 1);
        }
        if f + 1 < File::COUNT as i8 {
            mask |= 1 << (r * RANK_STRIDE + f + 1);
        }
    }
    mask
}

/// Rook ray: slides outward from `pos` in both directions, stopping *at* the
/// first occupied square (which can be captured, so it is included in the
/// mask).
pub(super) const fn rook_ray(pos: i8, len: i8, occ: u32) -> u32 {
    let mut mask = 0u32;
    // Slide left (toward square 0).
    let mut i = pos - 1;
    while i >= 0 {
        mask |= 1 << i;
        if (occ & (1 << i)) != 0 {
            break; // blocked — include blocker, then stop
        }
        i -= 1;
    }
    // Slide right (toward square len-1).
    let mut i = pos + 1;
    while i < len {
        mask |= 1 << i;
        if (occ & (1 << i)) != 0 {
            break;
        }
        i += 1;
    }
    mask
}

/// Cannon capture ray: locates the first occupied square on each side (the
/// "screen"), then sets the bit of the *second* occupied square beyond it (the
/// capture target). Empty squares and the screen itself are NOT included.
pub(super) const fn cannon_ray(pos: i8, len: i8, occ: u32) -> u32 {
    let mut mask = 0u32;

    // Left side.
    let mut i = pos - 1;
    let mut screen = false; // have we passed one occupied square?
    while i >= 0 {
        if (occ & (1 << i)) != 0 {
            if screen {
                // Second piece found — this is the capture target.
                mask |= 1 << i;
                break;
            }
            screen = true; // first piece = the screen
        }
        i -= 1;
    }

    // Right side.
    let mut i = pos + 1;
    let mut screen = false;
    while i < len {
        if (occ & (1 << i)) != 0 {
            if screen {
                mask |= 1 << i;
                break;
            }
            screen = true;
        }
        i += 1;
    }

    mask
}

/// Cannon attack ray: all squares strictly behind the screen piece, including
/// the capture target itself. This covers every square a cannon *threatens* or
/// x-rays once a screen is present.
///
/// Contrast with [`cannon_ray`] which only marks the capture target square.
pub(super) const fn cannon_beyond_attack(pos: i8, len: i8, occ: u32) -> u32 {
    let mut mask = 0u32;

    // Left side.
    let mut i = pos - 1;
    let mut screen = false;
    while i >= 0 {
        if screen {
            // Every square past the screen is part of the attack ray.
            mask |= 1 << i;
        }
        if (occ & (1 << i)) != 0 {
            if screen {
                break; // capture target reached — stop
            }
            screen = true; // just hit the screen piece
        }
        i -= 1;
    }

    // Right side.
    let mut i = pos + 1;
    let mut screen = false;
    while i < len {
        if screen {
            mask |= 1 << i;
        }
        if (occ & (1 << i)) != 0 {
            if screen {
                break;
            }
            screen = true;
        }
        i += 1;
    }

    mask
}

/// xorshift128 PRNG — used during magic search to generate random candidates.
/// Returns a new pseudo-random value and updates `state` in place.
pub(super) const fn xorshift128(state: &mut u128) -> u128 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Returns a sparse (few set bits) random value by ANDing three xorshift
/// outputs together. Sparse values tend to make better magic multipliers
/// because they concentrate the relevant bits more tightly.
pub(super) const fn sparse_rand(state: &mut u128) -> u128 {
    xorshift128(state) & xorshift128(state) & xorshift128(state)
}

/// Knight attacks from `from_sq` given board occupancy `occ`.
///
/// A knight moves in an "L" shape: one step along a *leg* direction, then two
/// steps perpendicular. The leg square must be empty (orthogonally blocked),
/// otherwise that arm of the L is cut off.
///
/// `LEGS` lists the four possible leg directions. For each unblocked leg, the
/// two possible landing squares in `TARGETS` are added to the attack mask.
pub(super) const fn knight_attacks(from_sq: Square, occ: u128) -> u128 {
    let r = from_sq.rank() as i8;
    let f = from_sq.file() as i8;
    // One step in each cardinal direction — the knight's "leg".
    const LEGS: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    // For each leg, the two L-shaped destinations (two steps perpendicular).
    const TARGETS: [[(i8, i8); 2]; 4] = [
        [(2, -1), (2, 1)],
        [(-2, -1), (-2, 1)],
        [(-1, 2), (1, 2)],
        [(-1, -2), (1, -2)],
    ];
    let mut attacks = 0u128;
    let mut i = 0;
    while i < 4 {
        let (ldr, ldf) = LEGS[i];
        let lr = r + ldr;
        let lf = f + ldf;
        // The leg square must be on the board and unoccupied.
        if lr >= 0
            && lr < Rank::COUNT as i8
            && lf >= 0
            && lf < File::COUNT as i8
            && (occ & (1 << (lr * RANK_STRIDE + lf))) == 0
        {
            let mut t = 0;
            while t < 2 {
                let (tdr, tdf) = TARGETS[i][t];
                let tr = r + tdr;
                let tf = f + tdf;
                if tr >= 0 && tr < Rank::COUNT as i8 && tf >= 0 && tf < File::COUNT as i8 {
                    attacks |= 1 << (tr * RANK_STRIDE + tf);
                }
                t += 1;
            }
        }
        i += 1;
    }
    attacks
}

/// Bishop attacks from `from_sq` given board occupancy `occ`.
///
/// A bishop moves exactly two squares diagonally. The intermediate "elbow"
/// square must be empty. Bishops are also confined to their own half of the
/// board (same side of the river as the source square).
pub(super) const fn bishop_attacks(from_sq: Square, occ: u128) -> u128 {
    let r = from_sq.rank() as i8;
    let f = from_sq.file() as i8;
    const DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    let mut attacks = 0u128;
    let mut i = 0;
    while i < 4 {
        let (dr, df) = DIRS[i];
        let er = r + dr; // elbow rank (one step diagonal)
        let ef = f + df; // elbow file
        let tr = r + dr * 2; // target rank (two steps diagonal)
        let tf = f + df * 2; // target file
        let on_board = tr >= 0
            && tr < Rank::COUNT as i8
            && tf >= 0
            && tf < File::COUNT as i8
            && er >= 0
            && er < Rank::COUNT as i8
            && ef >= 0
            && ef < File::COUNT as i8;
        // Bishops cannot cross the river: source and target must be in the same half.
        let same_half = (r < 5) == (tr < 5);
        // Elbow square must be unoccupied (not blocked).
        if on_board && same_half && (occ & (1 << (er * RANK_STRIDE + ef))) == 0 {
            attacks |= 1 << (tr * RANK_STRIDE + tf);
        }
        i += 1;
    }
    attacks
}

/// Squares from which a knight could attack target `sq`, given `occ`.
///
/// This is the *reverse* knight attack: "where could a knight be standing to
/// threaten `sq`?" It is used in check detection (does an enemy knight attack
/// our king?).
///
/// For each of the 8 possible knight origins `(sq + ORIGINS[i])`:
///   - The leg toward `sq` is the one-step portion of the L-shape. Its
///     direction is determined by whether `odr`/`odf` is large (|2|) or small
///     (|1|).
///   - If that leg square is empty, the origin is a valid attacker.
pub(super) const fn knight_to_attacks(sq: Square, occ: u128) -> u128 {
    let r = sq.rank() as i8;
    let f = sq.file() as i8;
    // All eight squares that could be a knight origin relative to `sq`.
    const ORIGINS: [(i8, i8); 8] = [
        (2, 1),
        (2, -1),
        (-2, 1),
        (-2, -1),
        (1, 2),
        (1, -2),
        (-1, 2),
        (-1, -2),
    ];
    let mut attacks = 0u128;
    let mut i = 0;
    while i < 8 {
        let (odr, odf) = ORIGINS[i];
        let or = r + odr; // candidate origin rank
        let of = f + odf; // candidate origin file
        if or >= 0 && or < Rank::COUNT as i8 && of >= 0 && of < File::COUNT as i8 {
            // The leg goes from `or/of` toward `sq`. Whichever axis has the
            // larger offset (|2|) is the two-step axis; the leg steps one
            // square along that axis toward `sq`.
            let leg_r = r + if odr > 0 { 1 } else { -1 };
            let leg_f = f + if odf > 0 { 1 } else { -1 };
            // Origin is only valid if its leg square is clear.
            if (occ & (1 << (leg_r * RANK_STRIDE + leg_f))) == 0 {
                attacks |= 1 << (or * RANK_STRIDE + of);
            }
        }
        i += 1;
    }
    attacks
}
