# Lingine — High-Performance Rust Xiangqi (Chinese Chess) Engine

<!--toc:start-->

- [Lingine — High-Performance Rust Xiangqi (Chinese Chess) Engine](#lingine-high-performance-rust-xiangqi-chinese-chess-engine)
  - [Developers & Contributors](#developers--contributors)
  - [Engine Evolutionary Path](#engine-evolutionary-path)
  - [Getting Started & Setup](#getting-started--setup)
    - [Automated Toolchain Setup](#automated-toolchain-setup)
  - [Script Tutorials & Usage Guide](#script-tutorials--usage-guide)
    - [Tutorial 1: Head-to-Head Matches (`run_match.py`)](#tutorial-1-head-to-head-matches-run_matchpy)
      - [Command-Line Arguments for run_match.py](#command-line-arguments-for-run_matchpy)
      - [Practical Examples for run_match.py](#practical-examples-for-run_matchpy)
    - [Tutorial 2: ELO Gauntlet Evaluator (`run_gauntlet.py`)](#tutorial-2-elo-gauntlet-evaluator-run_gauntletpy)
      - [Command-Line Arguments for run_gauntlet.py](#command-line-arguments-for-run_gauntletpy)
      - [ELO Estimation Methodology](#elo-estimation-methodology)
      - [Practical Examples for run_gauntlet.py](#practical-examples-for-run_gauntletpy)
    - [Tutorial 3: Historical Regression Suite (`run_historical_evals.py`)](#tutorial-3-historical-regression-suite-run_historical_evalspy)
      - [Available Modes](#available-modes)
      - [Configuration Options](#configuration-options)
      - [Practical Examples for run_historical_evals.py](#practical-examples-for-run_historical_evalspy)
  - [Developer Commands & Testing](#developer-commands--testing)
    - [1. Compile Lingine](#1-compile-lingine)
    - [2. Run PERFT (Performance Move Generation) Tests](#2-run-perft-performance-move-generation-tests)
    - [3. Run Rule Judge and Position Tests](#3-run-rule-judge-and-position-tests)
    - [4. Interactive Command-Line Testing (UCI Protocol)](#4-interactive-command-line-testing-uci-protocol)

    <!--toc:end-->

Lingine is a state-of-the-art, high-performance Xiangqi (Chinese Chess) engine
written in Rust. It utilizes modern chess programming techniques including
advanced bitboard architectures, magic bitboards for sliding and blockable
leaping pieces, parallel-safe 2-threaded communication, and a highly optimized
search algorithm featuring Singular, Check, and One-Reply extensions.

---

## Developers & Contributors

| Name               | Student ID |
| :----------------- | :--------- |
| **Tran Tuan Anh**  | 202416124  |
| **Le Thanh Trung** | 202400076  |
| **Bui Tien Dung**  | 202416167  |

---

## Engine Evolutionary Path

Lingine features a highly optimized chess programming pipeline. Key search and
evaluation features include:

- Principal Variation Search (PVS)
- Null Move Pruning (NMP) (depth >= 3, verification search at depth >= 12)
- Late Move Reductions (LMR) (depth >= 3, quiet moves, history/PV adjusted)
- Tapered Evaluation (Middlegame & Endgame scores blended based on phase weight:
  Rook/Cannon/Knight = 2, Advisor/Bishop = 1, Pawn/King = 0)
- Transposition Table (TT) in Quiescence Search

---

## Getting Started & Setup

Before running matches or benchmarks, you must download the third-party
tournament manager, standard baseline opponent engine, and opening databases.

### Automated Toolchain Setup

The project provides a setup script [`setup_tools.py`](scripts/setup_tools.py)
to automate this setup:

```bash
python3 ./scripts/setup_tools.py
```

**What the script sets up:**

- Creates the local `tools/` directory.
- Downloads **Sylvan-CLI** (`sylvan-cli`): The tournament manager and
  engine-coordinating protocol interface.
- Downloads **Fairy-Stockfish** (`fairy-stockfish_x86-64`): A multi-variant
  strength-limited baseline opponent engine.
- Downloads **Masters Opening Database** (`xqdb_masters_40711_UCI_games.pgn`): A
  database of 40,711 master-level opening games to seed different opening
  positions during testing.

---

## Script Tutorials & Usage Guide

Lingine includes a comprehensive python script suite under [`scripts/`](scripts)
to automate engine validation, ELO estimation, and regression testing.

### Tutorial 1: Head-to-Head Matches (`run_match.py`)

Use [`run_match.py`](scripts/run_match.py) to run a round-robin tournament
between any two engine binaries to determine ELO differences, draw ratios, and
victory margins.

```bash
python3 ./scripts/run_match.py [options]
```

#### Command-Line Arguments for run_match.py

- `-a`, `--engine-a PATH`: Absolute or relative path to Engine A executable
  **(Required)**.
- `--name-a NAME`: Custom display name for Engine A (Default: derived from file
  name).
- `--options-a "OPTIONS"`: Custom UCI options passed to Engine A (e.g.
  `"option.Hash=64"`).
- `-b`, `--engine-b PATH`: Absolute or relative path to Engine B executable
  **(Required)**.
- `--name-b NAME`: Custom display name for Engine B (Default: derived from file
  name).
- `--options-b "OPTIONS"`: Custom UCI options passed to Engine B.
- `-g`, `--games N`: Total number of games to play in the match (Default:
  `1000`).
- `-t`, `--tc TIMECONTROL`: Time control setting in minutes/increment format
  (Default: `"3+0.03"`).
- `-c`, `--concurrency N`: Number of games to run in parallel (Default:
  automatically optimized).
- `-d`, `--depth N`: Opening book ply depth to feed engines before they start
  searching (Default: `12`).
- `-f`, `--openings PATH`: Path to the opening book PGN (Default:
  `tools/xqdb_masters_40711_UCI_games.pgn`).
- `-o`, `--outdir DIR`: Output directory to store results (Default:
  `matches/NAME_A-vs-NAME_B_TIMESTAMP`).
- `-s`, `--skip-build`: Skips the automatic `cargo build --release` step for
  target/release.
- `--sprt "PARAMS"`: Configures Sequential Probability Ratio Testing to
  terminate early when statistical significance is met (e.g.,
  `"elo0=0 elo1=10 alpha=0.05 beta=0.05"`).
- `-v`, `--verbose`: Enable verbose debugging output.

#### Practical Examples for run_match.py

**Example A: Compare the current release build against a historical version
(e.g., 1.5.0):**

```bash
python3 ./scripts/run_match.py \
  -a ./target/release/lingine --name-a Current-Dev \
  -b ./historical/lingine-1.5.0-piece-square-tables --name-b Ver-1.5.0 \
  -g 100 -t 5/10+0.1
```

**Example B: Compare current development against Fairy-Stockfish limited to 1600
ELO with SPRT:**

```bash
python3 ./scripts/run_match.py \
  -a ./target/release/lingine --name-a Lingine-Dev \
  -b ./tools/fairy-stockfish_x86-64 --name-b FS-1600 \
  --options-b "option.UCI_LimitStrength=true option.UCI_Elo=1600" \
  -g 200 --sprt "elo0=0 elo1=15 alpha=0.05 beta=0.05"
```

_Output:_ The script automatically launches parallel threads, aggregates
outcomes, and invokes `analyze_match.py` to generate a beautiful, comprehensive
markdown report `summary.md` inside your output folder containing score margins,
ELO delta, error bars, and game outcomes.

---

### Tutorial 2: ELO Gauntlet Evaluator (`run_gauntlet.py`)

Use [`run_gauntlet.py`](scripts/run_gauntlet.py) to assess the absolute ELO
rating of an engine by putting it through a gauntlet tournament against a series
of strength-limited standard baseline bots.

```bash
python3 ./scripts/run_gauntlet.py [options]
```

#### Command-Line Arguments for run_gauntlet.py

- `-a`, `--engine PATH`: Path to the engine binary under evaluation (Default:
  `./target/release/lingine`).
- `--name NAME`: Display name for the target engine (Default: `Lingine`).
- `-g`, `--games N`: Number of games to play against **each** ELO level
  (Default: `500`).
- `-t`, `--tc TIMECONTROL`: Time control setting (Default: `"3+0.03"`).
- `-d`, `--depth N`: Opening book ply depth (Default: `12`).
- `-e`, `--elos LIST`: Comma-separated list of Fairy-Stockfish ELO ratings to
  play against (Default: `"1200,1400,1600,1800,2000,2200"`).
- `-c`, `--cores N`: Number of parallel games (Default: auto-optimized based on
  CPU).
- `-f`, `--openings-file`: Path to the opening book PGN (Default:
  `tools/xqdb_masters_40711_UCI_games.pgn`).
- `-o`, `--pgnout DIR`: Output folder directory to store PGN files and logs.
- `-s`, `--skip-build`: Skips the automatic Rust compilation.

#### ELO Estimation Methodology

The script computes the estimated ELO rating using the Bradley-Terry model. For
each opponent with a known ELO $E_{\text{opp}}$, the ELO delta is calculated
from the win-draw-loss percentage:

$$\Delta\text{ELO} = 400 \times \log_{10}\left(\frac{\text{Score\%}}{1 - \text{Score\%}}\right)$$

$$\text{Estimated ELO} = E_{\text{opp}} + \Delta\text{ELO}$$

The final ELO is calculated as the average of the estimations across all ELO
levels.

#### Practical Examples for run_gauntlet.py

**Example A: Run a rapid ELO test (4 games per level) against a standard
range:**

```bash
python3 ./scripts/run_gauntlet.py -g 4 --skip-build
```

**Example B: Run an extensive ELO validation against high-tier bots:**

```bash
python3 ./scripts/run_gauntlet.py \
  -a ./target/release/lingine --name Lingine-v1.6 \
  -g 50 -e "1600,1800,2000,2200,2400" -t "10/15+0.2"
```

_Output:_ After the tournament completes, the script displays a detailed
analysis table including game scores per level, score percentages, and ELO
calculations, and outputs a `summary.md` file inside the output directory.

---

### Tutorial 3: Historical Regression Suite (`run_historical_evals.py`)

Use [`run_historical_evals.py`](scripts/run_historical_evals.py) to run a
comprehensive suite of matches and gauntlets across all historical engine
versions. This is used to map out the regression, progression, and absolute ELO
impact of every single commit in Lingine's development history.

```bash
python3 ./scripts/run_historical_evals.py [options]
```

#### Available Modes

The suite executes three phases of benchmarks. By default, it runs all three,
but you can filter them:

- `-n`, `--neighbor-only`: Runs neighbor matches (e.g. `1.1.0 vs 1.2.0`,
  `1.2.0 vs 1.3.0`). This determines the **incremental ELO delta** added by each
  new feature.
- `-b`, `--base-only`: Runs base matches (e.g. `1.1.0 vs 1.0.0-base`,
  `1.5.0 vs 1.0.0-base`). This measures the **cumulative ELO improvement** from
  the baseline.
- `-g`, `--gauntlets-only`: Runs gauntlet matches to measure the **absolute ELO
  rating** of every historical version.
- `-m`, `--matches-only`: Runs both neighbor and base matches, skipping the
  gauntlets.

#### Configuration Options

- `-v`, `--version <VER>`: Filters tasks to only run matches/gauntlets involving
  a specific historical version (e.g. `1.6.0a-check-extensions`).
- `-t`, `--tc TIMECONTROL`: Custom time control setting to override default
  `3+0.03`.
- `-c`, `--concurrency N`: Adjusts parallel game execution limit.
- `-f`, `--force`: Overwrites any previous benchmark reports and forces a
  complete recalculation.
- `-d`, `--dry-run`: Prints the planned Sylvan-CLI and python commands without
  running them.
- `--gauntlet-games N`: Number of gauntlet games (default: 500).
- `--match-games N`: Number of 1v1 match games (default: 1000).

#### Practical Examples for run_historical_evals.py

**Example A: Dry-run the entire benchmark suite to verify the active plan:**

```bash
python3 ./scripts/run_historical_evals.py --dry-run
```

**Example B: Benchmark only the Check Extensions release (`1.6.0a`):**

```bash
python3 ./scripts/run_historical_evals.py -v 1.6.0a-check-extensions
```

**Example C: Run matches only (both base and neighbor) with optimized
concurrency:**

```bash
python3 ./scripts/run_historical_evals.py --matches-only -c 12
```

_Cache & Resume:_ The script features automatic state persistence. If a match or
gauntlet has already completed and generated a `summary.md`, it is skipped,
allowing you to stop and resume the comprehensive suite at any time.

---

## Developer Commands & Testing

### 1. Compile Lingine

To compile the latest engine binary with full release-mode optimizations:

```bash
cargo build --release
```

The resulting executable will be saved at `./target/release/lingine`.

### 2. Run PERFT (Performance Move Generation) Tests

Validate the correctness of the bitboard move generator against standardized
deep search coordinates:

```bash
cargo test --lib core::movegen
```

### 3. Run Rule Judge and Position Tests

Verify that the engine correctly handles checkmate detection, stalemates,
repetitions, and the 60-move rule:

```bash
cargo test --lib core::position::tests
```

### 4. Interactive Command-Line Testing (UCI Protocol)

You can launch and interact with the engine directly through the command line.
Run `./target/release/lingine` and type the standard UCI commands:

```text
$ ./target/release/lingine
uci
id name Lingine
id author tuasananh
option name Hash type spin default 16 min 1 max 1024
uciok
isready
readyok
position startpos moves h2e2 h9g7 h0g2 i9h9 r
go depth 6
info depth 1 score 12 nodes 21 nps 21000 pv h0g2
...
info depth 6 score 25 nodes 8792 nps 920000 pv h0g2
bestmove h0g2
quit
```

---
