mod defender_bonus;
pub mod eval_params;
mod mobility_tables;
mod piece_material_value;
mod piece_square_tables;
use defender_bonus::*;
pub use eval_params::*;
pub use mobility_tables::*;
pub use piece_material_value::*;
pub use piece_square_tables::*;

use strum::EnumCount;

use crate::core::{PackedScore, PieceType, Position, Score, Square};

macro_rules! packed {
    ($( ($x:expr, $y:expr) ),* $(,)?) => {
        [$(PackedScore::new($x, $y)),*]
    };

    ($x:expr, $y:expr) => {
        PackedScore::new($x, $y)
    };
}

pub(in crate::eval) use packed;

// Phase Constants (32-Point Model)
pub const PHASE_ROOK: u8 = 2; // Chariot
pub const PHASE_CANNON: u8 = 2; // Cannon
pub const PHASE_KNIGHT: u8 = 2; // Horse
pub const PHASE_ADVISOR: u8 = 1; // Advisor
pub const PHASE_BISHOP: u8 = 1; // Elephant
pub const PHASE_PAWN: u8 = 0; // Pawn
pub const PHASE_KING: u8 = 0; // King

pub const MAX_PHASE: u8 =
    (PHASE_ROOK * 2 + PHASE_CANNON * 2 + PHASE_KNIGHT * 2 + PHASE_ADVISOR * 2 + PHASE_BISHOP * 2)
        * 2; // 32

/// Blends Middlegame and Endgame scores based on the current game phase.
/// Performs signed round-to-nearest integer division.
#[inline]
fn get_tapered_score(score: PackedScore, current_phase: u8) -> Score {
    const MAX_PHASE_SCORE: Score = MAX_PHASE as Score;
    let phase = current_phase.clamp(0, MAX_PHASE) as Score;
    let sum = score.mg * phase as Score + score.eg * (MAX_PHASE_SCORE - phase) as Score;

    if sum >= 0 {
        (sum + MAX_PHASE_SCORE / 2) / MAX_PHASE_SCORE
    } else {
        (sum - MAX_PHASE_SCORE / 2) / MAX_PHASE_SCORE
    }
}

/// Helper to calculate the current phase using the engine's Bitboard wrappers.
#[inline]
pub fn calculate_phase(pos: &Position) -> u8 {
    pos.piece_type_count(PieceType::Rook) * PHASE_ROOK
        + pos.piece_type_count(PieceType::Cannon) * PHASE_CANNON
        + pos.piece_type_count(PieceType::Knight) * PHASE_KNIGHT
        + pos.piece_type_count(PieceType::Advisor) * PHASE_ADVISOR
        + pos.piece_type_count(PieceType::Bishop) * PHASE_BISHOP
}

/// Gets the phase weight of a specific piece type.
#[inline]
pub const fn piece_phase_weight(pt: PieceType) -> u8 {
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

/// Get the complete evaluation score of the current position from Red's
/// perspective. Blends Middlegame and Endgame scores when compile-time flag is
/// enabled, otherwise uses static evaluation.
#[inline]
pub fn evaluate(pos: &Position) -> Score {
    let base_score = pos.score();
    let mobility_score = compute_mobility_score(pos);
    let defender_score = compute_defender_bonus(pos);
    get_tapered_score(base_score + mobility_score + defender_score, pos.phase())
}

#[inline]
pub fn tapered_score_from_scratch(pos: &Position) -> PackedScore {
    let mut score = PackedScore::ZERO;
    for sq_idx in 0..Square::COUNT as u8 {
        let sq = Square::from_repr(sq_idx).unwrap();
        if let Some(piece) = pos.piece_at(sq) {
            let color = piece.color();
            let val = piece_material_value_tapered(piece, sq);
            let pst = piece_square_table_value_tapered(piece.piece_type(), color, sq);
            let piece_total = val + pst;
            score += color.signum() * piece_total;
        }
    }
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
        eval::{evaluate, tapered_score_from_scratch},
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
                    let expected = tapered_score_from_scratch(&pos);
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
