use super::*;
use crate::core::PackedScore;

#[derive(Clone, Debug)]
pub struct EvalParams {
    pub material: [PackedScore; 7], /* Indexed by PieceType (Rook, Advisor, Cannon, Pawn,
                                     * Knight, Bishop, King) */
    pub pawn_crossed: PackedScore,
    pub psts: [[PackedScore; 90]; 7], // Indexed by PieceType
    pub knight_mobility: [PackedScore; 9],
    pub rook_mobility: [PackedScore; 18],
    pub cannon_mobility: [PackedScore; 18],
    pub advisor_count_bonus: [PackedScore; 3],
    pub bishop_count_bonus: [PackedScore; 3],
}

pub static DEFAULT_PARAMS: EvalParams = EvalParams {
    material: [
        PieceMaterialValue::ROOK,    // Rook = 0
        PieceMaterialValue::ADVISOR, // Advisor = 1
        PieceMaterialValue::CANNON,  // Cannon = 2
        PieceMaterialValue::PAWN,    // Pawn = 3
        PieceMaterialValue::KNIGHT,  // Knight = 4
        PieceMaterialValue::BISHOP,  // Bishop = 5
        PackedScore::ZERO,           // King = 6
    ],
    pawn_crossed: PieceMaterialValue::PAWN_CROSSED,
    psts: [
        PIECE_SQUARE_TABLE_ROOK_TAPERED,    // Rook = 0
        PIECE_SQUARE_TABLE_ADVISOR_TAPERED, // Advisor = 1
        PIECE_SQUARE_TABLE_CANNON_TAPERED,  // Cannon = 2
        PIECE_SQUARE_TABLE_PAWN_TAPERED,    // Pawn = 3
        PIECE_SQUARE_TABLE_KNIGHT_TAPERED,  // Knight = 4
        PIECE_SQUARE_TABLE_BISHOP_TAPERED,  // Bishop = 5
        PIECE_SQUARE_TABLE_KING_TAPERED,    // King = 6
    ],
    knight_mobility: KNIGHT_MOBILITY_BONUS,
    rook_mobility: ROOK_MOBILITY_BONUS,
    cannon_mobility: CANNON_MOBILITY_BONUS,
    advisor_count_bonus: ADVISOR_COUNT_BONUS,
    bishop_count_bonus: BISHOP_COUNT_BONUS,
};

impl Default for EvalParams {
    fn default() -> Self {
        DEFAULT_PARAMS.clone()
    }
}

impl EvalParams {
    pub fn to_vector(&self) -> Vec<i32> {
        let mut v = Vec::with_capacity(1378);

        // Material
        for m in &self.material {
            v.push(m.mg);
            v.push(m.eg);
        }
        v.push(self.pawn_crossed.mg);
        v.push(self.pawn_crossed.eg);

        // PSTs
        for type_idx in 0..7 {
            for sq in 0..90 {
                let p = self.psts[type_idx][sq];
                v.push(p.mg);
                v.push(p.eg);
            }
        }

        // Mobility
        for m in &self.knight_mobility {
            v.push(m.mg);
            v.push(m.eg);
        }
        for m in &self.rook_mobility {
            v.push(m.mg);
            v.push(m.eg);
        }
        for m in &self.cannon_mobility {
            v.push(m.mg);
            v.push(m.eg);
        }

        // Defender bonuses
        for m in &self.advisor_count_bonus {
            v.push(m.mg);
            v.push(m.eg);
        }
        for m in &self.bishop_count_bonus {
            v.push(m.mg);
            v.push(m.eg);
        }

        v
    }

    pub fn update_from_vector(&mut self, v: &[i32]) {
        let mut idx = 0;

        // Material
        for m in &mut self.material {
            m.mg = v[idx];
            m.eg = v[idx + 1];
            idx += 2;
        }
        self.pawn_crossed.mg = v[idx];
        self.pawn_crossed.eg = v[idx + 1];
        idx += 2;

        // PSTs
        for type_idx in 0..7 {
            for sq in 0..90 {
                self.psts[type_idx][sq].mg = v[idx];
                self.psts[type_idx][sq].eg = v[idx + 1];
                idx += 2;
            }
        }

        // Mobility
        for m in &mut self.knight_mobility {
            m.mg = v[idx];
            m.eg = v[idx + 1];
            idx += 2;
        }
        for m in &mut self.rook_mobility {
            m.mg = v[idx];
            m.eg = v[idx + 1];
            idx += 2;
        }
        for m in &mut self.cannon_mobility {
            m.mg = v[idx];
            m.eg = v[idx + 1];
            idx += 2;
        }

        // Defender bonuses
        for m in &mut self.advisor_count_bonus {
            m.mg = v[idx];
            m.eg = v[idx + 1];
            idx += 2;
        }
        for m in &mut self.bishop_count_bonus {
            m.mg = v[idx];
            m.eg = v[idx + 1];
            idx += 2;
        }
    }
}
