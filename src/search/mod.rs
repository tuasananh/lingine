use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::{Move, Piece, PieceType, Position, Score};
use crate::uci::RunningStatus;

mod history_moves;
mod iterative_deepening;
mod killer_moves;
mod move_ordering;
mod negamax;
mod quiescence_search;
mod transposition_table;
mod uci_info;

pub use history_moves::*;
pub use killer_moves::*;
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

/// Shared context parameters passed down the recursive search stack.
pub struct Searcher<'a> {
    pos: Position,
    /// Track the running status of the search, may be stopped in another
    /// thread.
    keep_running: Arc<RunningStatus>,
    /// Tracks total nodes searched during this `go` invocation.
    nodes: u64,
    /// The moment when the search was started.
    start_time: Instant,
    /// Absolute time budget allowed for this search ply.
    allocated_time: Option<std::time::Duration>,
    /// Reference to the transposition table.
    transposition_table: &'a mut TranspositionTable,
    /// Current search sequence age.
    age: u8,
    /// Killer moves tracked per ply to sort high-quality quiet moves.
    killer_moves: KillerMoves,
    /// History heuristic table to prioritize frequently successful quiet moves.
    history_moves: &'a mut HistoryMoves,
    /// Tracks the maximum search depth reached, including quiescence.
    max_ply: u8,
}

pub struct SearcherParameters<'a> {
    // We take ownership of the position here since we will be modifying it during search.
    pub pos: Position,
    // The time limit for this search
    pub allocated_time: Option<Duration>,
    // The stop flag to signal search interruption from another thread
    pub keep_running: Arc<RunningStatus>,
    // The max depth to search to
    pub max_depth: i8,
    // The transposition table to use for caching search results
    //
    // See: https://www.chessprogramming.org/Transposition_Table
    pub transposition_table: &'a mut TranspositionTable,
    // The history table for quiet moves
    //
    // See: https://www.chessprogramming.org/History_Heuristic
    pub history_moves: &'a mut HistoryMoves,
    // The current search sequence age for TT entry management
    //
    // See: https://www.chessprogramming.org/Transposition_Table#Aging
    pub age: u8,
}

impl<'a> Searcher<'a> {
    /// Starts the iterative deepening search loop
    ///
    /// Returns the best move found, its score, and the total nodes searched.
    pub fn start_search(params: SearcherParameters) -> (Score, Move, u64) {
        let SearcherParameters {
            pos,
            allocated_time,
            keep_running: is_running,
            max_depth,
            transposition_table,
            history_moves,
            age,
        } = params;

        // Decay history table to make sure old moves do not accumulate indefinitely and
        // overshadow newer moves.

        history_moves.decay();

        let search = Searcher {
            pos,
            keep_running: is_running,
            nodes: 0,
            start_time: Instant::now(),
            allocated_time,
            transposition_table,
            age,
            killer_moves: KillerMoves::new(),
            history_moves,
            max_ply: 0,
        };

        search.iterative_deepening(max_depth)
    }

    #[inline]
    fn update_analytics(&mut self, ply: u8) {
        self.nodes += 1;
        self.max_ply = self.max_ply.max(ply);
    }

    #[inline]
    fn should_stop_search(&self) -> bool {
        if !self.keep_running.get() {
            return true;
        }
        // Periodically check if we have exceeded the allocated time budget to avoid
        // timing out in the middle of a ply.
        if self.nodes & 1023 == 0
            && let Some(limit) = self.allocated_time
            && self.start_time.elapsed() >= limit
        {
            // Timed out, stop the search
            self.keep_running.set(false);
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
    use std::sync::Arc;
    use std::time::Instant;

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
        let mut ctx = Searcher {
            pos: pos.clone(),
            keep_running: Arc::new(RunningStatus::default()),
            nodes: 0,
            start_time: Instant::now(),
            allocated_time: None,
            transposition_table: &mut transposition_table,
            age: 1,
            killer_moves: killers,
            history_moves: &mut history_moves,
            max_ply: 0,
        };
        let score = ctx.negamax(
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
        let mut ctx = Searcher {
            pos,
            keep_running: Arc::new(RunningStatus::default()),
            nodes: 0,
            start_time: Instant::now(),
            allocated_time: None,
            transposition_table: &mut transposition_table,
            age: 1,
            killer_moves: killers,
            history_moves: &mut history_moves,
            max_ply: 0,
        };
        ctx.keep_running.set(true);
        let score = ctx.negamax(
            1,
            5,
            -score::INFINITY,
            score::INFINITY,
            SearchContext::default(),
        );
        assert_eq!(score, score::mate_in(5));
    }
}
