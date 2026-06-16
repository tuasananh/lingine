#!/usr/bin/env python3
"""
generate_games.py
=================
Generate games from FairyStockfish playing against itself using sylvan-cli.
Outputs games in minimal PGN format with UCI moves to facilitate Texel tuning.
"""

import os
import sys
import subprocess
import argparse

# Add scripts directory to path if needed (though Python does this automatically when executing the script)
sys.path.append(os.path.dirname(os.path.abspath(__file__)))
import utils


def check_dependencies(args):
    required_files = {
        "tools/sylvan-cli": "Tournament coordinator sylvan-cli",
        "tools/fairy-stockfish_x86-64": "Fairy-Stockfish engine",
        args.openings: "Opening database file",
    }
    utils.check_dependencies(required_files)


def main():
    parser = argparse.ArgumentParser(
        description="FairyStockfish Game Generator for Texel Tuning"
    )
    parser.add_argument(
        "-g",
        "--games",
        type=int,
        default=100000,
        help="Total number of games (default: 100000)",
    )
    parser.add_argument(
        "-t",
        "--tc",
        default="1+0.08",
        help="Time control (default: 1+0.08)",
    )
    parser.add_argument(
        "-c",
        "--concurrency",
        type=int,
        help="Parallel games (default: auto)",
    )
    parser.add_argument(
        "-o",
        "--pgnout",
        default="tools/xqdb_generated_games.pgn",
        help="Output PGN file path (default: tools/xqdb_generated_games.pgn)",
    )
    parser.add_argument(
        "-f",
        "--openings",
        default="tools/xqdb_masters_40711_UCI_games.pgn",
        help="Path to openings file (default: tools/xqdb_masters_40711_UCI_games.pgn)",
    )
    parser.add_argument(
        "-d",
        "--depth",
        type=int,
        default=12,
        help="Opening book ply depth (default: 12)",
    )

    args = parser.parse_args()

    utils.print_header("FAIRY-STOCKFISH GAME GENERATOR FOR TEXEL TUNING")
    check_dependencies(args)

    concurrency = utils.get_optimal_concurrency(args.concurrency)

    print("\nConfiguration:")
    print(f"  -> Total Games:  {args.games}")
    print(f"  -> Concurrency:  {concurrency}")
    print(f"  -> Time Control: {args.tc}")
    print(f"  -> Opening Book: {args.openings}")
    print(f"  -> Book Depth:   {args.depth} plies")
    print(f"  -> Output PGN:   {args.pgnout}")

    # Remove old PGN if it exists
    if os.path.exists(args.pgnout):
        print(f"\nRemoving existing output file: {args.pgnout}")
        os.remove(args.pgnout)

    # Ensure parent directory exists
    outdir = os.path.dirname(args.pgnout)
    if outdir:
        os.makedirs(outdir, exist_ok=True)

    cli_args = [
        "./tools/sylvan-cli",
        "-engine",
        "cmd=./tools/fairy-stockfish_x86-64",
        "name=FS1",
        "proto=uci",
        "option.Hash=16",
        "-engine",
        "cmd=./tools/fairy-stockfish_x86-64",
        "name=FS2",
        "proto=uci",
        "option.Hash=16",
        "-each",
        "proto=uci",
        f"tc={args.tc}",
        "-openings",
        f"file={args.openings}",
        "format=pgn",
        f"plies={args.depth}",
        "-tournament",
        "round-robin",
        "-games",
        str(args.games),
        "-repeat",
        "-concurrency",
        str(concurrency),
        "-pgnout",
        args.pgnout,
        "min",
        "uci",
    ]

    print("\nLaunching match tournament using sylvan-cli...")
    try:
        subprocess.run(cli_args, check=True)
        print(
            f"\nGame generation completed successfully! Output saved to: {args.pgnout}"
        )
    except subprocess.CalledProcessError as e:
        print(f"\nError running game generation. Exit code: {e.returncode}")
        sys.exit(1)


if __name__ == "__main__":
    main()
