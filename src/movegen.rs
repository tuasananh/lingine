use crate::types::MoveList;
use std::marker::ConstParamTy;

#[derive(ConstParamTy, PartialEq, Eq)]
pub enum MoveGenType {
    Captures,
    Quiets,
    Evasions,
    PseudoLegal,
    Legal,
}

pub fn generate_moves<const T: MoveGenType>() -> MoveList {
    todo!();
}
