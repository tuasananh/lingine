mod piece_square_table;
pub use piece_square_table::*;

use crate::core::{Piece, Position, Score, Square};

/// Stores the evaluation values for Middlegame (MG) and Endgame (EG)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PackedScore {
    pub mg: i32,
    pub eg: i32,
}

impl PackedScore {
    pub const ZERO: Self = Self { mg: 0, eg: 0 };

    #[inline]
    pub const fn new(mg: i32, eg: i32) -> Self {
        Self { mg, eg }
    }
}

impl std::ops::Add for PackedScore {
    type Output = Self;
    #[inline]
    fn add(self, other: Self) -> Self {
        Self {
            mg: self.mg + other.mg,
            eg: self.eg + other.eg,
        }
    }
}

impl std::ops::Sub for PackedScore {
    type Output = Self;
    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            mg: self.mg - other.mg,
            eg: self.eg - other.eg,
        }
    }
}

impl std::ops::AddAssign for PackedScore {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        self.mg += other.mg;
        self.eg += other.eg;
    }
}

impl std::ops::SubAssign for PackedScore {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.mg -= other.mg;
        self.eg -= other.eg;
    }
}

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
pub fn get_tapered_score(mg_score: Score, eg_score: Score, current_phase: Score) -> Score {
    let phase = current_phase.clamp(0, MAX_PHASE);
    let sum = mg_score * phase + eg_score * (MAX_PHASE - phase);

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
pub const fn piece_phase_weight(pt: crate::core::PieceType) -> i32 {
    match pt {
        crate::core::PieceType::Rook => PHASE_ROOK,
        crate::core::PieceType::Cannon => PHASE_CANNON,
        crate::core::PieceType::Knight => PHASE_KNIGHT,
        crate::core::PieceType::Advisor => PHASE_ADVISOR,
        crate::core::PieceType::Bishop => PHASE_BISHOP,
        crate::core::PieceType::Pawn => PHASE_PAWN,
        crate::core::PieceType::King => PHASE_KING,
    }
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

/// Returns a piece's base material value, dynamically adjusting Pawn values
/// based on whether they have crossed the river.
#[inline]
pub const fn piece_material_value(piece: Piece, sq: Square) -> Score {
    match piece {
        Piece::RedRook | Piece::BlackRook => 600,
        Piece::RedCannon | Piece::BlackCannon => 285,
        Piece::RedKnight | Piece::BlackKnight => 270,
        Piece::RedBishop | Piece::BlackBishop => 120, // Elephant
        Piece::RedAdvisor | Piece::BlackAdvisor => 110,
        Piece::RedPawn => {
            if sq.rank() as u8 >= 5 {
                70
            } else {
                30
            }
        }
        Piece::BlackPawn => {
            if sq.rank() as u8 <= 4 {
                70
            } else {
                30
            }
        }
        Piece::RedKing | Piece::BlackKing => 0, /* Treated as 0 for incremental score (kings
                                                 * never captured) */
    }
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
                    let pre_mg = pos.mg_score();
                    let pre_eg = pos.eg_score();
                    let pre_phase = pos.phase();

                    // Do the move
                    pos.do_move(m);

                    // Compute scores & phase from scratch
                    let expected_score = pos.compute_tapered_evaluation_scores();
                    let expected_phase = pos.calculate_board_phase();

                    // Assert incremental score matches full board scan
                    assert_eq!(
                        pos.mg_score(),
                        expected_score.mg,
                        "MG score mismatch for FEN {} move {}: {}",
                        idx,
                        m_idx,
                        m
                    );
                    assert_eq!(
                        pos.eg_score(),
                        expected_score.eg,
                        "EG score mismatch for FEN {} move {}: {}",
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
                        pos.mg_score(),
                        pre_mg,
                        "MG score rollback mismatch for FEN {} move {}: {}",
                        idx,
                        m_idx,
                        m
                    );
                    assert_eq!(
                        pos.eg_score(),
                        pre_eg,
                        "EG score rollback mismatch for FEN {} move {}: {}",
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
