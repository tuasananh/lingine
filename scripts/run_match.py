#!/usr/bin/env python3
import os
import sys
import subprocess
import argparse
import datetime

import utils


def check_dependencies(args):
    required_files = {
        "tools/sylvan-cli": "Tournament coordinator sylvan-cli",
        args.openings: "Opening database file",
    }

    if args.engine_a != "./target/release/lingine" and not os.path.exists(
        args.engine_a
    ):
        required_files[args.engine_a] = "Engine A binary"
    if args.engine_b != "./target/release/lingine" and not os.path.exists(
        args.engine_b
    ):
        required_files[args.engine_b] = "Engine B binary"

    utils.check_dependencies(required_files)


def main():
    parser = argparse.ArgumentParser(
        description="Lingine 2-Player Match Runner & ELO Evaluator"
    )
    parser.add_argument("-a", "--engine-a", required=True, help="Path to Engine A")
    parser.add_argument("--name-a", help="Name of Engine A")
    parser.add_argument("--options-a", default="", help="Custom options for Engine A")
    parser.add_argument("-b", "--engine-b", required=True, help="Path to Engine B")
    parser.add_argument("--name-b", help="Name of Engine B")
    parser.add_argument("--options-b", default="", help="Custom options for Engine B")
    parser.add_argument(
        "-g",
        "--games",
        type=int,
        default=1000,
        help="Total number of games (default: 1000)",
    )
    parser.add_argument(
        "-t", "--tc", default="3+0.03", help="Time control (default: 3+0.03)"
    )
    parser.add_argument(
        "-c", "--concurrency", type=int, help="Parallel games (default: auto)"
    )
    parser.add_argument(
        "-d",
        "--depth",
        type=int,
        default=12,
        help="Opening book ply depth (default: 12)",
    )
    parser.add_argument(
        "-f",
        "--openings",
        default="tools/xqdb_masters_40711_UCI_games.pgn",
        help="Path to openings file",
    )
    parser.add_argument("-o", "--outdir", "--pgnout", help="Output directory")
    parser.add_argument(
        "-s", "--skip-build", action="store_true", help="Skip cargo build"
    )
    parser.add_argument("--sprt", help="SPRT termination parameters")

    args = parser.parse_args()

    args.name_a = args.name_a or os.path.basename(args.engine_a)
    args.name_b = args.name_b or os.path.basename(args.engine_b)

    if not args.outdir:
        default_outdir = f"matches/{args.name_a}-vs-{args.name_b}_{datetime.datetime.now().strftime('%Y%m%d_%H%M%S')}"
        print("\033[33mNo output folder specified.\033[0m")
        user_outdir = input(
            f"Enter match name or folder (Press Enter for default: {default_outdir}): "
        ).strip()
        if not user_outdir:
            args.outdir = default_outdir
        else:
            args.outdir = (
                f"matches/{user_outdir}" if "/" not in user_outdir else user_outdir
            )

    os.makedirs(args.outdir, exist_ok=True)
    pgnout = os.path.join(args.outdir, "records.pgn")

    utils.print_header("LINGINE 2-PLAYER MATCH RUNNER & ELO EVALUATOR")

    check_dependencies(args)

    if not args.skip_build and (
        "./target/release/lingine" in (args.engine_a, args.engine_b)
    ):
        utils.build_engine()
    else:
        print("\n[1/3] Skipping Lingine compilation.")

    concurrency = utils.get_optimal_concurrency(args.concurrency)

    if os.path.exists(pgnout):
        os.remove(pgnout)

    print("\n[2/3] Match Configuration:")
    print(f"  -> Engine A: {args.name_a} ({args.engine_a}) {args.options_a}")
    print(f"  -> Engine B: {args.name_b} ({args.engine_b}) {args.options_b}")
    print(f"  -> Total Games: {args.games}")
    print(f"  -> Concurrency: {concurrency}")

    engine_a_args = [
        "-engine",
        f"cmd={args.engine_a}",
        f"name={args.name_a}",
        f"stderr={args.name_a}_err.log",
    ]
    if args.options_a:
        engine_a_args.extend(args.options_a.split())

    engine_b_args = [
        "-engine",
        f"cmd={args.engine_b}",
        f"name={args.name_b}",
        f"stderr={args.name_b}_err.log",
    ]
    if args.options_b:
        engine_b_args.extend(args.options_b.split())

    cli_args = (
        ["./tools/sylvan-cli"]
        + engine_a_args
        + engine_b_args
        + [
            "-each",
            "proto=uci",
            f"tc={args.tc}",
            "option.Hash=16",
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
            pgnout,
        ]
    )

    if args.sprt:
        cli_args.extend(["-sprt", args.sprt])

    print("\n[3/3] Launching match tournament using sylvan-cli...")
    try:
        subprocess.run(cli_args, check=True)
        print(f"\nMatch completed successfully! Results saved to {pgnout}")

        # Run match analysis
        print("Running match analysis...")
        subprocess.run(
            [
                sys.executable,
                "scripts/analyze_match.py",
                pgnout,
                "-m",
                os.path.join(args.outdir, "summary.md"),
            ],
            check=True,
        )
    except subprocess.CalledProcessError as e:
        print(f"\nError running tournament. Exit code: {e.returncode}")
        sys.exit(1)


if __name__ == "__main__":
    main()
