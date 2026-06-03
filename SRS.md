# Software Requirements Specification (SRS)

## Project: Lingine — High-Performance Xiangqi Engine in Rust

> **Target**: ~2500 ELO Xiangqi Engine  
> **Language**: Rust (edition 2024)  
> **Protocol**: UCI only  
> **Inspiration**: Pikafish / Stockfish architecture adapted for the
> $9 \times 10$ board

---

## Table of Contents

1. [Introduction & Executive Summary](#1-introduction--executive-summary)
2. [Architectural Overview](#2-architectural-overview)
3. [Proposed Module Reorganization](#3-proposed-module-reorganization)
4. [Detailed Subsystem Specifications](#4-detailed-subsystem-specifications)
   - 4.1
     [Core Rules & Board Engine (`core/`)](#41-core-rules--board-engine-core)
   - 4.2 [Search Subsystem (`search/`)](#42-search-subsystem-search)
   - 4.3 [Evaluation Subsystem (`eval/`)](#43-evaluation-subsystem-eval)
   - 4.4 [UCI Subsystem (`uci/`)](#44-uci-subsystem-uci)
5. [Non-Functional Requirements & Performance Targets](#5-non-functional-requirements--performance-targets)
6. [Resolved Design Decisions](#6-resolved-design-decisions)

---

## 1. Introduction & Executive Summary

**Lingine** is a state-of-the-art Chinese Chess (Xiangqi) engine written in
Rust, aiming to achieve Grandmaster-level strength (~2500 ELO). By employing
modern chess engine techniques—including bitboard-accelerated move generation,
transposition tables, highly selective Alpha-Beta search, and a curated
evaluation function—Lingine provides a highly efficient and modular codebase.

This Software Requirements Specification (SRS) establishes the system
architecture, defines a reorganized file structure, specifies algorithm
configurations, and presents key design decisions to align prior work (such as
the completed move generator) with the upcoming search and evaluation
components.

---

## 2. Architectural Overview

The engine is divided into four cleanly decoupled subsystems:

1. **Core Board Engine (`core/`)**: Houses board representation, bitboards, FEN
   parser, move generation, and make/unmake move mechanisms.
2. **Search Subsystem (`search/`)**: Implements iterative deepening, Negamax
   Alpha-Beta search, transposition tables, and move ordering heuristics.
3. **Evaluation Subsystem (`eval/`)**: Calculates static positional values
   utilizing piece-square tables (PST), material balances, mobility, and safety
   rules.
4. **UCI Protocol Subsystem (`uci/`)**: Translates standard Universal Chess
   Interface (UCI) commands (e.g. `position`, `go`, `stop`) into internal engine
   commands.

```mermaid
graph TD
    %% Styling
    classDef core fill:#e1f5fe,stroke:#039be5,stroke-width:2px;
    classDef search fill:#e8f5e9,stroke:#2e7d32,stroke-width:2px;
    classDef eval fill:#fff3e0,stroke:#ef6c00,stroke-width:2px;
    classDef uci fill:#f3e5f5,stroke:#8e24aa,stroke-width:2px;

    %% Subsystems
    GUI[Chess GUI / CuteChess] <--> |UCI Commands / Strings| UCIHandler[UCI Subsystem: handler.rs]

    subgraph UCI Protocol Subsystem
        UCIHandler <--> EngineTrait[engine.rs Interface]
        types_uci[types.rs / parser.rs]
    end

    subgraph Bot Layer
        EngineBot[EngineBot]
    end

    subgraph Search Subsystem
        SearchController[search.rs]
        TT[Transposition Table: tt.rs]
        MoveOrdering[Ordering: ordering.rs]
        Repetition[Repetition / Perpetual Chase]
    end

    subgraph Evaluation Subsystem
        StaticEval[eval.rs]
        PST[Piece-Square Tables: pst.rs]
        Mobility[Mobility & Safety]
    end

    subgraph Core Subsystem
        Position[Position State: position.rs]
        MoveGen[Move Generator: movegen.rs]
        Bitboard[Bitboard Layer: bitboard.rs]
        Types[Domain Types: types.rs]
    end

    %% Connections
    EngineTrait <--> EngineBot
    EngineBot <--> SearchController
    SearchController <--> StaticEval
    SearchController <--> TT
    SearchController <--> MoveOrdering
    SearchController <--> Repetition

    SearchController <--> Position
    StaticEval <--> Position
    MoveOrdering <--> MoveGen
    MoveGen <--> Position
    Position <--> Bitboard
    Position <--> Types

    %% Class Application
    class Position,MoveGen,Bitboard,Types core;
    class SearchController,TT,MoveOrdering,Repetition search;
    class StaticEval,PST,Mobility eval;
    class UCIHandler,EngineTrait,types_uci uci;
```

---

## 3. Proposed Module Reorganization

To improve readability and navigate the codebase easily, we will group core
Xiangqi logic inside a `core/` folder, segregate search/evaluation into distinct
modules, and consolidate the fragmented files in the `uci/` module.

### Directory Structure Transition Map

| Current Path         | Proposed Path              | Component  | Description                              |
| -------------------- | -------------------------- | ---------- | ---------------------------------------- |
| `src/types.rs`       | `src/core/types.rs`        | **Core**   | Shared types (Color, Square, Move)       |
| `src/bitboard.rs`    | `src/core/bitboard.rs`     | **Core**   | `u128` bitboard definitions              |
| `src/position.rs`    | `src/core/position.rs`     | **Core**   | Board state & do_move/undo_move          |
| `src/movegen/`       | `src/core/movegen/`        | **Core**   | Move generation (attacks & tables)       |
| _(planned)_          | `src/search/mod.rs`        | **Search** | Alpha-Beta search control (planned)      |
| _(planned)_          | `src/search/tt.rs`         | **Search** | Transposition Table (TT)                 |
| _(planned)_          | `src/search/ordering.rs`   | **Search** | MVV-LVA, killers, history sorting        |
| _(planned)_          | `src/search/repetition.rs` | **Search** | Perpetual check/chase detector           |
| _(planned)_          | `src/eval/mod.rs`          | **Eval**   | Evaluation entrypoint & driver (planned) |
| _(planned)_          | `src/eval/material.rs`     | **Eval**   | Hard values & scaling adjustments        |
| _(planned)_          | `src/eval/pst.rs`          | **Eval**   | Positional tables per phase              |
| _(planned)_          | `src/eval/mobility.rs`     | **Eval**   | Safe squares and mobility metrics        |
| `src/uci/mod.rs`     | `src/uci/mod.rs`           | **UCI**    | Re-exports standard interface            |
| `src/uci/engine.rs`  | `src/uci/engine.rs`        | **UCI**    | Engine trait definition                  |
| `src/uci/handler.rs` | `src/uci/handler.rs`       | **UCI**    | Synchronous stdin-stdout processor       |
| _Consolidated_       | `src/uci/types.rs`         | **UCI**    | Merged params/responses structs          |
| `src/bot/`           | `src/bot/`                 | **Bot**    | Wrapper classes (PrintBot, EngineBot)    |

> [!NOTE] All parameters and response structs currently scattered inside
> `src/uci/` (e.g. `go_parameters.rs`, `register_parameters.rs`,
> `set_option_parameters.rs`, `responses.rs`, `move.rs`, `position.rs`) will be
> merged into a single clean file: `src/uci/types.rs`. This will flatten
> `src/uci/` from 11 files down to 4 files, significantly reducing filesystem
> noise.

---

## 4. Detailed Subsystem Specifications

### 4.1 Core Rules & Board Engine (`core/`)

The Core Subsystem encapsulates the state representation, validating move
legality, generating moves, and applying/reverting transitions incrementally.

- **Board Layout**: $9 \times 10$ coordinates represented as a flat array of 90
  elements, coupled with piece-type and piece-color bitboards.
- **Bitboards**: Structured as `u128` values using the lower 90 bits (indices
  $0 \dots 89$). Bit index is calculated as
  $\text{rank} \times 9 + \text{file}$.
- **Move Encoding**: Encoded in a 16-bit `u16` representation to maintain a
  cache-friendly footprint:
  - **Bits 0–6**: Destination square (0 to 89)
  - **Bits 7–13**: Origin square (0 to 89)
  - **Bits 14–15**: Transposition Table flags (Empty, Exact, Alpha, Beta)
- **Undo State**: Evaluated incrementally through `StateInfo` stack tracking.
  Avoids expensive full-board re-cloning.

#### Refactored `src/core/mod.rs` Expose Layout:

```rust
pub mod bitboard;
pub mod types;
pub mod position;
pub mod movegen;

pub use bitboard::Bitboard;
pub use types::{Color, Square, Rank, File, Piece, PieceType, Move, MoveList, Value, Key};
pub use position::Position;
```

---

### 4.2 Search Subsystem (`search/`)

The Search engine utilizes a selective iterative-deepening depth search
optimized with modern pruning heuristics to isolate high-value tactical
branches.

- **Framework**: Fail-soft Negamax with Alpha-Beta pruning.
- **Quiescence Search**: Evaluates quiet capture sequences to resolve tactical
  tension and avoid the horizon effect.
- **Iterative Deepening**: Progresses depth incrementally (1, 2, 3, etc.)
  allowing stable move prediction and robust time management.
- **Pruning & Reduction Heuristics**:
  - **Null Move Pruning (NMP)**: Detects high-advantage nodes by performing a
    shallow search after passing the turn.
  - **Late Move Reductions (LMR)**: Performs shallow-depth searches on moves
    ordered late in the list.
  - **Aspiration Windows**: Minimizes alpha-beta bounds around the prior
    iteration's score to limit search space.
- **Move Ordering Pipeline**: Maximizes search speed by triggering quick
  beta-cutoffs:
  1.  **Transposition Table (TT) Move**: Highest priority.
  2.  **Captures via MVV-LVA**: Most Valuable Victim / Least Valuable Attacker.
  3.  **Killer Moves**: Quiet moves that caused beta-cutoffs in sibling
      branches.
  4.  **History Heuristic**: Dynamic weights assigned to moves based on
      historical cutoffs.
- **Transposition Table (TT)**:
  - Keyed by a 64-bit Zobrist key.
  - Stores `score`, `depth`, `bound` (Exact, LowerBound, UpperBound), and
    `best_move`.
  - Replacement scheme: Two-tier or depth-preferred bucket replacement with
    aging.

---

### 4.3 Evaluation Subsystem (`eval/`)

Evaluation maps a static board state to a score in centipawns (from White/Red's
perspective). To hit the ~2500 ELO target, a hybrid static + incremental
evaluation model will be implemented.

- **Incremental Evaluation**: The board state maintains a running score of piece
  material and positional Piece-Square Tables (PST). When `do_move` is called,
  the positional score is adjusted incrementally via XOR/add/sub, avoiding full
  board loops.
- **Static Positional Tables**: Distinct PSTs mapped per piece type, color, and
  game phase (Opening vs. Endgame).
- **Material Base Values (Eleeye Bootstrap)**:
  - Rook: 600
  - Cannon: 285
  - Horse: 270
  - Elephant / Advisor: 120 / 110
  - Pawn: 30 (opening) $\to$ 70 (advanced, crossed river)
- **Advanced Positional Heuristics**:
  - **Mobility**: Reward sliding pieces based on the number of legal, safe
    squares they control.
  - **Pawn Structure**: Reward crossed-river pawns, penalize isolated pawns, and
    reward pawns controlling files.
  - **King Safety**: Penalize exposure of the King in the palace, reward intact
    Advisor-Elephant structures (the defensive shield).
  - **Cannon Platforms**: Reward Cannons aligned with a single intervening piece
    (the platform) pointing at vulnerable enemy pieces.

---

### 4.4 UCI Subsystem (`uci/`)

The protocol interface handles standard GUI communication commands.

- **Flattener**: Flattens sub-files into `uci/types.rs` to clean up the module
  structure.
- **Time Management**: Decodes search constraints (`wtime`, `btime`, `winc`,
  `binc`, `movetime`, `depth`) and allocates optimal search times.
- **Signals**: Handles async abort commands via `GoParameters::stop` thread
  indicators.

---

## 5. Non-Functional Requirements & Performance Targets

- **Safety & Core Correctness**: Zero unsafe code blocks in core rule files.
  100% of custom bitwise operations covered by unit and perft verification
  tests.
- **Search Speed (NPS)**: Target search processing speed $> 1,000,000$ Nodes Per
  Second (NPS) on modern multi-core x86 CPUs.
- **Memory Overhead**: Limit Transposition Table memory consumption to a
  user-defined threshold (default 64MB, configurable up to 2GB) without dynamic
  allocations during search.
- **Concurrency**: Phase 1-3 single-threaded search. Phase 4 transition to
  **Lazy SMP** parallel thread execution with lock-free shared TT.

---

## 6. Resolved Design Decisions

We have aligned on the following structural and algorithmic parameters for
Lingine:

- **Decision 1: Folder Restructuring (Core, Search, Eval, UCI)**
  - _Result_: **Option A** (Clean Subsystem Decoupling) has been fully executed.
    The rules engine resides inside `src/core/`, and `src/uci/` has been
    consolidated and flattened.
- **Decision 2: Rust Compiler Requirements (Nightly vs. Stable)**
  - _Result_: **Option B** (Stable Rust 2024). The move generator has been made
    stable-compatible by using standard `bool` const generics
    (`<const IS_WHITE: bool>`).
- **Decision 3: Perpetual Check & Repetition Rules**
  - _Result_: **Option B** (Asymmetric Repetition Scoring). We will port
    Pikafish's asymmetric check/chase detector using a unique piece-ID matrix
    (`idBoard`) to identify perpetual checking/chasing cycles and return
    win/loss scores accordingly.
- **Decision 4: UCI vs. UCCI Protocol**
  - _Result_: **Option A** (Strict UCI-Only). We will target global testing and
    tournament tools natively. No native UCCI parser is needed.
- **Decision 5: Transposition Table (TT) Packaging & Alignment**
  - _Result_: **Option B** (Standard Unpacked Structs). We will use a standard,
    simple unpacked array format initially to speed up development and ease
    debugging.
- **Decision 6: Positional Scoring Strategy**
  - _Result_: **Option B** (Pikafish-Style Incremental Accumulators). We will
    store the running `material_score` and `pst_score` accumulators inside the
    `StateInfo` struct. When `do_move` is called, these are updated
    incrementally in the new `StateInfo`. When `undo_move` is called, popping
    the `StateInfo` stack automatically rolls back the evaluation score with
    zero math operations or bug risk.
- **Decision 7: FEN Parsing Robustness**
  - _Result_: **Option A** (Dynamic Token Parsing). We will dynamically detect
    the FEN token length (4 vs 6 tokens) to support both castling/en-passant
    placeholders and traditional clean Xiangqi FENs, avoiding incorrect default
    resets.
- **Decision 8: Pondering Support**
  - _Result_: Pondering is currently unsupported due to the search execution loop
    blocking the engine actor thread. Non-blocking pondering support remains a
    deferred feature.
- **Decision 9: Move Flags Utility**
  - _Result_: We will use the 2-bit flags in the 16-bit `Move` representation
    to store the `TranspositionTableFlag` (Empty, Exact, Alpha, Beta) to save space inside the TT,
    rather than for move classifications (which are queried from the board/move generator).

---

## 7. Future High-Performance Optimizations (Deferred)

Once the base engine is complete and playing valid Chinese Chess, we will
execute the following state-of-the-art performance refactorings to maximize
search speed (NPS) and ELO strength:

### 7.1 Stack-Allocated History Stack

- **Target**: Eliminate all heap allocations in the tight search loop.
- **Action**: Refactor the dynamic `Vec<StateInfo>` inside `Position` to a
  pre-allocated fixed-size array stack `[StateInfo; 2048]`, avoiding resizing
  overhead and vector boundary checks during `do_move`/`undo_move`.

### 7.2 Cache-Line Packed TT Buckets (64-Byte Alignment)

- **Target**: Minimize RAM memory access latency and CPU cache misses during TT
  queries.
- **Action**: Pack `TTEntry` into exactly 16 bytes:
  ```rust
  #[repr(C)]
  pub struct TTEntry {
      pub key: u64,          // 8 bytes (Zobrist key)
      pub best_move: u16,    // 2 bytes
      pub score: i16,        // 2 bytes
      pub depth: u8,         // 1 byte
      pub bound: u8,         // 1 byte
      pub age: u16,          // 2 bytes
  }
  ```
  This allows a standard 64-byte CPU cache line to hold exactly 4 entries,
  enabling a single L1-cache search bucket hit per RAM fetch.
