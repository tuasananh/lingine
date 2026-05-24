use crate::{
    position::Position,
    types::{Color, File, Move, MoveGenType, MoveList, Piece, PieceType, Rank, Square},
};

pub fn generate_moves(pos: &Position, gen_type: MoveGenType) -> MoveList {
    let mut pseudo_moves = MoveList::new();
    let color = pos.side_to_move();
    let is_white = color == Color::White;

    for from_idx in 0..90 {
        let piece = pos.piece_at(Square::from_repr(from_idx as u8).unwrap());
        if piece == Piece::NoPiece || piece.color() != Some(color) {
            continue;
        }

        let from_sq = Square::from_repr(from_idx as u8).unwrap();
        let f = (from_idx as i8) % 9;
        let r = (from_idx as i8) / 9;

        match piece.piece_type() {
            PieceType::King => {
                let king_dirs = [(0, 1), (0, -1), (1, 0), (-1, 0)];
                for &(df, dr) in &king_dirs {
                    let nf = f + df;
                    let nr = r + dr;
                    if (3..=5).contains(&nf) && (if is_white { 0..=2 } else { 7..=9 }).contains(&nr) {
                        let to_sq = Square::from_file_rank(File::from_repr(nf as u8).unwrap(), Rank::from_repr(nr as u8).unwrap());
                        let target = pos.piece_at(to_sq);
                        if target == Piece::NoPiece || target.color() != Some(color) {
                            pseudo_moves.push(Move::new(from_sq, to_sq));
                        }
                    }
                }
            }
            PieceType::Advisor => {
                let advisor_dirs = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
                for &(df, dr) in &advisor_dirs {
                    let nf = f + df;
                    let nr = r + dr;
                    if (3..=5).contains(&nf) && (if is_white { 0..=2 } else { 7..=9 }).contains(&nr) {
                        let to_sq = Square::from_file_rank(File::from_repr(nf as u8).unwrap(), Rank::from_repr(nr as u8).unwrap());
                        let target = pos.piece_at(to_sq);
                        if target == Piece::NoPiece || target.color() != Some(color) {
                            pseudo_moves.push(Move::new(from_sq, to_sq));
                        }
                    }
                }
            }
            PieceType::Bishop => {
                let elephant_jumps = [
                    (2, 2, 1, 1),
                    (2, -2, 1, -1),
                    (-2, 2, -1, 1),
                    (-2, -2, -1, -1),
                ];
                for &(df, dr, lf, lr) in &elephant_jumps {
                    let nf = f + df;
                    let nr = r + dr;
                    if nf >= 0 && nf < 9 && (if is_white { 0..=4 } else { 5..=9 }).contains(&nr) {
                        let mid_sq = Square::from_file_rank(File::from_repr((f + lf) as u8).unwrap(), Rank::from_repr((r + lr) as u8).unwrap());
                        if pos.is_empty(mid_sq) {
                            let to_sq = Square::from_file_rank(File::from_repr(nf as u8).unwrap(), Rank::from_repr(nr as u8).unwrap());
                            let target = pos.piece_at(to_sq);
                            if target == Piece::NoPiece || target.color() != Some(color) {
                                pseudo_moves.push(Move::new(from_sq, to_sq));
                            }
                        }
                    }
                }
            }
            PieceType::Knight => {
                let horse_jumps = [
                    (1, 2, 0, 1),
                    (-1, 2, 0, 1),
                    (1, -2, 0, -1),
                    (-1, -2, 0, -1),
                    (2, 1, 1, 0),
                    (2, -1, 1, 0),
                    (-2, 1, -1, 0),
                    (-2, -1, -1, 0),
                ];
                for &(df, dr, lf, lr) in &horse_jumps {
                    let nf = f + df;
                    let nr = r + dr;
                    if nf >= 0 && nf < 9 && nr >= 0 && nr < 10 {
                        let leg_sq = Square::from_file_rank(File::from_repr((f + lf) as u8).unwrap(), Rank::from_repr((r + lr) as u8).unwrap());
                        if pos.is_empty(leg_sq) {
                            let to_sq = Square::from_file_rank(File::from_repr(nf as u8).unwrap(), Rank::from_repr(nr as u8).unwrap());
                            let target = pos.piece_at(to_sq);
                            if target == Piece::NoPiece || target.color() != Some(color) {
                                pseudo_moves.push(Move::new(from_sq, to_sq));
                            }
                        }
                    }
                }
            }
            PieceType::Pawn => {
                let (forward_dr, crossed) = if is_white { (1, r >= 5) } else { (-1, r <= 4) };
                let nr = r + forward_dr;
                if nr >= 0 && nr < 10 {
                    let to_sq = Square::from_file_rank(File::from_repr(f as u8).unwrap(), Rank::from_repr(nr as u8).unwrap());
                    let target = pos.piece_at(to_sq);
                    if target == Piece::NoPiece || target.color() != Some(color) {
                        pseudo_moves.push(Move::new(from_sq, to_sq));
                    }
                }
                if crossed {
                    for df in &[-1, 1] {
                        let nf = f + df;
                        if nf >= 0 && nf < 9 {
                            let to_sq = Square::from_file_rank(File::from_repr(nf as u8).unwrap(), Rank::from_repr(r as u8).unwrap());
                            let target = pos.piece_at(to_sq);
                            if target == Piece::NoPiece || target.color() != Some(color) {
                                pseudo_moves.push(Move::new(from_sq, to_sq));
                            }
                        }
                    }
                }
            }
            PieceType::Rook => {
                let rook_dirs = [(0, 1), (0, -1), (1, 0), (-1, 0)];
                for &(df, dr) in &rook_dirs {
                    let mut nf = f;
                    let mut nr = r;
                    loop {
                        nf += df;
                        nr += dr;
                        if nf < 0 || nf >= 9 || nr < 0 || nr >= 10 {
                            break;
                        }
                        let to_sq = Square::from_file_rank(File::from_repr(nf as u8).unwrap(), Rank::from_repr(nr as u8).unwrap());
                        let target = pos.piece_at(to_sq);
                        if target == Piece::NoPiece {
                            pseudo_moves.push(Move::new(from_sq, to_sq));
                        } else {
                            if target.color() != Some(color) {
                                pseudo_moves.push(Move::new(from_sq, to_sq));
                            }
                            break;
                        }
                    }
                }
            }
            PieceType::Cannon => {
                let cannon_dirs = [(0, 1), (0, -1), (1, 0), (-1, 0)];
                for &(df, dr) in &cannon_dirs {
                    let mut nf = f;
                    let mut nr = r;
                    let mut screen_found = false;
                    loop {
                        nf += df;
                        nr += dr;
                        if nf < 0 || nf >= 9 || nr < 0 || nr >= 10 {
                            break;
                        }
                        let to_sq = Square::from_file_rank(File::from_repr(nf as u8).unwrap(), Rank::from_repr(nr as u8).unwrap());
                        let target = pos.piece_at(to_sq);
                        if !screen_found {
                            if target == Piece::NoPiece {
                                pseudo_moves.push(Move::new(from_sq, to_sq));
                            } else {
                                screen_found = true;
                            }
                        } else {
                            if target != Piece::NoPiece {
                                if target.color() != Some(color) {
                                    pseudo_moves.push(Move::new(from_sq, to_sq));
                                }
                                break;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if gen_type == MoveGenType::PseudoLegal {
        return pseudo_moves;
    }

    let mut legal_moves = MoveList::new();
    for &m in &pseudo_moves {
        let mut temp_pos = pos.clone();
        temp_pos.do_move(m);
        if !temp_pos.is_in_check(color) {
            let is_capture = !pos.is_empty(m.square_to());
            match gen_type {
                MoveGenType::Legal | MoveGenType::Evasions => {
                    legal_moves.push(m);
                }
                MoveGenType::Captures => {
                    if is_capture {
                        legal_moves.push(m);
                    }
                }
                MoveGenType::Quiets => {
                    if !is_capture {
                        legal_moves.push(m);
                    }
                }
                _ => {}
            }
        }
    }

    legal_moves
}
