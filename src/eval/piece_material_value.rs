use crate::{
    core::{PackedScore, Piece, PieceType, Side, Square},
    eval::{eval_params::EvalParams, packed},
};

/// Returns a piece's base material value in Middlegame and Endgame, dynamically
/// adjusting Pawn values based on whether they have crossed the river.
#[inline]
pub(crate) const fn piece_material_value_tapered(piece: Piece, sq: Square) -> PackedScore {
    match piece {
        Piece::RedRook | Piece::BlackRook => PieceMaterialValue::ROOK,
        Piece::RedCannon | Piece::BlackCannon => PieceMaterialValue::CANNON,
        Piece::RedKnight | Piece::BlackKnight => PieceMaterialValue::KNIGHT,
        Piece::RedBishop | Piece::BlackBishop => PieceMaterialValue::BISHOP,
        Piece::RedAdvisor | Piece::BlackAdvisor => PieceMaterialValue::ADVISOR,
        Piece::RedPawn => {
            if sq.rank() as u8 >= 5 {
                PieceMaterialValue::PAWN_CROSSED
            } else {
                PieceMaterialValue::PAWN
            }
        }
        Piece::BlackPawn => {
            if sq.rank() as u8 <= 4 {
                PieceMaterialValue::PAWN_CROSSED
            } else {
                PieceMaterialValue::PAWN
            }
        }
        Piece::RedKing | Piece::BlackKing => PackedScore::ZERO,
    }
}

#[inline]
pub(in crate::eval) fn piece_material_value_tapered_with_params(
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
            params.material[PieceType::Pawn]
        }
    } else if pt == PieceType::King {
        PackedScore::ZERO
    } else {
        params.material[pt]
    }
}

pub(in crate::eval) struct PieceMaterialValue;

impl PieceMaterialValue {
    pub const ROOK: PackedScore = packed!(979, 1027);
    pub const ADVISOR: PackedScore = packed!(185, 141);
    pub const CANNON: PackedScore = packed!(526, 423);
    pub const PAWN: PackedScore = packed!(79, 125);
    pub const KNIGHT: PackedScore = packed!(468, 462);
    pub const BISHOP: PackedScore = packed!(173, 131);
    pub const PAWN_CROSSED: PackedScore = packed!(123, 222);
}
