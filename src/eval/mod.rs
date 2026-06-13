pub mod eval_params;
mod mobility_tables;
mod piece_square_tables;
pub use eval_params::*;
pub use mobility_tables::*;
pub use piece_square_tables::*;

use strum::EnumCount;

use crate::core::{PackedScore, Piece, PieceType, Position, Score, Side, Square};

macro_rules! packed {
    ($( ($x:expr, $y:expr) ),* $(,)?) => {
        [$(PackedScore::new($x, $y)),*]
    };
}

pub(in crate::eval) use packed;

// Phase Constants (32-Point Model)
pub const PHASE_ROOK: i32 = 2; // Chariot
pub const PHASE_CANNON: i32 = 2; // Cannon
pub const PHASE_KNIGHT: i32 = 2; // Horse
pub const PHASE_ADVISOR: i32 = 1; // Advisor
pub const PHASE_BISHOP: i32 = 1; // Elephant
pub const PHASE_PAWN: i32 = 0; // Pawn
pub const PHASE_KING: i32 = 0; // King

pub const MAX_PHASE: i32 =
    (PHASE_ROOK * 2 + PHASE_CANNON * 2 + PHASE_KNIGHT * 2 + PHASE_ADVISOR * 2 + PHASE_BISHOP * 2)
        * 2; // 32

/// Blends Middlegame and Endgame scores based on the current game phase.
/// Performs signed round-to-nearest integer division.
#[inline]
pub fn get_tapered_score(score: PackedScore, current_phase: Score) -> Score {
    let phase = current_phase.clamp(0, MAX_PHASE);
    let sum = score.mg * phase + score.eg * (MAX_PHASE - phase);

    if sum >= 0 {
        (sum + MAX_PHASE / 2) / MAX_PHASE
    } else {
        (sum - MAX_PHASE / 2) / MAX_PHASE
    }
}

/// Helper to calculate the current phase from raw u128 bitboards.
#[inline]
pub const fn calculate_phase_from_raw(
    rooks: u128,
    cannons: u128,
    knights: u128,
    advisors: u128,
    bishops: u128,
) -> i32 {
    let rook_count = rooks.count_ones() as i32;
    let cannon_count = cannons.count_ones() as i32;
    let knight_count = knights.count_ones() as i32;
    let advisor_count = advisors.count_ones() as i32;
    let bishop_count = bishops.count_ones() as i32;

    rook_count * PHASE_ROOK
        + cannon_count * PHASE_CANNON
        + knight_count * PHASE_KNIGHT
        + advisor_count * PHASE_ADVISOR
        + bishop_count * PHASE_BISHOP
}

/// Helper to calculate the current phase using the engine's Bitboard wrappers.
#[inline]
pub fn calculate_phase(
    rooks: crate::core::Bitboard,
    cannons: crate::core::Bitboard,
    knights: crate::core::Bitboard,
    advisors: crate::core::Bitboard,
    bishops: crate::core::Bitboard,
) -> i32 {
    calculate_phase_from_raw(
        rooks.raw(),
        cannons.raw(),
        knights.raw(),
        advisors.raw(),
        bishops.raw(),
    )
}

/// Gets the phase weight of a specific piece type.
#[inline]
pub const fn piece_phase_weight(pt: PieceType) -> i32 {
    match pt {
        PieceType::Rook => PHASE_ROOK,
        PieceType::Cannon => PHASE_CANNON,
        PieceType::Knight => PHASE_KNIGHT,
        PieceType::Advisor => PHASE_ADVISOR,
        PieceType::Bishop => PHASE_BISHOP,
        PieceType::Pawn => PHASE_PAWN,
        PieceType::King => PHASE_KING,
    }
}

// Tapered bonuses for having 0, 1, or 2 Advisors
pub const ADVISOR_COUNT_BONUS: [PackedScore; 3] = packed![(0, 0), (0, 0), (0, 0),];

// Tapered bonuses for having 0, 1, or 2 Bishops (Elephants)
pub const BISHOP_COUNT_BONUS: [PackedScore; 3] = packed![(0, 0), (0, 0), (0, 0),];

/// Computes defender count bonuses for both sides.
#[inline]
pub fn compute_defender_bonus(pos: &Position) -> PackedScore {
    let mut score = PackedScore::ZERO;

    // Red defenders
    let red_advisors = pos.piece_count(Piece::RedAdvisor) as usize;
    let red_bishops = pos.piece_count(Piece::RedBishop) as usize;
    score += ADVISOR_COUNT_BONUS[red_advisors.min(2)];
    score += BISHOP_COUNT_BONUS[red_bishops.min(2)];

    // Black defenders
    let black_advisors = pos.piece_count(Piece::BlackAdvisor) as usize;
    let black_bishops = pos.piece_count(Piece::BlackBishop) as usize;
    score -= ADVISOR_COUNT_BONUS[black_advisors.min(2)];
    score -= BISHOP_COUNT_BONUS[black_bishops.min(2)];

    score
}

/// Returns a piece's base material value in Middlegame and Endgame, dynamically
/// adjusting Pawn values based on whether they have crossed the river.
#[inline]
pub const fn piece_material_value_tapered(piece: Piece, sq: Square) -> PackedScore {
    match piece {
        Piece::RedRook | Piece::BlackRook => PackedScore::new(600, 600),
        Piece::RedCannon | Piece::BlackCannon => PackedScore::new(285, 240),
        Piece::RedKnight | Piece::BlackKnight => PackedScore::new(270, 290),
        Piece::RedBishop | Piece::BlackBishop => PackedScore::new(120, 120),
        Piece::RedAdvisor | Piece::BlackAdvisor => PackedScore::new(110, 110),
        Piece::RedPawn => {
            if sq.rank() as u8 >= 5 {
                PackedScore::new(70, 150) // Crossed river
            } else {
                PackedScore::new(30, 30) // Uncrossed
            }
        }
        Piece::BlackPawn => {
            if sq.rank() as u8 <= 4 {
                PackedScore::new(70, 150) // Crossed river
            } else {
                PackedScore::new(30, 30) // Uncrossed
            }
        }
        Piece::RedKing | Piece::BlackKing => PackedScore::ZERO,
    }
}

/// Get the complete evaluation score of the current position from Red's
/// perspective. Blends Middlegame and Endgame scores when compile-time flag is
/// enabled, otherwise uses static evaluation.
#[inline]
pub fn evaluate(pos: &Position) -> Score {
    pos.tapered_score()
}

#[inline]
pub fn piece_material_value_tapered_with_params(
    piece: Piece,
    sq: Square,
    params: &EvalParams,
) -> PackedScore {
    let pt = piece.piece_type();
    if pt == PieceType::Pawn {
        let crossed = match piece.color() {
            Side::Red => sq.rank() as u8 >= 5,
            Side::Black => sq.rank() as u8 <= 4,
        };
        if crossed {
            params.pawn_crossed
        } else {
            params.material[PieceType::Pawn as usize]
        }
    } else if pt == PieceType::King {
        PackedScore::ZERO
    } else {
        params.material[pt as usize]
    }
}

#[inline]
pub fn compute_defender_bonus_with_params(pos: &Position, params: &EvalParams) -> PackedScore {
    let mut score = PackedScore::ZERO;

    // Red defenders
    let red_advisors = pos.piece_count(Piece::RedAdvisor) as usize;
    let red_bishops = pos.piece_count(Piece::RedBishop) as usize;
    score += params.advisor_count_bonus[red_advisors.min(2)];
    score += params.bishop_count_bonus[red_bishops.min(2)];

    // Black defenders
    let black_advisors = pos.piece_count(Piece::BlackAdvisor) as usize;
    let black_bishops = pos.piece_count(Piece::BlackBishop) as usize;
    score -= params.advisor_count_bonus[black_advisors.min(2)];
    score -= params.bishop_count_bonus[black_bishops.min(2)];

    score
}

pub fn evaluate_with_params(pos: &Position, params: &EvalParams) -> Score {
    let mut base_score = PackedScore::ZERO;
    let mut phase = 0;

    for sq_idx in 0..Square::COUNT {
        if let Some(piece) = pos.piece_at(Square::from_repr(sq_idx as u8).unwrap()) {
            let sq = Square::from_repr(sq_idx as u8).unwrap();
            let color = piece.color();
            let val = piece_material_value_tapered_with_params(piece, sq, params);
            let pst =
                piece_square_table_value_tapered_with_params(piece.piece_type(), color, sq, params);
            base_score += color.signum() * (val + pst);
            phase += piece_phase_weight(piece.piece_type());
        }
    }

    let mobility_score = compute_mobility_score_with_params(pos, params);
    let defender_score = compute_defender_bonus_with_params(pos, params);

    get_tapered_score(base_score + mobility_score + defender_score, phase)
}

#[cfg(test)]
mod tests {
    use crate::{
        core::{MoveGenType, MoveList, Position, generate_moves, score},
        eval::evaluate,
    };

    #[test]
    fn test_initial_material_evaluation_tapered() {
        // Set standard starting FEN
        let pos = Position::from_fen(
            "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1",
        )
        .unwrap();
        // Since board is symmetric, evaluation must be exactly 0
        assert_eq!(evaluate(&pos), score::DRAW);
    }

    #[test]
    fn test_incremental_evaluation_consistency_tapered() {
        let positions = [
            "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1".to_string(),
            "4k4/9/9/4P4/4p4/9/9/9/9/4K4 w - - 0 1".to_string(),
            "r1bakab1r/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1".to_string(),
        ];

        for (idx, fen) in positions.iter().enumerate() {
            let mut pos = Position::from_fen(fen).unwrap();

            // Generate all pseudo-legal moves
            let mut moves = MoveList::new();
            generate_moves(&pos, MoveGenType::PseudoLegal, &mut moves);

            for (m_idx, m) in moves.iter().copied().enumerate() {
                if pos.legal(m) {
                    let pre_score = pos.score();
                    let pre_phase = pos.phase();

                    // Do the move
                    pos.do_move(m);

                    // Compute scores & phase from scratch
                    let expected = pos.compute_tapered_evaluation_scores();
                    let expected_phase = pos.calculate_board_phase();

                    // Assert incremental score matches full board scan
                    assert_eq!(
                        pos.score(),
                        expected,
                        "Score mismatch for FEN {} move {}: {}",
                        idx,
                        m_idx,
                        m
                    );
                    assert_eq!(
                        pos.phase(),
                        expected_phase,
                        "Phase mismatch for FEN {} move {}: {}",
                        idx,
                        m_idx,
                        m
                    );

                    // Undo the move
                    pos.undo_move(m);

                    // Assert scores rolled back perfectly
                    assert_eq!(
                        pos.score(),
                        pre_score,
                        "Score rollback mismatch for FEN {} move {}: {}",
                        idx,
                        m_idx,
                        m
                    );
                    assert_eq!(
                        pos.phase(),
                        pre_phase,
                        "Phase rollback mismatch for FEN {} move {}: {}",
                        idx,
                        m_idx,
                        m
                    );
                }
            }
        }
    }
}
