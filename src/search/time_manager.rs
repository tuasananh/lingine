use std::time::{Duration, Instant};

use crate::{core::Side, search::MAX_DEPTH, uci::GoParameters};

pub struct TimeManager {
    soft_time_limit: Option<Duration>,
    hard_time_limit: Option<Duration>,
    max_depth: i8,
    start_time: Instant,
    max_nodes: u64,
}

impl TimeManager {
    pub const TIME_OVERHEAD: Duration = Duration::from_millis(15);
    pub const HARD_BOUND: u32 = 8;
    pub const SOFT_BOUND: u32 = 40;

    pub fn new(limits: &GoParameters, side: Side) -> Self {
        let (time_left, inc) = match side {
            Side::Red => (limits.wtime, limits.winc),
            Side::Black => (limits.btime, limits.binc),
        };
        let (soft, hard) = if limits.infinite {
            (None, None)
        } else if let Some(movetime) = limits.movetime {
            (
                Some(movetime.saturating_sub(Self::TIME_OVERHEAD)),
                Some(movetime),
            )
        } else if let Some(time) = time_left {
            let inc_val = inc.unwrap_or(Duration::ZERO);

            if let Some(movestogo) = limits.movestogo {
                let time = time / movestogo.get() + inc_val;
                let soft = time.saturating_sub(Self::TIME_OVERHEAD);
                (Some(soft), Some(time))
            } else {
                let total = time + inc_val;
                (
                    Some(total / Self::SOFT_BOUND),
                    Some(total / Self::HARD_BOUND),
                )
            }
        } else {
            (None, None)
        };

        Self {
            soft_time_limit: soft,
            hard_time_limit: hard,
            max_depth: limits.depth.unwrap_or(MAX_DEPTH as u32).min(i8::MAX as u32) as i8,
            start_time: Instant::now(),
            max_nodes: limits.nodes.unwrap_or(u64::MAX),
        }
    }

    pub fn is_hard_bound_reached(&self) -> bool {
        if let Some(hard_limit) = self.hard_time_limit {
            return self.start_time.elapsed() >= hard_limit;
        }
        false
    }

    pub fn is_soft_bound_reached(&self) -> bool {
        if let Some(soft_limit) = self.soft_time_limit {
            return self.start_time.elapsed() >= soft_limit;
        }
        false
    }

    pub fn max_depth(&self) -> i8 {
        self.max_depth
    }

    pub fn max_nodes(&self) -> u64 {
        self.max_nodes
    }

    pub fn executed_time(&self) -> Duration {
        self.start_time.elapsed()
    }
}
