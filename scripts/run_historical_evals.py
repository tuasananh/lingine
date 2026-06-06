#!/usr/bin/env python3
import os
import sys
import subprocess
import argparse
import time

import utils


def main():
    if not os.path.exists("Cargo.toml") or not os.path.exists("historical"):
        print(
            "\033[0;31mError: This script must be run from the repository root directory.\033[0m"
        )
        sys.exit(1)

    parser = argparse.ArgumentParser(
        description="Lingine Historical Engine Benchmark Suite"
    )
    parser.add_argument(
        "-m",
        "--matches-only",
        action="store_true",
        help="Run both neighbor matches and base matches",
    )
    parser.add_argument(
        "-g",
        "--gauntlets-only",
        action="store_true",
        help="Run gauntlet tournaments only",
    )
    parser.add_argument(
        "-n", "--neighbor-only", action="store_true", help="Run neighbor matches only"
    )
    parser.add_argument(
        "-b", "--base-only", action="store_true", help="Run base matches only"
    )
    parser.add_argument("-v", "--version", help="Only run tasks involving this version")
    parser.add_argument("-t", "--tc", help="Time control setting")
    parser.add_argument(
        "-c", "--concurrency", type=int, help="Number of parallel games"
    )
    parser.add_argument(
        "-f", "--force", action="store_true", help="Overwrite existing results"
    )
    parser.add_argument(
        "-d",
        "--dry-run",
        action="store_true",
        help="Show what would be run without executing",
    )
    parser.add_argument(
        "--gauntlet-games",
        type=int,
        default=500,
        help="Number of gauntlet games (default: 500)",
    )
    parser.add_argument(
        "--match-games",
        type=int,
        default=1000,
        help="Number of 1v1 match games (default: 1000)",
    )
    args = parser.parse_args()

    run_neighbor = args.neighbor_only or args.matches_only
    run_base = args.base_only or args.matches_only
    run_gauntlet = args.gauntlets_only

    if not run_neighbor and not run_base and not run_gauntlet:
        run_neighbor = run_base = run_gauntlet = True

    files = sorted(
        [
            f
            for f in os.listdir("historical")
            if f.startswith("lingine-")
            and os.path.isfile(os.path.join("historical", f))
        ]
    )
    if not files:
        print(
            "\033[0;31mError: No historical engines found in 'historical' matching 'lingine-*'!\033[0m"
        )
        sys.exit(1)

    versions = [os.path.join("historical", f) for f in files]
    clean_names = [f[len("lingine-") :] for f in files]

    if args.version and args.version not in clean_names:
        print(
            f"\033[0;31mError: Specified version '{args.version}' was not found!\033[0m"
        )
        sys.exit(1)

    neighbor_a, neighbor_b, neighbor_name_a, neighbor_name_b = [], [], [], []
    for i in range(len(versions) - 1):
        neighbor_a.append(versions[i])
        neighbor_b.append(versions[i + 1])
        neighbor_name_a.append(clean_names[i])
        neighbor_name_b.append(clean_names[i + 1])

    base_ver = versions[0]
    base_name = clean_names[0]
    base_a, base_b, base_name_a, base_name_b = [], [], [], []
    for i in range(1, len(versions)):
        base_a.append(versions[i])
        base_b.append(base_ver)
        base_name_a.append(clean_names[i])
        base_name_b.append(base_name)

    (
        active_neighbor_a,
        active_neighbor_b,
        active_neighbor_name_a,
        active_neighbor_name_b,
    ) = [], [], [], []
    if run_neighbor:
        for i in range(len(neighbor_a)):
            if not args.version or args.version in (
                neighbor_name_a[i],
                neighbor_name_b[i],
            ):
                active_neighbor_a.append(neighbor_a[i])
                active_neighbor_b.append(neighbor_b[i])
                active_neighbor_name_a.append(neighbor_name_a[i])
                active_neighbor_name_b.append(neighbor_name_b[i])

    active_base_a, active_base_b, active_base_name_a, active_base_name_b = (
        [],
        [],
        [],
        [],
    )
    if run_base:
        for i in range(len(base_a)):
            if not args.version or args.version in (base_name_a[i], base_name_b[i]):
                active_base_a.append(base_a[i])
                active_base_b.append(base_b[i])
                active_base_name_a.append(base_name_a[i])
                active_base_name_b.append(base_name_b[i])

    active_gauntlet_engines, active_gauntlet_names = [], []
    if run_gauntlet:
        for i in range(len(versions)):
            if not args.version or args.version == clean_names[i]:
                active_gauntlet_engines.append(versions[i])
                active_gauntlet_names.append(clean_names[i])

    utils.print_header("LINGINE HISTORICAL RUNNER ACTIVE PLAN")

    success_count, failed_count, skipped_count, run_count = 0, 0, 0, 0
    success_items, failed_items, skipped_items = [], [], []

    def run_match_task(eng_a, name_a, eng_b, name_b, out_dir):
        nonlocal success_count, failed_count, skipped_count, run_count
        sum_file = os.path.join(out_dir, "summary.md")
        pgn_file = os.path.join(out_dir, "records.pgn")

        if not args.force and os.path.exists(sum_file) and os.path.exists(pgn_file):
            print(
                f"\033[0;34m[SKIP]\033[0m Match {name_a} vs {name_b} already exists in {out_dir}"
            )
            skipped_count += 1
            skipped_items.append(f"Match: {name_a} vs {name_b}")
            return

        print(
            f"\033[0;33m[RUNNING]\033[0m Match {name_a} vs {name_b} ({args.match_games} games)"
        )

        cmd = [
            sys.executable,
            "scripts/run_match.py",
            "-a",
            eng_a,
            "--name-a",
            name_a,
            "-b",
            eng_b,
            "--name-b",
            name_b,
            "-g",
            str(args.match_games),
            "-s",
            "-o",
            out_dir,
        ]
        if args.tc:
            cmd.extend(["-t", args.tc])
        if args.concurrency:
            cmd.extend(["-c", str(args.concurrency)])

        if args.dry_run:
            print(f"          [DRY-RUN] Would run: {' '.join(cmd)}")
            run_count += 1
            return

        res = subprocess.run(cmd)
        if res.returncode == 0:
            success_count += 1
            success_items.append(f"Match: {name_a} vs {name_b}")
        else:
            failed_count += 1
            failed_items.append(f"Match: {name_a} vs {name_b} (Exit: {res.returncode})")

    def run_gauntlet_task(eng, name, out_dir):
        nonlocal success_count, failed_count, skipped_count, run_count
        sum_file = os.path.join(out_dir, "summary.md")
        pgn_file = os.path.join(out_dir, "records.pgn")

        if not args.force and os.path.exists(sum_file) and os.path.exists(pgn_file):
            print(
                f"\033[0;34m[SKIP]\033[0m Gauntlet for {name} already exists in {out_dir}"
            )
            skipped_count += 1
            skipped_items.append(f"Gauntlet: {name}")
            return

        print(
            f"\033[0;33m[RUNNING]\033[0m Gauntlet for {name} ({args.gauntlet_games} games)"
        )

        cmd = [
            sys.executable,
            "scripts/run_gauntlet.py",
            "-a",
            eng,
            "--name",
            name,
            "-g",
            str(args.gauntlet_games),
            "-s",
            "-o",
            out_dir,
        ]
        if args.tc:
            cmd.extend(["-t", args.tc])
        if args.concurrency:
            cmd.extend(["-c", str(args.concurrency)])

        if args.dry_run:
            print(f"          [DRY-RUN] Would run: {' '.join(cmd)}")
            run_count += 1
            return

        res = subprocess.run(cmd)
        if res.returncode == 0:
            success_count += 1
            success_items.append(f"Gauntlet: {name}")
        else:
            failed_count += 1
            failed_items.append(f"Gauntlet: {name} (Exit: {res.returncode})")

    start_time = time.time()

    if active_neighbor_a:
        utils.print_header("SECTION 1: NEIGHBOR MATCHES")
        for i in range(len(active_neighbor_a)):
            run_match_task(
                active_neighbor_a[i],
                active_neighbor_name_a[i],
                active_neighbor_b[i],
                active_neighbor_name_b[i],
                f"matches/historical/neighbor_matches/{active_neighbor_name_a[i]}-vs-{active_neighbor_name_b[i]}",
            )

    if active_base_a:
        utils.print_header("SECTION 2: BASE MATCHES VS 1.0.0-BASE")
        for i in range(len(active_base_a)):
            run_match_task(
                active_base_a[i],
                active_base_name_a[i],
                active_base_b[i],
                active_base_name_b[i],
                f"matches/historical/base_matches/{active_base_name_a[i]}-vs-{active_base_name_b[i]}",
            )

    if active_gauntlet_engines:
        utils.print_header("SECTION 3: GAUNTLET TOURNAMENTS")
        for i in range(len(active_gauntlet_engines)):
            run_gauntlet_task(
                active_gauntlet_engines[i],
                active_gauntlet_names[i],
                f"matches/historical/gauntlets/{active_gauntlet_names[i]}",
            )

    elapsed = time.time() - start_time
    utils.print_header("EVALUATION RUN COMPLETE")
    print(f"Total Execution Time: {utils.format_time(elapsed)}")

    if args.dry_run:
        print("\n[DRY-RUN] Generating comprehensive version reports...")
    else:
        print("\nGenerating comprehensive version reports...")
        if args.version:
            subprocess.run(
                [
                    sys.executable,
                    "scripts/compile_version_report.py",
                    "--version",
                    args.version,
                ]
            )
        else:
            for ver in clean_names:
                if os.path.exists(
                    f"matches/historical/gauntlets/{ver}"
                ) or os.path.exists(
                    f"matches/historical/base_matches/{ver}-vs-{clean_names[0]}"
                ):
                    subprocess.run(
                        [
                            sys.executable,
                            "scripts/compile_version_report.py",
                            "--version",
                            ver,
                        ]
                    )


if __name__ == "__main__":
    main()
