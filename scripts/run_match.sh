#!/bin/bash
# ======================================================================
#   LINGINE 2-PLAYER MATCH RUNNER & ELO EVALUATOR
# ======================================================================
# Script to conduct a head-to-head match between 2 engines (or 2 versions
# of Lingine) to determine ELO difference and performance statistics.
# ======================================================================

set -e

# ANSI color codes for terminal formatting
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
# MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Print the main header banner
print_header() {
  echo -e "${CYAN}======================================================================${NC}"
  echo -e "${CYAN}${BOLD}       LINGINE 2-PLAYER MATCH RUNNER & ELO EVALUATOR                  ${NC}"
  echo -e "${CYAN}======================================================================${NC}"
}

# Default parameters
ENGINE_A=""
NAME_A=""
OPTIONS_A=""
ENGINE_B=""
NAME_B=""
OPTIONS_B=""
GAMES=40
TC="3+0.03"
DEPTH=12
OPENINGS="tools/xqdb_masters_40711_UCI_games.pgn"
OUTDIR=""
SKIP_BUILD=false
SPRT=""

# Auto-detect optimal CPU concurrency
CPU_CORES=$(nproc 2>/dev/null || echo 4)
# Default to half the cores since each parallel game runs 2 independent engine instances
CONCURRENCY=$((CPU_CORES / 2))
if [ "$CONCURRENCY" -lt 1 ]; then
  CONCURRENCY=1
fi

show_help() {
  echo "Usage: $0 [options]"
  echo ""
  echo "Match Configuration Options:"
  echo "  -a, --engine-a PATH     Path to Engine A (Required)"
  echo "  --name-a NAME           Name of Engine A (Default: derived from file name)"
  echo "  --options-a OPTIONS     Custom command-line options for Engine A (Default: none)"
  echo "  -b, --engine-b PATH     Path to Engine B (Required)"
  echo "  --name-b NAME           Name of Engine B (Default: derived from file name)"
  echo "  --options-b OPTIONS     Custom command-line options for Engine B (Default: none)"
  echo "  -g, --games N           Total number of games to play (Default: $GAMES)"
  echo "  -t, --tc TIMECONTROL    Time control setting (Default: $TC)"
  echo "  -c, --concurrency N     Number of games to run in parallel (Default: auto-optimized = $CONCURRENCY)"
  echo "  -d, --depth N           Opening book ply depth (Default: $DEPTH)"
  echo "  -f, --openings PATH     Path to PGN/EPD openings file (Default: $OPENINGS)"
  echo "  -o, --outdir, --pgnout DIR   Output directory to store PGN and Markdown summary (Default: matches/NAME_A-vs-NAME_B_YYYYMMDD_HHMMSS)"
  echo "  -s, --skip-build        Skip automatic Rust cargo compilation for Lingine"
  echo "  --sprt PARAMS           Configure SPRT termination (e.g., \"elo0=0 elo1=10 alpha=0.05 beta=0.05\")"
  echo "  -h, --help              Show this help message"
  echo ""
  echo "Example of comparing current build against Fairy-Stockfish 1400:"
  echo "  $0 -a ./target/release/lingine -b ./tools/fairy-stockfish_x86-64 --options-b \"option.UCI_LimitStrength=true option.UCI_Elo=1400\""
  exit 0
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
  case $1 in
  -a | --engine-a)
    ENGINE_A="$2"
    shift 2
    ;;
  --name-a)
    NAME_A="$2"
    shift 2
    ;;
  --options-a)
    OPTIONS_A="$2"
    shift 2
    ;;
  -b | --engine-b)
    ENGINE_B="$2"
    shift 2
    ;;
  --name-b)
    NAME_B="$2"
    shift 2
    ;;
  --options-b)
    OPTIONS_B="$2"
    shift 2
    ;;
  -g | --games)
    GAMES="$2"
    shift 2
    ;;
  -t | --tc)
    TC="$2"
    shift 2
    ;;
  -c | --concurrency)
    CONCURRENCY="$2"
    shift 2
    ;;
  -d | --depth)
    DEPTH="$2"
    shift 2
    ;;
  -f | --openings)
    OPENINGS="$2"
    shift 2
    ;;
  -o | --pgnout | --outdir)
    OUTDIR="$2"
    shift 2
    ;;
  -s | --skip-build)
    SKIP_BUILD=true
    shift
    ;;
  --sprt)
    SPRT="$2"
    shift 2
    ;;
  -h | --help)
    show_help
    ;;
  *)
    echo -e "${RED}Error: Invalid argument: $1${NC}"
    exit 1
    ;;
  esac
done

# Verify required parameters are provided
if [ -z "$ENGINE_A" ] || [ -z "$ENGINE_B" ]; then
  echo -e "${RED}Error: You must specify both engines using -a/--engine-a and -b/--engine-b.${NC}"
  echo -e "Run the following command for help: $0 -h"
  exit 1
fi

# Fallback names based on filenames if names are omitted
if [ -z "$NAME_A" ]; then
  NAME_A=$(basename "$ENGINE_A")
fi
if [ -z "$NAME_B" ]; then
  NAME_B=$(basename "$ENGINE_B")
fi

# Determine output folder
if [ -z "$OUTDIR" ]; then
  DEFAULT_OUTDIR="matches/${NAME_A}-vs-${NAME_B}_$(date +%Y%m%d_%H%M%S)"
  echo -e "${YELLOW}No output folder specified.${NC}"
  echo -n "Enter match name or folder (Press Enter for default: $DEFAULT_OUTDIR): "
  read -r USER_OUTDIR
  if [ -z "$USER_OUTDIR" ]; then
    OUTDIR="$DEFAULT_OUTDIR"
  else
    # If they entered a name without a path separator, place it in matches/
    if [[ "$USER_OUTDIR" != *"/"* ]]; then
      OUTDIR="matches/$USER_OUTDIR"
    else
      OUTDIR="$USER_OUTDIR"
    fi
  fi
fi

# Create target directory
mkdir -p "$OUTDIR"
PGNOUT="$OUTDIR/records.pgn"

print_header

# Verify tool and resource dependencies
check_dependencies() {
  local missing=()

  if [ ! -f "./tools/sylvan-cli" ]; then
    missing+=("Tournament coordinator sylvan-cli (Run ./scripts/setup_tools.sh to download)")
  fi

  if [ ! -f "$OPENINGS" ]; then
    missing+=("Opening database file: $OPENINGS")
  fi

  # Check paths only if it is not an auto-build target
  if [ "$ENGINE_A" != "./target/release/lingine" ] && [ ! -f "$ENGINE_A" ]; then
    missing+=("Engine A path does not exist: $ENGINE_A")
  fi

  if [ "$ENGINE_B" != "./target/release/lingine" ] && [ ! -f "$ENGINE_B" ]; then
    missing+=("Engine B path does not exist: $ENGINE_B")
  fi

  if [ ${#missing[@]} -ne 0 ]; then
    echo -e "\n${RED}${BOLD}ERROR: MISSING REQUIRED DEPENDENCIES OR RESOURCES${NC}"
    for item in "${missing[@]}"; do
      echo -e "  - $item"
    done
    echo -e "${RED}======================================================================${NC}"
    exit 1
  fi
}

check_dependencies

# Compile Lingine automatically if specified and local engine is selected
if [ "$SKIP_BUILD" = false ] && { [ "$ENGINE_A" = "./target/release/lingine" ] || [ "$ENGINE_B" = "./target/release/lingine" ]; }; then
  echo -e "\n${YELLOW}[1/3] Compiling the latest version of Lingine (cargo build --release)...${NC}"
  if ! cargo build --release; then
    echo -e "${RED}Error: Lingine compilation failed! Please check the Rust source code.${NC}"
    exit 1
  fi
  echo -e "${GREEN}=> Compilation successful: ./target/release/lingine${NC}"
else
  echo -e "\n${YELLOW}[1/3] Skipping Lingine compilation (either requested or not using target/release/lingine).${NC}"
fi

# Setup arguments for Engine A
ENGINE_A_ARGS=("-engine" "cmd=$ENGINE_A" "name=$NAME_A" "stderr=${NAME_A}_err.log")
if [ -n "$OPTIONS_A" ]; then
  read -r -a extra_opts_a <<<"$OPTIONS_A"
  ENGINE_A_ARGS+=("${extra_opts_a[@]}")
fi

# Setup arguments for Engine B
ENGINE_B_ARGS=("-engine" "cmd=$ENGINE_B" "name=$NAME_B" "stderr=${NAME_B}_err.log")
if [ -n "$OPTIONS_B" ]; then
  read -r -a extra_opts_b <<<"$OPTIONS_B"
  ENGINE_B_ARGS+=("${extra_opts_b[@]}")
fi

# Remove older PGN to avoid score contamination
if [ -f "$PGNOUT" ]; then
  rm "$PGNOUT"
fi

# Display match configuration details before launching
echo -e "\n${YELLOW}[2/3] Match Configuration:${NC}"
echo -e "  -> Engine A: ${GREEN}$NAME_A${NC} (${BLUE}$ENGINE_A${NC}) $OPTIONS_A"
echo -e "  -> Engine B: ${GREEN}$NAME_B${NC} (${BLUE}$ENGINE_B${NC}) $OPTIONS_B"
echo -e "  -> Total Games: ${BOLD}$GAMES${NC}"
echo -e "  -> Time Control: ${BOLD}$TC${NC}"
echo -e "  -> Opening Depth: ${BOLD}$DEPTH plies${NC} (${BLUE}$OPENINGS${NC})"
echo -e "  -> Parallel Games (Concurrency): ${GREEN}$CONCURRENCY${NC} (System: CPU with $CPU_CORES threads)"
if [ "$CPU_CORES" -ge 16 ]; then
  echo -e "     ${YELLOW}* Tip: Your CPU supports $CPU_CORES threads! You can pass '-c 28' or '-c 30' to maximize performance.${NC}"
fi
echo -e "  -> PGN Output Path: ${BLUE}$PGNOUT${NC}"
if [ -n "$SPRT" ]; then
  echo -e "  -> SPRT Testing: ${BOLD}$SPRT${NC}"
fi

# Build complete CLI execution command array
CLI_ARGS=(
  "./tools/sylvan-cli"
  "${ENGINE_A_ARGS[@]}"
  "${ENGINE_B_ARGS[@]}"
  "-each"
  "proto=uci"
  "tc=$TC"
  "option.Hash=16"
  "-openings"
  "file=$OPENINGS"
  "format=pgn"
  "plies=$DEPTH"
  "-tournament"
  "round-robin"
  "-games"
  "$GAMES"
  "-repeat"
  "-concurrency"
  "$CONCURRENCY"
  "-pgnout"
  "$PGNOUT"
)

if [ -n "$SPRT" ]; then
  CLI_ARGS+=("-sprt" "$SPRT")
fi

echo -e "\n${YELLOW}[3/3] Launching match tournament using sylvan-cli...${NC}"
echo -e "${BLUE}Executing:${NC} ${CLI_ARGS[*]}"
echo -e "----------------------------------------------------------------------"

# Run match tournament via sylvan-cli
set +e
"${CLI_ARGS[@]}"
SYLVAN_STATUS=$?
set -e

echo -e "----------------------------------------------------------------------"
if [ $SYLVAN_STATUS -eq 0 ]; then
  echo -e "${GREEN}${BOLD}Match completed successfully! Detailed results saved to $PGNOUT${NC}"
  echo -e "${YELLOW}Running match analysis and generating summary.md...${NC}"
  python3 scripts/analyze_match.py "$PGNOUT" -m "$OUTDIR/summary.md"
else
  echo -e "${RED}${BOLD}An error occurred while running sylvan-cli. Exit code: $SYLVAN_STATUS${NC}"
fi
echo -e "${CYAN}======================================================================${NC}"
