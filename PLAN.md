# Lingine — Engine Development Plan

> **Target**: ~2500 ELO Xiangqi engine in Rust  
> **Protocol**: UCI (UCCI wrapper later if needed for Chinese GUIs)  
> **Inspiration**: Pikafish / Stockfish architecture adapted for Xiangqi

---

## Decisions Agreed On

| Topic | Decision |
|---|---|
| Move generation dispatch | Separate fns: `generate_legal`, `generate_captures`, `generate_quiets`, `generate_evasions` |
| Sliding piece generation | On-the-fly ray scanning first; precomputed Rank/File Occupancy Lookup later after profiling (no magic bitboards needed for orthogonal-only sliding) |
| Enum const generics | Use `#![feature(min_adt_const_params)]` for now; refactor to stable when it lands |
| Search | Alpha-Beta + Iterative Deepening + Perpetual Check/Chase detection |
| Evaluation | Material + PST (Eleeye values to start) + Mobility (updated incrementally inside `Position`) |
| Search enhancements | Transposition Table + move ordering (TT/MVV-LVA/killers/history via Move flags) + null move pruning |
| Protocol | UCI only (`uci`/`uciok`, standard move notation `a0b1`) |
| UCCI | Thin wrapper later if Chinese GUI support is needed (≈1 day effort) |
| Threading | Single-threaded first; Lazy SMP after search is stable |
| Evaluation tuning | Bootstrap from Eleeye values → SPSA/Texel tuning to reach 2400+ |
| Testing framework | Perft tests per piece type + unit tests; cutechess-cli gauntlet vs Fairy-Stockfish for ELO |
| Move encoding | `u16` encoding: 7 bits from, 7 bits to, 2 bits for flags (Capture, Quiet, Check) for cache-friendly sorting |
| Board undo state | Track backtracking using `UndoInfo` struct (captured piece, previous hash, rule50, etc.) |

---

## The 7 Phases

### Phase 1 — Fix compiler errors & declare modules ✅ (mostly done)
- [x] Remove `ConstParamTy` (use `min_adt_const_params` feature)
- [x] Declare `mod bitboard` in `main.rs`
- [x] Declare `mod types`, `mod movegen`, `mod position` in `main.rs`
- [ ] Ensure `cargo check` passes cleanly

### Phase 1.5 — Idiomatic Module Refactoring
- [ ] Flatten `src/types/mod.rs` to `src/types.rs`
  - [ ] Move file contents of `src/types/mod.rs` to `src/types.rs`
  - [ ] Delete folder `src/types/`
- [ ] Consolidate tiny UCI submodules in `src/uci/`
  - [ ] Create `src/uci/types.rs` containing all param/response structs:
    - [ ] `GoParameters` (from `go_parameters.rs`)
    - [ ] `SetOptionParameters` (from `set_option_parameters.rs`)
    - [ ] `RegisterParameters` (from `register_parameters.rs`)
    - [ ] `BestMove`, `Bound`, `UciId`, `UciInfo`, `UciOption`, `UciScore`, `UciScoreBound` (from `responses.rs`)
    - [ ] `UciMove` (from `move.rs`)
    - [ ] `UciPosition` (from `position.rs`)
  - [ ] Keep `src/uci/handler.rs` as command processor loop
  - [ ] Keep `src/uci/engine.rs` as UCI Engine trait interface
  - [ ] Clean up `src/uci/mod.rs` to only export `Engine`, `UCIHandler`, and necessary types from `types.rs`
  - [ ] Delete consolidated files: `go_parameters.rs`, `register_parameters.rs`, `set_option_parameters.rs`, `responses.rs`, `move.rs`, `position.rs`
- [ ] Introduce core engine modules
  - [ ] Create `src/search.rs` (declared in `main.rs`)
  - [ ] Create `src/eval.rs` (declared in `main.rs`)
- [ ] Update `src/main.rs` module declarations and ensure everything builds perfectly

### Phase 2 — Complete `Position` (board state)
- [ ] FEN parsing: read/write board state from a Xiangqi FEN string
- [ ] Internal board representation: piece-square array + per-piece bitboards
- [ ] `UndoInfo` struct design to hold captured piece, old Zobrist hash, and rule50 counter
- [ ] `do_move(mv: Move, &mut undo: UndoInfo)` — apply a move to the position, update Zobrist and PST incrementally
- [ ] `undo_move(mv: Move, &undo: UndoInfo)` — revert a move using stored undo state
- [ ] Zobrist hashing — incrementally updated hash key for transposition table
- [ ] Xiangqi perpetual check and perpetual chase detection rules (asymmetric repeat scoring)

### Phase 3 — Move generation
All 7 Xiangqi piece types, verified with perft tests:
- [ ] **Rook (車)** — orthogonal, blocked by first piece (ray scanning -> Rank/File Occupancy Lookup)
- [ ] **Cannon (炮)** — orthogonal, must jump exactly one piece to capture (ray scanning -> Rank/File Occupancy Lookup)
- [ ] **Horse (馬)** — L-shape, blocked at the leg square
- [ ] **Elephant (象)** — 2-diagonal, blocked at midpoint, cannot cross river
- [ ] **Advisor (士)** — diagonal 1 step, palace only
- [ ] **King (將/帥)** — 1 step orthogonal, palace only; flying general rule
- [ ] **Pawn (兵/卒)** — 1 forward before river; 1 forward or sideways after
- [ ] Perft tests matching known node counts for standard positions

### Phase 4 — Search
- [ ] Negamax alpha-beta with fail-soft and perpetual check/chase penalty detection
- [ ] Iterative deepening with aspiration windows
- [ ] Transposition table (TT) with Zobrist keys
- [ ] Move ordering: TT move > Move-flag captures > killer moves > history heuristic
- [ ] Null move pruning
- [ ] Late move reductions (LMR)
- [ ] Quiescence search (captures + checks)
- [ ] Time management: honour `wtime`/`btime`/`movetime`/`infinite` from `GoParameters`
- [ ] Wire `GoParameters::stop` flag into the search loop

### Phase 5 — Evaluation
- [ ] Incremental evaluation verification (compare incremental PST against full evaluation sanity checks)
- [ ] Material values (bootstrap from Eleeye open-source values)
- [ ] Piece-square tables (PST) per piece, per side
- [ ] Mobility scoring (count of pseudo-legal moves)
- [ ] King safety heuristics (palace control, flying general threats)
- [ ] Pawn structure (passed pawns after river crossing)
- [ ] Tempo / side-to-move bonus

### Phase 6 — Wire into `EngineBot` (replace `PrintBot`)
- [ ] Create `src/bot/engine_bot.rs` implementing `Engine` trait
- [ ] Delegate `position()` → update internal `Position`
- [ ] Delegate `go()` → run search, send `UciInfo` per depth iteration, return `BestMove`
- [ ] Honour stop flag: `if params.stop.load(Relaxed) { break; }`
- [ ] Swap `PrintBot` for `EngineBot` in `main.rs`

### Phase 7 — ELO testing & tuning
- [ ] Set up `cutechess-cli` gauntlet: Lingine vs Fairy-Stockfish (weak settings)
- [ ] Benchmark positions suite (`src/benchmark/`)
- [ ] SPSA/Texel tuning: run against large Xiangqi game database, tune PST weights
- [ ] Lazy SMP (multi-threaded search) once ELO plateaus around 2200

---

## Module Structure

```
src/
├── main.rs                    Entry point; declares all root modules
├── benchmark/                 Benchmark positions (cfg(test))
├── bitboard.rs                Bitboard (u128, 90 squares); masks for palace, files, ranks, sides
├── types.rs                   All domain types (Color, File, Rank, Square, Piece, PieceType, Move, Key)
├── position.rs                Board state, FEN parser, do_move/undo_move, Zobrist, perpetual check/chase
├── movegen.rs                 Move generation per piece type (orthogonal occupancy lookup)
├── search.rs                  [NEW] Search logic (Alpha-Beta, Aspiration, LMR, TT, quiescence)
├── eval.rs                    [NEW] Evaluation (Material, PST, mobility, safety)
├── bot/
│   ├── mod.rs
│   ├── print_bot.rs           Current stub engine (PrintBot)
│   └── engine_bot.rs          Real engine wrapping search
└── uci/
    ├── mod.rs                 Public exports (Engine, UCIHandler, etc.)
    ├── types.rs               [NEW] Consolidated parameters/responses types (GoParameters, BestMove, etc.)
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
| Piece | Centipawns |
|---|---|
| Rook (車) | 600 |
| Cannon (炮) | 285 |
| Horse (馬) | 270 |
| Elephant (象) | 120 |
| Advisor (士) | 110 |
| Pawn (兵) | 30–70 (increases after crossing river) |
| King (將) | ∞ |

### ELO milestones (rough)
| Stage | Expected ELO |
|---|---|
| Correct movegen + material eval | ~800–1000 |
| + Alpha-Beta + basic ordering | ~1400–1600 |
| + TT + null move + LMR | ~1800–2000 |
| + PST + mobility | ~2000–2200 |
| + Texel tuning | ~2300–2500 |
| + Lazy SMP | ~2500+ |

---

## Known Technical Debt

| Item | Location | Notes |
|---|---|---|
| `BestMove::null()` removed | `responses.rs` | Will need re-adding when `EngineBot` returns null moves on error |
| Handler reverted to synchronous | `handler.rs` | Actor model was built and discarded; sync is simpler while `go` is instant |
| `ponderhit` mid-search | `handler.rs` | Cannot interrupt go() while it runs; acceptable for Phase 4 |
| `UciMove` has no `Display` | `uci/move.rs` | Needed for `pv` field in `UciInfo`; add in Phase 4 |
| FEN not validated at UCI layer | `uci/position.rs` | Validation happens in `Position::from_fen()` in Phase 2 |
| `min_adt_const_params` nightly | `main.rs` | Used for enum const generic movegen dispatch; stabilisation tracked in rust#154042 |
