use crate::{core::Move, search::MAX_PLY};
use arrayvec::ArrayVec;

pub struct PrincipalVariationTable {
    /// The last row stores the empty move
    data: [ArrayVec<Move, MAX_PLY>; MAX_PLY + 1],
}

impl Default for PrincipalVariationTable {
    fn default() -> Self {
        Self::new()
    }
}

impl PrincipalVariationTable {
    pub fn new() -> Self {
        Self {
            data: std::array::from_fn(|_| ArrayVec::new()),
        }
    }
    pub fn get_line(&self, ply: u8) -> &[Move] {
        &self.data[ply as usize]
    }

    pub fn clear_line(&mut self, ply: u8) {
        self.data[ply as usize].clear();
    }

    pub fn update_best_move(&mut self, ply: u8, best_move: Move) {
        let ply = ply as usize;
        let (left, right) = self.data.split_at_mut(ply + 1);
        let current_line = &mut left[ply];
        let next_line = &right[0];
        current_line.clear();
        current_line.push(best_move);
        current_line.extend(next_line.iter().cloned());
    }
}

impl super::Searcher<'_> {
    pub(super) fn pv_search<const PV: bool>(
        &mut self,
        depth: i8,
        ply: u8,
        alpha: i32,
        beta: i32,
    ) -> i32 {
        let mut score = -self.negamax::<false, false>(
            depth - 1,
            ply + 1,
            -alpha - 1,
            -alpha,
            Default::default(),
        );

        // If the search fails, we need to re-search with the full window to get the correct score and PV.
        if alpha < score && score < beta {
            score =
                -self.negamax::<false, PV>(depth - 1, ply + 1, -beta, -alpha, Default::default());
        }

        score
    }
}

#[cfg(test)]
mod tests {
    use crate::core::Square;

    use super::*;

    #[test]
    fn test_initialization() {
        let pvt = PrincipalVariationTable::default();
        assert!(pvt.get_line(0).is_empty());
        assert!(pvt.get_line(10).is_empty());
    }

    #[test]
    fn test_clear_line() {
        let mut pvt = PrincipalVariationTable::default();

        pvt.update_best_move(0, Move::new(Square::A1, Square::A2));
        assert_eq!(pvt.get_line(0).len(), 1);

        pvt.clear_line(0);
        assert!(pvt.get_line(0).is_empty());
    }

    #[test]
    fn test_update_best_move_leaf_node() {
        let mut pvt = PrincipalVariationTable::default();

        pvt.update_best_move(5, Move::new(Square::A1, Square::A2));

        assert_eq!(pvt.get_line(5), &[Move::new(Square::A1, Square::A2)]);
    }

    #[test]
    fn test_triangular_pv_propagation() {
        let mut pvt = PrincipalVariationTable::default();

        // 1. Terminal node at ply 2 finds a best move
        pvt.update_best_move(2, Move::new(Square::C1, Square::C2));
        assert_eq!(pvt.get_line(2), &[Move::new(Square::C1, Square::C2)]);

        // 2. Node at ply 1 evaluates its children, finds a move that leads to the ply 2 move
        pvt.update_best_move(1, Move::new(Square::B1, Square::B2));
        assert_eq!(
            pvt.get_line(1),
            &[
                Move::new(Square::B1, Square::B2),
                Move::new(Square::C1, Square::C2)
            ]
        );

        // 3. Root node at ply 0 evaluates children, finds best move
        pvt.update_best_move(0, Move::new(Square::A1, Square::A2));
        assert_eq!(
            pvt.get_line(0),
            &[
                Move::new(Square::A1, Square::A2),
                Move::new(Square::B1, Square::B2),
                Move::new(Square::C1, Square::C2)
            ]
        );
    }

    #[test]
    fn test_pv_overwrite_on_new_best_move() {
        let mut pvt = PrincipalVariationTable::default();

        // Setup initial PV: A1->A2, B1->B2, C1->C2
        pvt.update_best_move(2, Move::new(Square::C1, Square::C2));
        pvt.update_best_move(1, Move::new(Square::B1, Square::B2));
        pvt.update_best_move(0, Move::new(Square::A1, Square::A2));

        // Suppose a later branch finds a new best line starting at ply 1
        pvt.update_best_move(2, Move::new(Square::E1, Square::E2));
        pvt.update_best_move(1, Move::new(Square::D1, Square::D2));

        // Ply 1 should now reflect the new line, completely overwriting the B/C sequence
        assert_eq!(
            pvt.get_line(1),
            &[
                Move::new(Square::D1, Square::D2),
                Move::new(Square::E1, Square::E2)
            ]
        );

        // Ply 0 still holds the old line until `update_best_move` is called for it
        assert_eq!(
            pvt.get_line(0),
            &[
                Move::new(Square::A1, Square::A2),
                Move::new(Square::B1, Square::B2),
                Move::new(Square::C1, Square::C2)
            ]
        );

        // Update Root with a new best move that flows into the new ply 1 line
        pvt.update_best_move(0, Move::new(Square::A2, Square::A3));
        assert_eq!(
            pvt.get_line(0),
            &[
                Move::new(Square::A2, Square::A3),
                Move::new(Square::D1, Square::D2),
                Move::new(Square::E1, Square::E2)
            ]
        );
    }
}
