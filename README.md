<div align="center">
  <img
    width="240"
    height="240"
    alt="Lingine"
    src="./assets/logo.png"
  />
  <h1><code>Lingine</code></h1>
</div>

A UCI-compliant Xiangqi (Chinese Chess) engine written in Rust.

Challenge me on [Playstrategy](https://playstrategy.org/@/lingine)!

## Table of Contents

<!--toc:start-->

- [Table of Contents](#table-of-contents)
- [Developers](#developers)
- [Releases](#releases)
- [Getting Started](#getting-started)
  - [Precompiled Binaries](#precompiled-binaries)
  - [Building from source](#building-from-source)
  - [Running `Lingine` with a GUI](#running-lingine-with-a-gui)
- [Development Guide](#development-guide)
  - [Automated Toolchain Setup](#automated-toolchain-setup)
  - [Scripts](#scripts)
  - [Working with the source](#working-with-the-source)
- [Acknowledgemments](#acknowledgemments)
- [License](#license)
<!--toc:end-->

---

## Developers

| Name               | Student ID |
| :----------------- | :--------- |
| **Tran Tuan Anh**  | 202416124  |
| **Le Thanh Trung** | 202400076  |
| **Bui Tien Dung**  | 202416167  |

---

## Releases

| Version                                                           | Estimated ELO | Release Date  | What's new                                                                                                                                             |
| :---------------------------------------------------------------- | :------------ | :------------ | :----------------------------------------------------------------------------------------------------------------------------------------------------- |
| [1.9.0](https://github.com/tuasananh/lingine/releases/tag/v1.9.0) | 2244 ± 10.9   | June 17, 2026 | - Aspiration Window Fail-high Reductions<br>- Reverse Futility Pruning<br>- Futility Pruning<br>- Late Move Pruning<br>- Internal Iterative Reductions |
| [1.8.0](https://github.com/tuasananh/lingine/releases/tag/v1.8.0) | 2189 ± 10.3   | June 16, 2026 | - Mobility Bonus<br>- Defender Bonus<br>- Texel Tuned Evaluation Parameters                                                                            |
| [1.7.0](https://github.com/tuasananh/lingine/releases/tag/v1.7.0) | 2027 ± 9.2    | June 13, 2026 | - Tapered Evaluation<br>- Principal Variation Search<br>- Late Move Reductions<br>- Null Move Pruning                                                  |
| [1.6.5](https://github.com/tuasananh/lingine/releases/tag/v1.6.5) | 1936 ± 8.9    | June 9, 2026  | - Better tooling with better project stucture<br>- Transposition Table in Quiescence-Search                                                            |
| [1.6.0](https://github.com/tuasananh/lingine/releases/tag/v1.6.0) | 1916 ± 8.8    | June 1, 2026  | - Check Extensions<br>- Singular Extensions<br>- One-Reply Extensions                                                                                  |
| [1.5.0](https://github.com/tuasananh/lingine/releases/tag/v1.5.0) | 1854 ± 8.7    | June 1, 2026  | - Piece-square Tables                                                                                                                                  |
| [1.4.0](https://github.com/tuasananh/lingine/releases/tag/v1.4.0) | 1612 ± 8.6    | May 31, 2026  | - Move Ordering with Killers and History Heuristics                                                                                                    |
| [1.3.0](https://github.com/tuasananh/lingine/releases/tag/v1.3.0) | 1572 ± 8.7    | May 31, 2026  | - Aspiration Window                                                                                                                                    |
| [1.2.0](https://github.com/tuasananh/lingine/releases/tag/v1.2.0) | 1567 ± 8.7    | May 31, 2026  | - Transposition Table                                                                                                                                  |
| [1.1.0](https://github.com/tuasananh/lingine/releases/tag/v1.1.0) | 1460 ± 8.9    | May 29, 2026  | - Follow Rules of Xiangqi                                                                                                                              |
| [1.0.0](https://github.com/tuasananh/lingine/releases/tag/v1.0.0) | 1465 ± 8.8    | May 28, 2026  | - Bitboard move generation <br>- Fail-soft Negamax Alpha-Beta Search<br>- Quiescence Search<br>- Basic Material Evaluation                             |

## Getting Started

### Precompiled Binaries

You can download precompiled builds from the [Releases page](https://github.com/tuasananh/lingine/releases).

### Building from source

To build `Lingine`, you just need a Rust compiler, here is the [installation guide](https://www.rust-lang.org/tools/install).

After cloning the repository, run this command

```bash
cargo build --release
```

to get the release version with all optimizations. The binary will be generated
in `target/release/lingine`.

### Running `Lingine` with a GUI

Because `Lingine` is fully UCI-compliant, you can play against it with any GUI
that support UCI engines. The guide below is for development on Linux, where the
tools installed will contain a `sylvan` binary that supports UCI engines out of
the box.

## Development Guide

For development, you also need to have `python` installed in your system.
Development happens on Linux, if you use Windows (why do you even use
Microslop), don't bother.

### Automated Toolchain Setup

The project provides a setup script [`setup_tools.py`](scripts/setup_tools.py)
to automate this setup:

```bash
python3 ./scripts/setup_tools.py
```

**What the script sets up:**

- Creates the local `tools/` directory.
- Downloads `sylvan-cli`: The tournament manager and engine-coordinating protocol
  interface.
- Downloads `sylvan`: The GUI to play against `Lingine` locally.
- Downloads `fairy-stockfish_x86-64`: A multi-variant strength-limited baseline
  opponent engine.
- Downloads **Masters Opening Database** (`xqdb_masters_40711_UCI_games.pgn`): A
  database of 40,711 master-level opening games to seed different opening
  positions during testing.

### Scripts

Lingine includes a comprehensive python script suite under [`scripts/`](scripts)
to automate engine validation, ELO estimation, and regression testing.

| Script                  | Usage                                                                                                                                                                                                                           |
| :---------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| run_match.py            | Run a round-robin tournament between any two engine binaries to determine ELO differences, draw ratios, and victory margins.                                                                                                    |
| run_gauntlet.py         | Assess the absolute ELO rating of an engine by putting it through a gauntlet tournament against a series of strength-limited standard baseline bots.                                                                            |
| run_historical_evals.py | Run a comprehensive suite of matches and gauntlets across all historical engine versions. This is used to map out the regression, progression, and absolute ELO impact of every single commit in Lingine's development history. |

### Working with the source

Build the debug build:

```bash
cargo build
```

Run tests:

```bash
cargo test
```

## Acknowledgemments

- `Sylvan` by Wilbert Lee and [contributors to the now deleted repository](https://github.com/EterCyber/Sylvan/graphs/contributors).
- [Reckless](https://github.com/codedeliveryservice/Reckless) and
  [Viridithas](https://github.com/cosmobobak/viridithas) for incredible Rust examples.
- [Fairy-Stockfish](https://github.com/fairy-stockfish/Fairy-Stockfish) and
  [Pikafish](https://github.com/official-pikafish/Pikafish) for
  being great Xiangqi engines that helps with testing and code examples.
- [Chess Programming Wiki](https://www.chessprogramming.org/) for resources on
  computer engines.
- [Gemini 3.1 Pro](https://gemini.google.com/app) for the logo (I am sorry).

## License

This project is licensed under the [GNU Affero General Public License v3.0](./LICENSE).
