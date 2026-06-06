#!/usr/bin/env python3
import os
import sys
import subprocess
import re
import math
import argparse


import utils


def check_dependencies():
    required_files = {
        "tools/sylvan-cli": "Tournament coordinator sylvan-cli",
        "tools/fairy-stockfish_x86-64": "Fairy-Stockfish baseline opponent engine",
        "tools/xqdb_masters_40711_UCI_games.pgn": "Masters opening database PGN",
    }
    utils.check_dependencies(required_files)


def run_tournament(args):
    """Execute the gauntlet tournament via sylvan-cli with configured options."""
    concurrency = utils.get_optimal_concurrency(args.concurrency)
    concurrency_msg = (
        f"{concurrency} (User configured)"
        if args.concurrency is not None
        else f"{concurrency} (Auto-optimized)"
    )

    # Parse opponent ELO list
    try:
        elo_list = [int(x.strip()) for x in args.elos.split(",")]
    except ValueError:
        print("Error: Invalid opponent ELO list. Correct example: 1000,1200,1400")
        sys.exit(1)

    total_games = len(elo_list) * args.games

    print("\n[2/3] Launching ELO Gauntlet tournament...")
    print(f"  -> Parallel matches (concurrency): {concurrency_msg}")
    print(f"  -> Opponent ELO benchmarks: {', '.join(map(str, elo_list))}")
    print(f"  -> Games per opponent tier: {args.games} (Total: {total_games} games)")
    print(f"  -> Time Control configuration: {args.tc}")
    print(f"  -> Opening book depth: {args.depth} plies")
    print(f"  -> Output PGN records file: {args.pgnout}")

    # Delete old gauntlet PGN to avoid score contamination
    if os.path.exists(args.pgnout):
        os.remove(args.pgnout)

    cmd = [
        "./tools/sylvan-cli",
        "-engine",
        f"cmd={args.engine}",
        f"name={args.name}",
        f"stderr={args.name}_err.log",
    ]

    # Dynamically register Fairy-Stockfish engine instances for each ELO milestone
    for elo in elo_list:
        cmd.extend(
            [
                "-engine",
                "cmd=./tools/fairy-stockfish_x86-64",
                f"name=FS-{elo}",
                "option.UCI_LimitStrength=true",
                f"option.UCI_Elo={elo}",
            ]
        )

    cmd.extend(
        [
            "-each",
            "proto=uci",
            f"tc={args.tc}",
            "option.Hash=16",
            "-openings",
            f"file={args.openings_file}",
            "format=pgn",
            f"plies={args.depth}",
            "-tournament",
            "gauntlet",
            "-games",
            str(args.games),
            "-repeat",
            "-concurrency",
            str(concurrency),
            "-pgnout",
            args.pgnout,
        ]
    )

    try:
        subprocess.run(cmd, check=True)
        print("=> Gauntlet tournament completed successfully!")
    except subprocess.CalledProcessError as e:
        utils.print_header("SYLVAN-CLI EXECUTION ERROR")
        print(
            f"Sylvan-CLI encountered an issue running the tournament. Exit code: {e.returncode}"
        )
        sys.exit(1)


def parse_pgn(pgn_path):
    if not os.path.exists(pgn_path):
        return []
    with open(pgn_path, "r", encoding="utf-8") as f:
        content = f.read()

    games_raw = content.split("[Event ")
    results = []

    for game in games_raw:
        if not game.strip():
            continue
        white_match = re.search(r'\[(?:White|Red)\s+"([^"]+)"\]', game)
        black_match = re.search(r'\[Black\s+"([^"]+)"\]', game)
        result_match = re.search(r'\[Result\s+"([^"]+)"\]', game)

        if white_match and black_match and result_match:
            results.append(
                {
                    "white": white_match.group(1),
                    "black": black_match.group(1),
                    "result": result_match.group(1),
                }
            )
    return results


def calculate_and_display_elo(args):
    """Parse the PGN results file and calculate absolute ELO rating."""
    print("\n[3/3] Analyzing match outcomes and estimating ELO...")
    games = parse_pgn(args.pgnout)

    if not games:
        print(f"Error: No match records found in '{args.pgnout}' for ELO analysis.")
        sys.exit(1)

    # Parse configured opponent ELOs
    opponent_elos = {}
    for elo_str in args.elos.split(","):
        elo_val = int(elo_str.strip())
        opponent_elos[f"FS-{elo_val}"] = elo_val

    stats = {}
    for game in games:
        w, b, res = game["white"], game["black"], game["result"]
        if w == args.name:
            opponent = b
            lingine_color = "white"
        elif b == args.name:
            opponent = w
            lingine_color = "black"
        else:
            continue

        if opponent not in stats:
            stats[opponent] = {"wins": 0, "draws": 0, "losses": 0, "games": 0}

        stats[opponent]["games"] += 1

        if res == "1-0":
            if lingine_color == "white":
                stats[opponent]["wins"] += 1
            else:
                stats[opponent]["losses"] += 1
        elif res == "0-1":
            if lingine_color == "black":
                stats[opponent]["wins"] += 1
            else:
                stats[opponent]["losses"] += 1
        elif res in ["1/2-1/2", "0.5-0.5"]:
            stats[opponent]["draws"] += 1
        else:
            stats[opponent]["draws"] += 1

    print("\n" + "=" * 70)
    print(
        f"           ELO PERFORMANCE ANALYSIS REPORT - {args.name.upper()}             "
    )
    print("=" * 70)
    print(
        f"{'Opponent':<15}{'Games':<8}{'Wins':<8}{'Draws':<8}{'Losses':<8}{'Score %':<12}{'Est. ELO':<15}"
    )
    print("-" * 70)

    elo_estimates = []
    # Only sort and display configured benchmark opponents
    sorted_opponents = sorted(
        [k for k in stats.keys() if k in opponent_elos],
        key=lambda x: opponent_elos.get(x, 1200),
    )

    for opp in sorted_opponents:
        s = stats[opp]
        opp_base_elo = opponent_elos.get(opp)
        if opp_base_elo is None:
            continue

        wins, draws, losses, n = s["wins"], s["draws"], s["losses"], s["games"]
        points = wins + 0.5 * draws
        score_pct = points / n

        if score_pct >= 0.99:
            delta_elo = 400
        elif score_pct <= 0.01:
            delta_elo = -400
        else:
            delta_elo = 400 * math.log10(score_pct / (1 - score_pct))

        estimated_elo = opp_base_elo + delta_elo
        elo_estimates.append(estimated_elo)

        score_str = f"{score_pct * 100:.1f}%"
        elo_str = f"{int(round(estimated_elo))} ELO"

        print(
            f"{opp:<15}{n:<8}{wins:<8}{draws:<8}{losses:<8}{score_str:<12}{elo_str:<15}"
        )

    print("-" * 70)
    if elo_estimates:
        final_elo = sum(elo_estimates) / len(elo_estimates)
        print(
            f"\n=> AVERAGE ESTIMATED ELO RATING FOR {args.name.upper()}: {int(round(final_elo))} ELO"
        )
    else:
        print("\nInsufficient match data available to calculate ELO estimates.")
    print("=" * 70 + "\n")


def main():
    parser = argparse.ArgumentParser(
        description="Tournament coordinator and ELO evaluator for Xiangqi/Chinese Chess engines."
    )
    parser.add_argument(
        "-c",
        "--cores",
        "--concurrency",
        type=int,
        default=None,
        dest="concurrency",
        help="Number of concurrent matches to run in parallel (default: auto-optimized)",
    )
    parser.add_argument(
        "-g",
        "--games",
        type=int,
        default=500,
        help="Number of games to play against each ELO tier opponent (default: 500)",
    )
    parser.add_argument(
        "-t",
        "--tc",
        type=str,
        default="3+0.03",
        help="Time control setting (default: 3+0.03)",
    )
    parser.add_argument(
        "-d",
        "--depth",
        type=int,
        default=12,
        help="Forced opening book depth in plies (default: 12)",
    )
    parser.add_argument(
        "-f",
        "--openings-file",
        type=str,
        default="tools/xqdb_masters_40711_UCI_games.pgn",
        help="Path to the openings PGN/EPD database (default: tools/xqdb_masters_40711_UCI_games.pgn)",
    )
    parser.add_argument(
        "-a",
        "--engine",
        "--bot-path",
        type=str,
        default=None,
        dest="engine",
        help="Path to the main engine binary under test (default: ./target/release/lingine)",
    )
    parser.add_argument(
        "--name",
        "--bot-name",
        type=str,
        default=None,
        dest="name",
        help="Name of the main engine under test (default: Lingine)",
    )
    parser.add_argument(
        "-o",
        "--pgnout",
        "--outdir",
        type=str,
        default=None,
        dest="outdir",
        help="Output directory to store tournament results (default: matches/BOT_NAME_TIMESTAMP)",
    )
    parser.add_argument(
        "-e",
        "--elos",
        type=str,
        default="1200,1400,1600,1800,2000,2200",
        help="Comma-separated list of baseline opponent ELO bounds (default: 1200,1400,1600,1800,2000,2200)",
    )
    parser.add_argument(
        "-s",
        "--skip-build",
        action="store_true",
        help="Skip automatic Rust compilation of target/release/lingine",
    )

    args = parser.parse_args()

    # Interactive engine prompts if not provided
    engine_path = args.engine
    engine_name = args.name

    if not engine_path:
        print("\033[33mNo main engine path specified.\033[0m")
        user_engine = input(
            "Enter main engine path (Default: ./target/release/lingine): "
        ).strip()
        if not user_engine:
            engine_path = "./target/release/lingine"
        else:
            engine_path = user_engine

    if not engine_name or not engine_name.strip():
        default_name = os.path.basename(engine_path) if engine_path else "Lingine"
        if default_name.lower() == "lingine":
            default_name = "Lingine"
        print("\033[33mNo main engine name specified.\033[0m")
        user_name = input(f"Enter main engine name (Default: {default_name}): ").strip()
        if not user_name:
            engine_name = default_name
        else:
            engine_name = user_name

    # Absolute fallback to prevent empty names
    if not engine_name or not engine_name.strip():
        engine_name = "gauntlet"

    import datetime

    outdir = args.outdir
    if not outdir:
        default_outdir = (
            f"matches/{engine_name}_{datetime.datetime.now().strftime('%Y%m%d_%H%M%S')}"
        )
        print("\033[33mNo output folder specified.\033[0m")
        user_outdir = input(
            f"Enter gauntlet name or output folder (Press Enter for default: {default_outdir}): "
        ).strip()
        if not user_outdir:
            outdir = default_outdir
        else:
            if "/" not in user_outdir:
                outdir = f"matches/{user_outdir}"
            else:
                outdir = user_outdir

    os.makedirs(outdir, exist_ok=True)
    pgnout = os.path.join(outdir, "records.pgn")
    args.pgnout = pgnout
    args.outdir = outdir
    args.engine = engine_path
    args.name = engine_name

    utils.print_header("LINGINE GAUNTLET TOURNAMENT & ELO EVALUATOR")
    check_dependencies()

    if not args.skip_build and args.engine == "./target/release/lingine":
        utils.build_engine()
    else:
        print(
            f"\n[1/3] Skipped engine compilation (Main engine path: '{args.engine}')."
        )

    run_tournament(args)
    calculate_and_display_elo(args)

    print("\n=> Executing detailed game outcomes analysis and generating summary.md...")
    try:
        analyze_cmd = [
            sys.executable,
            "scripts/analyze_gauntlet.py",
            args.pgnout,
            "-b",
            args.name,
            "--markdown",
            os.path.join(args.outdir, "summary.md"),
        ]
        subprocess.run(analyze_cmd, check=True)
        print(
            f"=> Analysis completed! Performance summaries saved to: {os.path.join(args.outdir, 'summary.md')}"
        )
    except Exception as e:
        print(f"Error executing gauntlet outcome analysis: {e}")


if __name__ == "__main__":
    main()
