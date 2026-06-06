import os
import sys
import subprocess
import shutil

def print_header(title):
    print("=" * 70)
    print(f"   {title.upper()}   ")
    print("=" * 70)

def check_dependencies(required_files):
    missing = []
    for filepath, description in required_files.items():
        if not os.path.exists(filepath):
            missing.append(f"- {description} ({filepath})")

    if missing:
        print_header("ERROR: MISSING TOOLS OR DATA")
        print("Please run the installation script before starting:")
        print("  python3 scripts/setup_tools.py\n")
        print("Current missing components:")
        for item in missing:
            print(item)
        print("=" * 70)
        sys.exit(1)

def build_engine():
    print("\n[1/3] Compiling Lingine (cargo build --release)...")
    try:
        result = subprocess.run(
            ["cargo", "build", "--release"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        if result.returncode != 0:
            print_header("RUST PROJECT COMPILATION ERROR")
            print(result.stderr)
            sys.exit(1)
        print("=> Compilation successful: ./target/release/lingine")
    except FileNotFoundError:
        print_header("ERROR: RUST/CARGO NOT FOUND")
        print("Please make sure Rust (cargo) is installed on your system.")
        sys.exit(1)

def get_optimal_concurrency(user_concurrency=None):
    if user_concurrency is not None:
        return user_concurrency
    cores = os.cpu_count() or 4
    # Optimize at 1 engine per core (2 engines per game, hence cores // 2)
    # Cap at 20 parallel threads to avoid OS process scheduling overhead
    return min(20, max(1, cores // 2))

def format_time(seconds):
    seconds = int(seconds)
    if seconds >= 3600:
        return f"{seconds // 3600}h {(seconds % 3600) // 60}m {seconds % 60}s"
    elif seconds >= 60:
        return f"{seconds // 60}m {seconds % 60}s"
    else:
        return f"{seconds}s"
