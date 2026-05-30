# Lingine — Engine Development Plan

> **Target**: ~2500 ELO Xiangqi engine in Rust  
> **Protocol**: UCI (UCCI wrapper later if needed for Chinese GUIs)  
> **Inspiration**: Pikafish / Stockfish architecture adapted for Xiangqi

---

## Decisions Agreed On

| Topic                    | Decision                                                                                                                                            |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Move generation dispatch | Separate fns: `generate_legal`, `generate_captures`, `generate_quiets`, `generate_evasions`                                                         |
| Sliding piece generation | On-the-fly ray scanning first; precomputed Rank/File Occupancy Lookup later after profiling (no magic bitboards needed for orthogonal-only sliding) |
| Enum const generics      | Refactored to stable using standard `bool` const generics (`<const IS_WHITE: bool>`)                                                                |
| Search                   | Alpha-Beta + Iterative Deepening + Perpetual Check/Chase detection                                                                                  |
| Evaluation               | Material + PST (Eleeye values to start) + Mobility (updated incrementally as Pikafish-style accumulators inside `StateInfo` history stack)          |
| Search enhancements      | Transposition Table + move ordering (TT/MVV-LVA/killers/history via Move flags) + null move pruning                                                 |
| Protocol                 | UCI only (`uci`/`uciok`, standard move notation `a0b1`)                                                                                             |
| UCCI                     | Thin wrapper later if Chinese GUI support is needed (≈1 day effort)                                                                                 |
| Threading                | Single-threaded first; Lazy SMP after search is stable                                                                                              |
| Evaluation tuning        | Bootstrap from Eleeye values → SPSA/Texel tuning to reach 2400+                                                                                     |
| Testing framework        | Perft tests per piece type + unit tests; sylvan-cli gauntlet vs Fairy-Stockfish for ELO                                                             |
| Move encoding            | `u16` encoding: 7 bits from, 7 bits to, 2 bits for flags (Capture, Quiet, Check) for cache-friendly sorting; query captures via board               |
| Board undo state         | Track backtracking using `StateInfo` struct (captured piece, previous hash, rule50, check status, evaluation accumulators)                          |
| FEN parsing              | Robust parsing dynamically detecting token counts (4 vs 6 tokens) for correct halfmove/fullmove indices                                             |
| Pondering                | Non-blocking ponderhit transition via shared atomic `ponder_flag` in `GoParameters`                                                                 |

---

## Roadmap & Iterations

### Phase 1 — Fix compiler errors & declare modules ✅

- [x] Remove unstable generic dispatch (refactor to stable `bool` const generics)
- [x] Declare `mod bitboard` in `src/lib.rs` (then refactored to `src/core/`)
- [x] Declare `mod types`, `mod movegen`, `mod position` in `src/lib.rs` (then refactored to `src/core/`)
- [x] Ensure `cargo check` passes cleanly

### Phase 1.5 — Idiomatic Module Refactoring

- [x] Flatten `src/types/mod.rs` to `src/types.rs`
  - [x] Move file contents of `src/types/mod.rs` to `src/types.rs`
  - [x] Delete folder `src/types/`
- [x] Consolidate tiny UCI submodules in `src/uci/`
  - [x] Create `src/uci/types.rs` containing all param/response structs:
    - [x] `GoParameters` (from `go_parameters.rs`)
    - [x] `SetOptionParameters` (from `set_option_parameters.rs`)
    - [x] `RegisterParameters` (from `register_parameters.rs`)
    - [x] `BestMove`, `Bound`, `UciId`, `UciInfo`, `UciOption`, `UciScore`, `UciScoreBound` (from `responses.rs`)
    - [x] `UciMove` (from `move.rs`)
    - [x] `UciPosition` (from `position.rs`)
  - [x] Keep `src/uci/handler.rs` as command processor loop
  - [x] Keep `src/uci/engine.rs` as UCI Engine trait interface
  - [x] Clean up `src/uci/mod.rs` to only export `Engine`, `UCIHandler`, and necessary types from `types.rs`
  - [x] Delete consolidated files: `go_parameters.rs`, `register_parameters.rs`, `set_option_parameters.rs`, `responses.rs`, `move.rs`, `position.rs`
- [x] Introduce core engine modules (Planned for Iteration 1) ✅
  - [x] Create `src/search/` module stubs & initial logic ✅
  - [x] Create `src/eval/` module stubs & initial logic ✅
- [x] Update `src/lib.rs module declarations for search/eval ✅

### Phase 2 — Complete `Position` (board state) ✅

- [x] FEN parsing: read/write board state from a Xiangqi FEN string
- [x] Internal board representation: piece-square array + per-piece bitboards
- [x] `UndoInfo` struct design to hold captured piece, old Zobrist hash, and rule50 counter (using `StateInfo`)
- [x] `do_move(mv: Move)` — apply a move to the position, update Zobrist and PST incrementally
- [x] `undo_move(mv: Move)` — revert a move using stored undo state
- [x] Zobrist hashing — incrementally updated hash key for transposition table
- [x] Xiangqi perpetual check and perpetual chase detection rules (asymmetric repeat scoring via `rule_judge`)

### Phase 3 — Move generation ✅

All 7 Xiangqi piece types, verified with perft tests:

- [x] **Rook (車)** — orthogonal, blocked by first piece (ray scanning -> Rank/File Occupancy Lookup)
- [x] **Cannon (炮)** — orthogonal, must jump exactly one piece to capture (ray scanning -> Rank/File Occupancy Lookup)
- [x] **Horse (馬)** — L-shape, blocked at the leg square
- [x] **Elephant (象)** — 2-diagonal, blocked at midpoint, cannot cross river
- [x] **Advisor (士)** — diagonal 1 step, palace only
- [x] **King (將/帥)** — 1 step orthogonal, palace only; flying general rule
- [x] **Pawn (兵/卒)** — 1 forward before river; 1 forward or sideways after
- [x] Perft tests matching known node counts for standard positions

### Phase 4 — Iterative Engine Milestones

Instead of implementing search and evaluation fully separate, we iteratively build and benchmark playable bot versions.

#### Iteration 1: The Crawler (Target: ~1000 ELO, Achieved: 1406 ELO in v1.0.0) ✅

_Goal: Play valid Xiangqi, capture hanging pieces, avoid basic blunders._

- [x] **Basic Search**: Simple Alpha-Beta Negamax with fail-soft
- [x] **Quiescence**: Basic captures-only search to avoid horizon effect
- [x] **Pure Material Eval**: Static material values (Eleeye-derived)
- [x] **Check / Repetition Penalty**: Hard penalty for mate/perpetual in search
- [x] **Wired Engine**: Replace `PrintBot` with `EngineBot`, hook `go` loop to respond with real moves
- [x] **Verification**: Ensure UCI engine responds correctly without crashes

#### Iteration 1.5: The Judge (Target: ~1500 ELO, Achieved: 1597 ELO in v1.1.0) ✅

_Goal: Full rule compliance, robust perpetual check/chase detection, automated version-vs-version benchmarking._

- [x] **Comprehensive Rule Judge**: Complete Xiangqi rules implementation (`rule_judge` in `src/core/position.rs`):
  - [x] 60-move rule (120 plies of quiet moves)
  - [x] Insufficient material draw rules
  - [x] Repetition loop checks
  - [x] Perpetual checking penalty (asymmetric repeat scoring, perpetual checker loses)
  - [x] Perpetual chasing penalty (detect chases using recursive rollback clone and piece IDs, perpetual chaser loses)
- [x] **2-Player Match Runner (`scripts/run_match.sh`)**: Fast evaluation comparing two engine versions using `sylvan-cli`
- [x] **Gauntlet Automation (`scripts/run_gauntlet.py`)**: Script running tournaments against different levels of Fairy-Stockfish to calibrate and estimate ELO
- [x] **Time Management & Safety Buffer**: Refined time allocation and process delay handling to avoid GUI timeouts
- [x] **Verification**: Played head-to-head match vs v1.0.0 (40 games: +34.9 ELO difference, 55.0% score) and gauntlet (120 games: average estimated ELO of 1597)

#### Iteration 2: The Walker (Target: ~1800 ELO) ⎔

_Goal: Deeper search depth, selective searching heuristics, solid positional alignment._

- [ ] **Transposition Table (TT)**: Store/retrieve search results with Zobrist keys
- [ ] **Aspiration Windows**: Reduce search windows for speed
- [ ] **Move Ordering**: Order TT moves > MVV-LVA captures > Killers/History to maximize beta-cutoffs
- [ ] **Piece-Square Tables (PST)**: Add positional piece-square tables for developmental guidance and king safety
- [ ] **Incremental Eval**: Keep evaluation updated incrementally during `do_move`/`undo_move` to avoid full board scan overhead
- [ ] **Verification**: Run `./scripts/run_match.sh` vs v1.1.0 and run `./scripts/run_gauntlet.py` to confirm ELO gains

#### Iteration 3: The Runner (Target: ~2000 ELO)

_Goal: Fast, selective tactical search and positional maturity._

- [ ] **Search Pruning**: Null move pruning (NMP) + Late move reductions (LMR)
- [ ] **Move Ordering Heuristics**: Add History heuristic for sorting quiet moves
- [ ] **Mobility & Safety**: Add mobility evaluation bonus and basic king/palace safety scoring
- [ ] **Pawn Structure**: Dynamic scoring for passed/crossed-river pawns
- [ ] **Verification**: Custom bench suite in `src/benchmark/` to track search speeds (NPS) and node reductions

#### Iteration 4: The Master (Target: ~2400+ ELO)

_Goal: Super-Grandmaster strength with parallel search and tuned parameters._

- [ ] **Lazy SMP**: Implement multi-threaded search sharing a single Transposition Table
- [ ] **Texel/SPSA Tuning**: Run automated parameter tuning on game databases to optimize PST/material values
- [ ] **Verification**: Gauntlet runs vs Fairy-Stockfish using `./tools/sylvan-cli` to benchmark absolute ELO strength

---

## Validation & Gauntlet Setup

To validate engine strength, we run automated matches using `sylvan-cli` against a reference engine, **Fairy-Stockfish**.

### 1. Install `sylvan-cli`

```bash
git clone https://github.com/hotfics/Sylvan --depth 1
cd Sylvan
qmake
make
cp ./projects/cli/sylvan-cli ../sylvan-cli
```

### 2. Build Reference Engine: Fairy-Stockfish

We use Fairy-Stockfish as our sparring partner because it supports Xiangqi natively via UCI.

```bash
cd tools
curl -L -O https://github.com/fairy-stockfish/Fairy-Stockfish-NNUE/releases/download/xiangqi-ae0082262b68/fairy-stockfish_x86-64
```

This downloads into the tools folder

### 3. Run ELO Gauntlet

Create a test script `scripts/run_gauntlet.sh` to compile Lingine and play 100 fast games (e.g. 10s + 0.1s increment) against Fairy-Stockfish.

To calibrate testing, configure Fairy-Stockfish to a specific ELO using `option.UCI_LimitStrength=true` and `option.UCI_Elo=<target>`:

```bash
#!/bin/bash
# Compile latest release
cargo build --release

# Execute gauntlet under sylvan-cli
# Set Fairy-Stockfish ELO to 1200 to test Iteration 1
./tools/sylvan-cli \
  -engine cmd=./target/release/lingine name=Lingine \
  -engine cmd=./tools/fairy-stockfish_x86-64 name=Fairy-Stockfish option.UCI_LimitStrength=true option.UCI_Elo=1200 \
  -each proto=uci tc=10/10+0.1 option.Hash=16 \
  -tournament round-robin -games 100 -concurrency 4 \
  -pgnout gauntlet.pgn -variant xiangqi
```

We can analyze `gauntlet.pgn` with Ordo or other tools to calculate relative ELO.

Alternatively, use `option.Skill\ Level=<0-20>` to limit search depth/time directly (e.g., 0 is weakest, 20 is max).

---

## Module Structure

```
src/
├── main.rs                    Entry point
├── lib.rs                     Declares library modules
├── benchmark/                 Benchmark positions (cfg(test))
├── core/                      Core board and rules subsystem
│   ├── mod.rs                 Exposes core module structure
│   ├── bitboard.rs            Bitboard (u128, 90 squares); masks for palace, files, ranks, sides
│   ├── types.rs               All domain types (Color, File, Rank, Square, Piece, PieceType, Move, Key)
│   ├── position.rs            Board state, FEN parser, do_move/undo_move, Zobrist, perpetual check/chase
│   └── movegen/               Move generation per piece type (orthogonal occupancy lookup)
├── search/                    [PLANNED] Search logic (Alpha-Beta, Aspiration, LMR, TT, quiescence)
├── eval/                      [PLANNED] Evaluation (Material, PST, mobility, safety)
├── bot/
│   ├── mod.rs
│   ├── print_bot.rs           Current stub engine (PrintBot)
│   └── engine_bot.rs          Real engine wrapping search
└── uci/
    ├── mod.rs                 Public exports (Engine, UCIHandler, etc.)
    ├── types.rs               Consolidated parameters/responses types (GoParameters, BestMove, etc.)
    ├── engine.rs              Engine trait
    └── handler.rs             UCIHandler (synchronous loop)
```

---

## Key Technical Notes

### Bitboard layout

- `u128`, 90 bits used (bits 0–89), bits 90–127 unused
- Bit index = `rank * 9 + file` (rank 0 = White's back rank)
- Constants: `PALACE`, `PAWN_FILE`, `side()`, `pawn()`, `file()`, `rank()`

### Move encoding (`types::Move`)

- `u16`, 7 bits origin (bits 7–13) + 7 bits destination (bits 0–6)
- `Move::none()` and `Move::null()` encoded as same-square moves

### UCI ↔ engine bridge

- `UciMove` (in `uci/move.rs`) is the parsing layer type; convert to `types::Move` via `src_file/src_rank/dst_file/dst_rank` accessors when applying to the engine position.
- `GoParameters::stop` (`Arc<AtomicBool>`) is the interruption signal; check with `Ordering::Relaxed` in tight search loops.

### Evaluation bootstrap values (Eleeye-derived)

| Piece         | Centipawns                             |
| ------------- | -------------------------------------- |
| Rook (車)     | 600                                    |
| Cannon (炮)   | 285                                    |
| Horse (馬)    | 270                                    |
| Elephant (象) | 120                                    |
| Advisor (士)  | 110                                    |
| Pawn (兵)     | 30–70 (increases after crossing river) |
| King (將)     | ∞                                      |

### ELO milestones (rough)

| Stage                           | Expected ELO |
| ------------------------------- | ------------ |
| Correct movegen + material eval | ~800–1000    |
| + Alpha-Beta + basic ordering   | ~1400–1600   |
| + TT + null move + LMR          | ~1800–2000   |
| + PST + mobility                | ~2000–2200   |
| + Texel tuning                  | ~2300–2500   |
| + Lazy SMP                      | ~2500+       |

---

## Known Technical Debt

| Item                            | Location          | Notes                                                                              |
| ------------------------------- | ----------------- | ---------------------------------------------------------------------------------- |
| `BestMove::null()` removed      | `responses.rs`    | Will need re-adding when `EngineBot` returns null moves on error                   |
| Handler reverted to synchronous | `handler.rs`      | Actor model was built and discarded; sync is simpler while `go` is instant         |
| `ponderhit` mid-search          | `handler.rs`      | Cannot interrupt go() while it runs; acceptable for Phase 4                        |
| `UciMove` has no `Display`      | `uci/types.rs`    | Needed for `pv` field in `UciInfo`; add in Phase 4                                 |
| FEN not validated at UCI layer  | `uci/types.rs`    | Validation happens in Position::from_fen() in Phase 2                            |
