use std::sync::Arc;

use crate::core::{Move, Piece, PieceType, Position};
use crate::uci::RunningStatus;

mod aspiration_search;
mod history_moves;
mod iterative_deepening;
mod killer_moves;
mod move_ordering;
mod negamax;
mod quiescence_search;
mod time_manager;
mod transposition_table;
mod uci_info;

pub use history_moves::*;
pub use killer_moves::*;
pub use time_manager::*;
pub use transposition_table::*;

pub const MAX_PLY: usize = 128;
pub const MAX_DEPTH: usize = 64;

/// Tracks search extension and move exclusion parameters for the current
/// branch.
#[derive(Copy, Clone, Debug, Default)]
struct SearchContext {
    pub excluded_move: Move,
    pub extensions: u8,
}

pub struct SharedContext<'a> {
    /// Reference to the transposition table.
    pub transposition_table: &'a mut TranspositionTable,
    /// History heuristic table to prioritize frequently successful quiet moves.
    pub history_moves: &'a mut HistoryMoves,
    /// Current search sequence age.
    pub age: u8,
    /// Track the running status of the search, may be stopped in another
    /// thread.
    pub keep_running: Arc<RunningStatus>,
}

/// Shared context parameters passed down the recursive search stack.
pub struct Searcher<'a> {
    pos: Position,
    /// Tracks total nodes searched during this `go` invocation.
    nodes: u64,
    /// The time budget allocated for this search, if any.
    time_manager: TimeManager,
    /// Absolute time budget allowed for this search ply.
    killer_moves: KillerMoves,
    /// Tracks the maximum search depth reached, including quiescence.
    max_ply: u8,
    /// Shared context parameters passed down the recursive search stack
    shared: SharedContext<'a>,
}

impl<'a> Searcher<'a> {
    /// Starts the iterative deepening search loop
    ///
    /// Returns the best move found.
    pub fn start_search(pos: Position, time_manager: TimeManager, shared: SharedContext) -> Move {
        // Decay history table to make sure old moves do not accumulate indefinitely and
        // overshadow newer moves.
        shared.history_moves.decay();

        let is_running = shared.keep_running.clone();

        let search = Searcher {
            pos,
            nodes: 0,
            time_manager,
            killer_moves: KillerMoves::new(),
            max_ply: 0,
            shared,
        };

        is_running.set(true);
        let answer = search.iterative_deepening();
        is_running.set(false);
        answer
    }

    #[inline]
    fn update_analytics(&mut self, ply: u8) {
        self.nodes += 1;
        self.max_ply = self.max_ply.max(ply);
    }

    #[inline]
    fn should_stop_search(&self) -> bool {
        if !self.shared.keep_running.get() {
            return true;
        }
        const POLL_INTERVAL: u64 = 4096;
        // Periodically check if we have exceeded the allocated time budget to avoid
        // timing out in the middle of a ply.
        if self.nodes >= self.time_manager.max_nodes()
            || (self.nodes.is_multiple_of(POLL_INTERVAL)
                && self.time_manager.is_hard_bound_reached())
        {
            // Timed out, stop the search
            self.shared.keep_running.set(false);
            return true;
        }
        false
    }
}

/// Simple helper to rank piece types for MVV-LVA move ordering.
#[inline]
const fn get_piece_value_rank(p: Piece) -> i32 {
    match p.piece_type() {
        PieceType::King => 8,
        PieceType::Rook => 7,
        PieceType::Cannon => 6,
        PieceType::Knight => 5,
        PieceType::Advisor => 4,
        PieceType::Bishop => 3,
        PieceType::Pawn => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Move, Side, Square};
    use crate::core::{Position, score};
    use crate::uci::GoParameters;
    use std::sync::Arc;

    #[test]
    fn test_repetition_draw_repetition() {
        let mut pos = Position::new();
        // Setup a simple position with files D and E blocked by pawns to avoid
        // King-facing checks
        pos.set("4k4/9/9/9/3PP4/9/9/9/9/4K4 w - - 0 1").unwrap();
        // Setup moves to repeat: King moves back and forth
        let w_move1 = Move::new(Square::E0, Square::D0);
        let w_move2 = Move::new(Square::D0, Square::E0);
        let b_move1 = Move::new(Square::E9, Square::D9);
        let b_move2 = Move::new(Square::D9, Square::E9);

        // White moves to D0
        pos.do_move(w_move1);
        // Black moves to D9
        pos.do_move(b_move1);
        // White moves back to E0
        pos.do_move(w_move2);
        // Black moves back to E9
        pos.do_move(b_move2);

        // Now White moves to D0 again (first repetition check at ply 5)
        pos.do_move(w_move1);

        // Black moves to D9 again (repeating the state at ply 1)
        pos.do_move(b_move1);
        // This completed a repetition, neither side is in check.
        assert_eq!(pos.rule_judge(6), Some(score::ZERO));

        // Call negamax with depth=1, we should get 0 (draw)
        let mut transposition_table = TranspositionTable::new(1);
        let killers = KillerMoves::default();
        let mut history_moves = HistoryMoves::default();
        let time_manager = TimeManager::new(&GoParameters::default(), Side::Red);
        let mut ctx = Searcher {
            pos: pos.clone(),
            nodes: 0,
            time_manager,
            killer_moves: killers,
            max_ply: 0,
            shared: SharedContext {
                keep_running: Arc::new(RunningStatus::default()),
                transposition_table: &mut transposition_table,
                age: 1,
                history_moves: &mut history_moves,
            },
        };
        let score = ctx.negamax::<false>(
            1,
            6,
            -score::INFINITY,
            score::INFINITY,
            SearchContext::default(),
        );
        assert_eq!(score, 0);
    }

    #[test]
    fn test_repetition_perpetual_check_repetition() {
        let mut pos = Position::new();
        // White King at E0, White Rook at D1, Black King at D9
        pos.set("3k5/9/9/9/9/9/9/9/3R5/4K4 w - - 0 1").unwrap();

        // White Rook checks: D1 to D8 (giving check)
        let r_check1 = Move::new(Square::D1, Square::D8);
        // Black King evades: D9 to E9 (not checking)
        let k_move1 = Move::new(Square::D9, Square::E9);
        // White Rook checks again: D8 to E8 (giving check)
        let r_check2 = Move::new(Square::D8, Square::E8);
        // Black King evades: E9 to D9
        let k_move2 = Move::new(Square::E9, Square::D9);
        // White Rook checks again: E8 to D8 (giving check)
        let r_check3 = Move::new(Square::E8, Square::D8);

        // 1. White checks
        pos.do_move(r_check1);
        assert!(pos.is_in_check(Side::Black));

        // 2. Black evades
        pos.do_move(k_move1);

        // 3. White checks again
        pos.do_move(r_check2);
        assert!(pos.is_in_check(Side::Black));

        // 4. Black moves King back to D9
        pos.do_move(k_move2);

        // 5. White checks again with Rook to D8 (repetition + check!)
        pos.do_move(r_check3);

        // Now Black turn to move. White just gave the repeating check on all turns in
        // the loop.
        assert_eq!(pos.rule_judge(5), Some(score::mate_in(5)));
        assert!(pos.is_in_check(Side::Black));

        // Black should win because White is perpetually checking!
        // negamax should return a win score (MATE_VALUE - ply)
        let mut transposition_table = TranspositionTable::new(1);
        let killers = KillerMoves::default();
        let mut history_moves = HistoryMoves::default();
        let time_manager = TimeManager::new(&GoParameters::default(), Side::Red);
        let mut ctx = Searcher {
            pos,
            nodes: 0,
            time_manager,
            killer_moves: killers,
            max_ply: 0,
            shared: SharedContext {
                keep_running: Arc::new(RunningStatus::default()),
                transposition_table: &mut transposition_table,
                age: 1,
                history_moves: &mut history_moves,
            },
        };
        ctx.shared.keep_running.set(true);
        let score = ctx.negamax::<false>(
            1,
            5,
            -score::INFINITY,
            score::INFINITY,
            SearchContext::default(),
        );
        assert_eq!(score, score::mate_in(5));
    }
}
