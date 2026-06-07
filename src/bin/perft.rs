use clap::Parser;

/// Command line arguments for the perft (Performance Move Generation) tool.
#[derive(Parser, Debug)]
#[command(
    version,
    about = "Xiangqi (Chinese Chess) perft move generation benchmark tool",
    long_about = "Xiangqi perft tool that calculates the number of leaf nodes, checks, captures, and checkmates reachable from a given position. Includes built-in verification against a standard table of records."
)]
struct Args {
    /// Board position in FEN notation.
    /// If both --fen and --index are omitted, this defaults to standard initial
    /// position (index 1).
    #[arg(short, long)]
    fen: Option<String>,

    /// Index of the standard perft position (1-11) to run. Overrides --fen.
    /// If both --fen and --index are omitted, defaults to 1.
    #[arg(short, long)]
    index: Option<usize>,

    /// Target search depth to compute node counts and statistics.
    #[arg(short, long, default_value_t = 5)]
    depth: u32,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // If both index and fen are None, default to index = Some(1)
    let (index, fen_arg) = match (args.index, args.fen) {
        (None, None) => (Some(1), None),
        (idx, fen) => (idx, fen),
    };

    let positions = perft_positions();
    let (fen, expected_record) = if let Some(idx) = index {
        if idx < 1 || idx > positions.len() {
            anyhow::bail!(
                "Invalid position index: {}. Must be between 1 and {}.",
                idx,
                positions.len()
            );
        }
        let pos = &positions[idx - 1];
        println!(
            "Running perft for standard position {} ({}):",
            idx, pos.description
        );
        println!("FEN: {}", pos.fen);

        // Try to find the expected record for this specific depth if it exists in our
        // static table
        let expected = pos.expected.iter().find(|e| e.depth == args.depth).copied();
        (pos.fen.clone(), expected)
    } else {
        // fen_arg is guaranteed to be Some because if both were None, we fell back to
        // index 1 above.
        (fen_arg.unwrap(), None)
    };

    let actual = Perft::perft(&fen, args.depth)?;

    if let Some(expected) = expected_record {
        println!(
            "\nVerification against expected table record (depth {}):",
            args.depth
        );
        if actual == expected {
            println!("✅ SUCCESS: Results match the expected table records exactly!");
        } else {
            println!("❌ FAILURE: Results MISMATCH the expected table records!");
            println!("Expected: {:?}", expected);
            println!("Actual:   {:?}", actual);
            anyhow::bail!("Perft verification failed against the expected table record.");
        }
    }

    Ok(())
}

use anyhow::Result;
use std::sync::OnceLock;

use lingine::core::{MoveGenType, MoveList, Position, generate_moves};

/// PerftExpected defines the expected numbers of nodes, checks, captures,
/// and checkmates at a specific depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerftExpected {
    pub depth: u32,
    pub nodes: u64,
    pub checks: u64,
    pub captures: u64,
    pub mates: u64,
}

/// PerftPosition defines a standard test position with its FEN representation,
/// description, and array of expected statistics at various depths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerftPosition {
    pub index: usize,
    pub fen: String,
    pub description: String,
    pub expected: Vec<PerftExpected>,
}

static PERFT_POSITIONS: OnceLock<Vec<PerftPosition>> = OnceLock::new();

/// Returns the list of standard perft positions, parsed dynamically on first
/// call from the compile-time included `perft_positions.txt` file.
pub fn perft_positions() -> &'static [PerftPosition] {
    PERFT_POSITIONS.get_or_init(|| parse_positions(include_str!("perft_positions.txt")))
}

/// Dynamic parser for the custom perft_positions.txt format.
/// Parses positions separated by "---", supporting comments starting with '#'
/// and standard key-value fields.
fn parse_positions(input: &str) -> Vec<PerftPosition> {
    let mut positions = Vec::new();
    for segment in input.split("---") {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }

        let mut index = 0;
        let mut description = String::new();
        let mut fen = String::new();
        let mut expected = Vec::new();
        let mut in_expected = false;

        for line in segment.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line == "expected:" {
                in_expected = true;
                continue;
            }

            if in_expected {
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() == 5
                    && let (Ok(depth), Ok(nodes), Ok(checks), Ok(captures), Ok(mates)) = (
                        parts[0].parse::<u32>(),
                        parts[1].parse::<u64>(),
                        parts[2].parse::<u64>(),
                        parts[3].parse::<u64>(),
                        parts[4].parse::<u64>(),
                    )
                {
                    expected.push(PerftExpected {
                        depth,
                        nodes,
                        checks,
                        captures,
                        mates,
                    });
                }
            } else if let Some((key, val)) = line.split_once(':') {
                let val = val.trim();
                match key.trim() {
                    "index" => {
                        if let Ok(idx) = val.parse::<usize>() {
                            index = idx;
                        }
                    }
                    "description" => description = val.to_string(),
                    "fen" => fen = val.to_string(),
                    _ => {}
                }
            }
        }

        if index > 0 && !fen.is_empty() {
            positions.push(PerftPosition {
                index,
                fen,
                description,
                expected,
            });
        }
    }
    positions
}

/// Perft is a benchmarking tool for chess engines that counts the number of
/// possible positions (nodes) reachable from a given position up to a certain
/// depth. It also counts specific move types such as checks, captures, and
/// mates. This implementation is designed for Chinese Chess (Xiangqi) and
/// includes detailed statistics for each move at the root level.
#[derive(Debug, PartialEq, Eq)]
pub struct Perft {
    nodes: u64,
    checks: u64,
    captures: u64,
    mates: u64,
}

impl Default for Perft {
    fn default() -> Self {
        Self::new()
    }
}

impl Perft {
    pub fn new() -> Self {
        Self {
            nodes: 0,
            checks: 0,
            captures: 0,
            mates: 0,
        }
    }

    /// Runs the perft test for a given FEN string and depth. It initializes a
    /// position from the FEN, then calls the recursive helper function to
    /// count nodes, checks, captures, and mates. The results are printed to
    /// the console.
    pub fn perft(fen: &str, depth: u32) -> Result<PerftExpected> {
        let mut helper = Perft::default();
        let mut pos = Position::new();
        pos.set(fen)?;
        helper.perft_helper::<true>(&mut pos, depth);

        println!("Perft results for depth {depth}:\n{helper:?}");
        Ok(PerftExpected {
            depth,
            nodes: helper.nodes,
            checks: helper.checks,
            captures: helper.captures,
            mates: helper.mates,
        })
    }

    fn perft_helper<const ROOT: bool>(&mut self, pos: &mut Position, depth: u32) -> u64 {
        let mut cnt;
        let mut sub_nodes = 0;

        let mut moves = MoveList::new();
        generate_moves(pos, MoveGenType::Legal, &mut moves);

        let start_timepoint = if ROOT {
            Some(std::time::Instant::now())
        } else {
            None
        };

        let leaf = depth == 2;

        for m in moves {
            if ROOT && depth <= 1 {
                cnt = 1;
                self.nodes += 1;
                sub_nodes += 1;

                if !pos.is_empty(m.to()) {
                    self.captures += 1;
                }
                if pos.gives_check(m) {
                    self.checks += 1;
                }
            } else {
                pos.do_move(m);

                if leaf {
                    let mut next_moves = MoveList::new();
                    generate_moves(pos, MoveGenType::Legal, &mut next_moves);

                    cnt = next_moves.len() as u64;

                    if !next_moves.is_empty() {
                        for nm in next_moves {
                            if !pos.is_empty(nm.to()) {
                                self.captures += 1;
                            }

                            if pos.gives_check(nm) {
                                self.checks += 1;
                            }
                        }
                    } else {
                        self.mates += 1;
                    }

                    self.nodes += cnt;
                    sub_nodes += cnt;
                } else {
                    cnt = self.perft_helper::<false>(pos, depth - 1);
                    sub_nodes += cnt;
                }

                pos.undo_move();
            }

            if ROOT {
                println!("Move {}: {}", m, cnt);
            }
        }

        if ROOT {
            let elapsed = start_timepoint.unwrap().elapsed();
            let elapsed_secs = elapsed.as_secs_f64();
            println!("Time taken: {:.2?}", elapsed);
            if elapsed_secs > 0.0 {
                let nps = self.nodes as f64 / elapsed_secs;
                if nps >= 1_000_000.0 {
                    println!("Speed: {:.2} MNPS ({:.0} nps)", nps / 1_000_000.0, nps);
                } else {
                    println!("Speed: {:.0} nps", nps);
                }
            }
        }

        sub_nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_perft(fen: &str, depth: u32, expected: PerftExpected) -> Result<()> {
        let result = Perft::perft(fen, depth)?;
        assert_eq!(
            result, expected,
            "Mismatch at depth {} for FEN {}",
            depth, fen
        );
        Ok(())
    }

    fn assert_position_depths<R>(idx: usize, depths: R) -> Result<()>
    where
        R: IntoIterator<Item = u32>,
    {
        let positions = perft_positions();
        let pos = &positions[idx - 1];
        for depth in depths {
            if let Some(&exp) = pos.expected.iter().find(|e| e.depth == depth) {
                assert_perft(&pos.fen, depth, exp)?;
            }
        }
        Ok(())
    }

    macro_rules! perft_test {
        ($name:ident, $idx:expr, $depths:expr) => {
            #[test]
            fn $name() -> Result<()> {
                assert_position_depths($idx, $depths)
            }
        };
    }

    perft_test!(test_perft_position_1, 1, 1..=5);
    perft_test!(test_perft_position_2, 2, 1..=5);
    perft_test!(test_perft_position_3, 3, 1..=5);
    perft_test!(test_perft_position_4, 4, 1..=6);
    perft_test!(test_perft_position_5, 5, 1..=5);
    perft_test!(test_perft_position_6, 6, 1..=6);
    perft_test!(test_perft_position_7, 7, 1..=6);
    perft_test!(test_perft_position_8, 8, 1..=6);
    perft_test!(test_perft_position_9, 9, 1..=6);
    perft_test!(test_perft_position_10, 10, 1..=5);
    perft_test!(test_perft_position_11, 11, 1..=5);
}
