# Time Management Strategy for Lingine

This document provides a comprehensive breakdown of the mathematical formulas, search termination heuristics, and Rust implementation plan for the dynamic time management (TM) system in **Lingine**. The design is inspired by the state-of-the-art clock management strategies of **Pikafish** and **Stockfish**, optimized for single-threaded Xiangqi play.

---

## 1. The Core Philosophy of Dynamic Time Management

In a naive chess engine, clock management is static:
* The engine calculates a fixed time budget per move (e.g., $\text{timeLeft} / 20$).
* The search is terminated immediately when the elapsed time exceeds this rigid boundary.

This leads to two critical inefficiencies:
1. **Wasted Search Nodes (Incomplete Plies):** Because search trees grow exponentially, a search depth typically takes $3\times$ to $4\times$ longer than all prior depths combined. If an engine stops mid-depth because it hit a static time limit, all nodes searched during that incomplete depth are discarded, resulting in massive wasted computation.
2. **Tactical Insensitivity:** A static system spends the same amount of time on a highly volatile double-check position as it does on a forced capture (recapture). In volatile positions, the engine must spend more time to avoid blundering; in simple positions, it should save time for later.

### The Pikafish Solution
Pikafish solves this by dividing time management into **two targets** and **dynamic scaling factors**:
* **Optimum Time ($\text{optimumTime}$):** The ideal target time we want to spend under normal circumstances.
* **Maximum Time ($\text{maximumTime}$):** The hard limit at which we *must* abort the search to prevent losing on time.
* **Dynamic Modifiers:** At the end of each completed iterative deepening ply, the engine scales $\text{optimumTime}$ using positional cues (evaluation shifts, move stability, node concentration) to compute a custom target $\text{totalTime}$.
* **The Next-Depth Check:** The engine only starts the next search depth if the elapsed time is $\le 26\%$ of the target. This ensures the engine rarely starts a search ply it cannot finish.

---

## 2. Mathematical Breakdown of Pikafish Time Management

### A. Game-Wide Scale Initialization
To ensure time expenditure matches the game's actual speed (bullet vs. classical), Pikafish calculates a scaling factor on the first move of the game:

$$\text{originalTimeAdjust} = 0.3356 \times \log_{10}(\text{timeLeft}_{\text{ms}}) - 0.4903$$

* This factor is clamped to a healthy range (e.g., $[0.1, 2.0]$) and **preserved across all moves** of the same game.
* For bullet games (low time), $\text{originalTimeAdjust}$ is small, forcing the engine to play fast.
* For classical games (high time), $\text{originalTimeAdjust}$ is large, allowing the engine to invest deep thought early on.

---

### B. Baseline Optimum and Maximum Calculations
Before starting the search, the baseline $\text{optimumTime}$ and $\text{maximumTime}$ are calculated based on the time control type.

#### 1. Increment and Sudden Death Time Controls (`movestogo == 0`)
First, compute the remaining playable budget, accounting for increments and a safety overhead buffer:
$$\text{timeLeft} = \max\left(1.0,\, \text{time} + \text{inc} \times (\text{mtg} - 1) - \text{overhead} \times (2 + \text{mtg})\right)$$
*(where $\text{mtg} = 50$ is the assumed moves to go).*

Next, compute logarithmic constants based on remaining game time (in seconds):
$$\text{logTimeInSec} = \log_{10}(\text{scaledTime} / 1000.0)$$
$$\text{optConstant} = \min(0.0034013 + 0.00020657 \times \text{logTimeInSec},\, 0.004536)$$
$$\text{maxConstant} = \max(3.7803 + 2.8003 \times \text{logTimeInSec},\, 2.5470)$$

Calculate the scaling percentage, using the current game move index (ply) to spend more in the middlegame:
$$\text{optScale} = \min\left(0.017244 + (\text{game\_ply} + 2.71111)^{0.43433} \times \text{optConstant},\, \frac{0.20577 \times \text{time}}{\text{timeLeft}}\right) \times \text{originalTimeAdjust}$$
$$\text{maxScale} = \min(7.002,\, \text{maxConstant} + \text{game\_ply} / 13.184)$$

Finally, establish the raw targets:
$$\text{optimumTime} = \max(1.0,\, \text{optScale} \times \text{timeLeft})$$
$$\text{maximumTime} = \max\left(\text{optimumTime},\, \min(0.8237 \times \text{time} - \text{overhead},\, \text{maxScale} \times \text{optimumTime})\right)$$

*If pondering is active, the engine adds a $+25\%$ bonus to $\text{optimumTime}$.*

#### 2. Moves-To-Go Time Controls (`movestogo > 0`)
When a specific number of moves must be played in a time window:
$$\text{optScale} = \min\left(\frac{0.88 + \text{game\_ply} / 116.4}{\text{movestogo}},\, \frac{0.88 \times \text{time}}{\text{timeLeft}}\right)$$
$$\text{maxScale} = 1.3 + 0.11 \times \text{movestogo}$$

---

### C. Dynamic Modifiers (Inter-Ply Adjustments)
At the end of each completed iterative deepening depth, the engine calculates the customized position-specific time budget:

$$\text{totalTime} = \text{optimumTime} \times \text{fallingEval} \times \text{reduction} \times \text{bestMoveInstability} \times \text{highBestMoveEffort}$$

#### 1. Falling Evaluation Heuristic ($\text{fallingEval}$)
If the position's evaluation drops significantly, the engine assumes it is facing a threat and extends its thinking time to find a defensive resource:
* Let $V_{\text{current}}$ be the evaluation of the depth just completed.
* Let $V_{\text{prev\_avg}}$ be the average evaluation of the prior depth.
* Let $V_{\text{iter}}[i]$ be the historical evaluations of the last few plies.
* Compute evaluation shifts:
  $$\Delta_1 = V_{\text{prev\_avg}} - V_{\text{current}}$$
  $$\Delta_2 = V_{\text{iter}}[\text{index}] - V_{\text{current}}$$
* Calculate multiplier:
  $$\text{fallingEval} = \text{clamp}\left(\frac{16.93 + 2.73 \times \Delta_1 + 0.8 \times \Delta_2}{100.0},\, 0.610,\, 1.860\right)$$

#### 2. Best Move Stability Heuristic ($\text{reduction}$)
If the best move at the root has remained unchanged for many depths, the engine can trust it and save time:
* Compute the difference: $\text{stabilityDiff} = \text{currentDepth} - \text{lastBestMoveChangedDepth}$.
* Interpolate the scaling factor (clamping between $0.67$ and $1.44$):
  $$\text{timeReduction} = \text{clamp}(\text{interpolate}(\text{stabilityDiff},\, 8.0,\, 17.0,\, 0.67,\, 1.44),\, 0.67,\, 1.44)$$
* Calculate multiplier:
  $$\text{reduction} = \frac{2.10 + \text{previousTimeReduction}}{2.480 \times \text{timeReduction}}$$
* High stability results in a small $\text{reduction}$ value, terminating search early.

#### 3. Best Move Instability Heuristic ($\text{bestMoveInstability}$)
If the best move keeps shifting across search depths, it implies a highly complex tactical position. The engine scales up the budget:
* In multi-threaded engines, this uses helper thread transitions. In single-threaded Lingine, we scale it based on how frequently the root best move has changed:
  $$\text{bestMoveInstability} = 0.960 + 1.63 \times \text{bestMoveChanges}$$
* Instability can scale the budget up by $\ge 2\times$.

#### 4. High Best Move Effort Heuristic ($\text{highBestMoveEffort}$)
If the best move is highly forced or obvious, it will consume almost all search effort (nodes). The engine scales down the budget because alternative moves don't deserve deep verification:
* Let $\text{nodesEffort} = \frac{\text{bestMoveNodes} \times 100000}{\text{totalNodes}}$
* Interpolate and clamp:
  $$\text{highBestMoveEffort} = \text{clamp}(\text{interpolate}(\text{nodesEffort},\, 78000,\, 94000,\, 0.960,\, 0.74),\, 0.74,\, 0.960)$$
* If effort exceeds $94\%$, time reduces to $0.74\times$.

---

## 3. Concrete Rust Implementation Plan for Lingine

We will integrate this time management system into Lingine without complicating its clean single-threaded architecture.

### File 1: Creating `src/bot/time_manager.rs`
This module encapsulates all clock math.

```rust
use std::time::{Duration, Instant};
use crate::core::types::Color;

/// Dynamic Time Manager implementing Pikafish/Stockfish time allocation rules.
#[derive(Debug, Clone)]
pub struct TimeManager {
    /// Moment when the search started for this move.
    start_time: Instant,
    /// Ideal thinking time target.
    optimum_time: Duration,
    /// Absolute maximum thinking time ceiling.
    maximum_time: Duration,
    /// Persistent scaling factor calibrated to the game's time control.
    original_time_adjust: f64,
}

impl Default for TimeManager {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            optimum_time: Duration::ZERO,
            maximum_time: Duration::ZERO,
            original_time_adjust: -1.0, // Uninitialized indicator
        }
    }
}

impl TimeManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset game scaling factor. Call this at the start of a new game (`ucinewgame`).
    pub fn reset_game(&mut self) {
        self.original_time_adjust = -1.0;
    }

    /// Initialise the optimum and maximum boundaries before search begins.
    pub fn init(
        &mut self,
        wtime: Option<Duration>,
        btime: Option<Duration>,
        winc: Option<Duration>,
        binc: Option<Duration>,
        movestogo: Option<std::num::NonZeroU32>,
        us: Color,
        game_ply: u32,
    ) {
        self.start_time = Instant::now();

        // 1. Determine base times
        let time_left = match us {
            Color::White => wtime.unwrap_or(Duration::from_secs(300)),
            Color::Black => btime.unwrap_or(Duration::from_secs(300)),
        };
        let inc = match us {
            Color::White => winc.unwrap_or(Duration::ZERO),
            Color::Black => binc.unwrap_or(Duration::ZERO),
        };

        let move_overhead = Duration::from_millis(50); // Clock latency protection buffer
        let time_left_ms = time_left.as_millis() as f64;

        // 2. Initialize or preserve the game-wide scaling factor
        if self.original_time_adjust < 0.0 {
            self.original_time_adjust = (0.3356 * time_left_ms.log10() - 0.4903).clamp(0.1, 2.0);
        }

        // 3. Compute available time horizon (mtg = moves to go)
        let mtg = movestogo.map_or(50, |n| n.get().min(50)) as f64;
        let time_left_budget = (time_left + inc * (mtg as u32 - 1))
            .saturating_sub(move_overhead * (2 + mtg as u32));
        let time_left_budget_ms = (time_left_budget.as_millis() as f64).max(1.0);

        let opt_scale;
        let max_scale;

        if movestogo.is_none() {
            // Sudden death / standard increment calculations
            let log_time_in_sec = (time_left_ms / 1000.0).log10();
            let opt_constant = (0.0034013 + 0.00020657 * log_time_in_sec).min(0.004536);
            let max_constant = (3.7803 + 2.8003 * log_time_in_sec).max(2.5470);

            opt_scale = (0.017244 + (game_ply as f64 + 2.71111).powf(0.43433) * opt_constant)
                .min(0.20577 * time_left_ms / time_left_budget_ms)
                * self.original_time_adjust;

            max_scale = (max_constant + game_ply as f64 / 13.184).min(7.002);
        } else {
            // Moves-to-go calculations
            opt_scale = ((0.88 + game_ply as f64 / 116.4) / mtg)
                .min(0.88 * time_left_ms / time_left_budget_ms);
            max_scale = 1.3 + 0.11 * mtg;
        }

        // 4. Set final bounds
        let opt_ms = (opt_scale * time_left_budget_ms).max(1.0);
        self.optimum_time = Duration::from_millis(opt_ms as u64);

        let max_ms = opt_ms.max(
            (0.8237 * time_left_ms - move_overhead.as_millis() as f64)
                .min(max_scale * opt_ms)
        );
        self.maximum_time = Duration::from_millis(max_ms as u64);
    }

    pub fn optimum(&self) -> Duration {
        self.optimum_time
    }

    pub fn maximum(&self) -> Duration {
        self.maximum_time
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}
```

---

### File 2: Integrating inside `SearchContext` (`src/search/mod.rs`)
We add the hard `maximum_time` restriction down our search tree. 

1. Update `SearchContext` to hold the hard maximum limit:
```rust
pub struct SearchContext<'a> {
    pub stop: &'a Arc<AtomicBool>,
    pub nodes: &'a mut u64,
    pub start_time: Instant,
    /// Change from single time_limit to maximum_time
    pub maximum_time: Option<std::time::Duration>,
    pub transposition_table: &'a mut TranspositionTable,
    pub age: u8,
    pub killers: &'a mut [[Move; 2]; 128],
    pub history_table: &'a mut [[[i32; 90]; 90]; 2],
}
```

2. Inside `negamax()` and `quiescence()` periodic node checks (every 1024 nodes):
```rust
    // Check stop signals periodically
    if *ctx.nodes & 1023 == 0 {
        if ctx.stop.load(Ordering::Relaxed) {
            return 0;
        }
        if let Some(max_limit) = ctx.maximum_time
            && ctx.start_time.elapsed() >= max_limit
        {
            ctx.stop.store(true, Ordering::Relaxed);
            return 0;
        }
    }
```

---

### File 3: Engine Bot search loop update (`src/bot/engine_bot.rs`)
This is where the dynamic stopping calculations and next-depth decisions are enforced.

We update `EngineBot` to carry a `TimeManager` instance:
```rust
pub struct EngineBot {
    position: Position,
    transposition_table: TranspositionTable,
    age: u8,
    /// Preserved time management state across game moves
    time_manager: TimeManager, 
}
```

Make sure to reset it on `ucinewgame`:
```rust
    fn ucinewgame(&mut self) {
        self.position = Position::new();
        self.transposition_table.clear();
        self.age = 0;
        self.time_manager.reset_game();
    }
```

Update the main search loop inside the `go()` method:
```rust
    fn go(&mut self, params: GoParameters, tx: Sender<UciInfo>) -> Result<BestMove> {
        let mut pos = self.position.clone();
        let start_time = Instant::now();
        let mut nodes = 0u64;

        let max_depth = params.depth.unwrap_or(100) as i32;

        // 1. Initialize clock limits using the TimeManager
        let use_tm = params.wtime.is_some() || params.btime.is_some();
        if use_tm {
            self.time_manager.init(
                params.wtime,
                params.btime,
                params.winc,
                params.binc,
                params.movestogo,
                pos.side_to_move(),
                pos.game_ply() as u32,
            );
        }

        self.age = self.age.wrapping_add(1);

        let mut best_move = Move::none();
        let mut last_depth_score = 0;
        let mut iter_values = [0; 4]; // Store last 4 scores to compute falling eval
        let mut last_best_move_depth = 1;
        let mut best_move_changes = 0.0;
        let mut prev_time_reduction = 1.0;

        let mut killers = [[Move::none(); 2]; 128];
        let mut history_table = [[[0; 90]; 90]; 2];

        for depth in 1..=max_depth {
            if params.stop.load(Ordering::Relaxed) {
                break;
            }

            // 2. Soft-stop & next-depth decision
            if use_tm && depth > 1 {
                let elapsed = self.time_manager.elapsed();
                
                // Calculate Dynamic Target (total_time)
                
                // A. Falling Eval Heuristic
                let prev_avg = (iter_values.iter().sum::<i32>() as f64 / 4.0) as i32;
                let delta_1 = prev_avg - last_depth_score;
                let delta_2 = iter_values[(depth as usize) & 3] - last_depth_score;
                let falling_eval = ((16.93 + 2.73 * delta_1 as f64 + 0.8 * delta_2 as f64) / 100.0)
                    .clamp(0.610, 1.860);

                // B. Best Move Stability Heuristic
                let stability_diff = (depth - last_best_move_depth) as f64;
                let time_reduction = if stability_diff <= 8.0 {
                    0.67
                } else if stability_diff >= 17.0 {
                    1.44
                } else {
                    0.67 + (stability_diff - 8.0) * (1.44 - 0.67) / (17.0 - 8.0)
                };
                let reduction = (2.10 + prev_time_reduction) / (2.480 * time_reduction);
                prev_time_reduction = time_reduction;

                // C. Move Instability Heuristic
                let best_move_instability = 0.960 + 1.63 * best_move_changes;

                // Combine to form final target time
                let optimum = self.time_manager.optimum().as_millis() as f64;
                let total_time_ms = optimum * falling_eval * reduction * best_move_instability;
                let total_time = Duration::from_millis(total_time_ms as u64);

                let hard_max = self.time_manager.maximum();

                // Soft stop check
                if elapsed > total_time.min(hard_max) {
                    break;
                }

                // Next-depth decision check:
                // Abort before starting a ply that we don't have time to complete.
                let next_depth_threshold = total_time_ms * 0.26;
                if elapsed.as_millis() as f64 > next_depth_threshold {
                    break;
                }
            }

            // [Search Execution Logic - negamax Aspiration loops...]
            let max_time_param = if use_tm { Some(self.time_manager.maximum()) } else { None };
            
            // Perform actual depth search...
            // (If the search finishes cleanly, update best_move, score tracking)
            let mut ctx = SearchContext {
                stop: &params.stop,
                nodes: &mut nodes,
                start_time,
                maximum_time: max_time_param, // Pass down hard limit
                transposition_table: &mut self.transposition_table,
                age: self.age,
                killers: &mut killers,
                history_table: &mut history_table,
            };

            // [Perform Aspiration Negamax loop...]
            // Let's assume search returns `best_score` and `depth_best_move`
            
            if !params.stop.load(Ordering::Relaxed) {
                // Update move change tracking
                if !depth_best_move.is_none() && depth_best_move != best_move {
                    best_move_changes = (best_move_changes + 1.0).min(3.0);
                    last_best_move_depth = depth;
                } else {
                    best_move_changes = (best_move_changes - 0.5).max(0.0);
                }

                if !depth_best_move.is_none() {
                    best_move = depth_best_move;
                }
                
                last_depth_score = best_score;
                iter_values[(depth as usize) & 3] = best_score;

                // [UCI Info stream reporting...]
            }
        }

        // Return best move...
    }
```

---

## 4. Verification and Testing

To verify that the dynamic time management improves both efficiency and tactical safety under tight time limits:

1. **Unit Verification:**
   Ensure `TimeManager` calculations yield mathematically correct bounds for blitz (`wtime = 10000ms`, `winc = 100ms`) and classical (`wtime = 3600000ms`, `winc = 10000ms`) bounds.
2. **Clock Safety Run:**
   Confirm that the engine never suffers a time loss (exceeding `wtime`) across a 100-game match on low time constraints.
3. **Head-to-Head Gauntlet:**
   Run a head-to-head match using `sylvan-cli` comparing **Lingine (Dynamic TM)** vs **Lingine (Naive TM)**.
   * **Time Control:** 5 seconds + 0.05 seconds increment.
   * **Goal:** Verify that the Dynamic TM version achieves a positive ELO delta, spends less time on simple positions, and saves enough time to out-search the naive bot during critical tactical plies.
