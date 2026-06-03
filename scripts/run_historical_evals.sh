#!/bin/bash
# ======================================================================
#   LINGINE HISTORICAL ENGINE BENCHMARK SUITE
# ======================================================================
# Automates running matches and gauntlets for all historical versions
# of the Lingine chess/xiangqi engine found in the historical/ directory.
# ======================================================================

set -e

# Ensure we are in the repository root
if [ ! -f "Cargo.toml" ] || [ ! -d "historical" ]; then
  echo -e "\033[0;31mError: This script must be run from the repository root directory.\033[0m"
  exit 1
fi

# ANSI Color Codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
# MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Initialize options variables
MODE_NEIGHBOR=false
MODE_BASE=false
MODE_GAUNTLET=false
FILTER_VERSION=""
TC=""
CONCURRENCY=""
FORCE=false
DRY_RUN=false
SHOW_HELP=false

# Discover versions in historical/
HISTORICAL_DIR="historical"
# shellcheck disable=SC2207
FILES=($(find "$HISTORICAL_DIR" -maxdepth 1 -type f -name "lingine-*" | sort))

if [ ${#FILES[@]} -eq 0 ]; then
  echo -e "${RED}Error: No historical engines found in '$HISTORICAL_DIR' matching 'lingine-*'!${NC}"
  exit 1
fi

VERSIONS=()
CLEAN_NAMES=()
for f in "${FILES[@]}"; do
  VERSIONS+=("$f")
  base=$(basename "$f")
  clean="${base#lingine-}"
  CLEAN_NAMES+=("$clean")
done

show_help() {
  echo -e "${CYAN}======================================================================${NC}"
  echo -e "${CYAN}${BOLD}       LINGINE HISTORICAL ENGINE BENCHMARK SUITE                     ${NC}"
  echo -e "${CYAN}======================================================================${NC}"
  echo -e "Automates running chess/xiangqi matches and gauntlets for all historical"
  echo -e "versions found under the 'historical/' directory."
  echo ""
  echo -e "Usage: $0 [options]"
  echo ""
  echo -e "${BOLD}Execution Modes:${NC} (Multiple can be combined. Defaults to running everything)"
  echo -e "  -m, --matches-only      Run both neighbor matches and base matches"
  echo -e "  -g, --gauntlets-only    Run gauntlet tournaments only"
  echo -e "  -n, --neighbor-only     Run neighbor matches only (e.g., 1.1.0 vs 1.2.0)"
  echo -e "  -b, --base-only         Run base matches only (e.g., 1.x.y vs 1.0.0-base)"
  echo -e "  -v, --version <VER>     Only run tasks involving this version"
  echo -e "                          (e.g., --version 1.6.0a-check-extensions)"
  echo ""
  echo -e "${BOLD}Evaluation Tuning Options:${NC}"
  echo -e "  -t, --tc TIMECONTROL    Time control setting (Passed down, e.g., \"10/10+0.1\")"
  echo -e "  -c, --concurrency N     Number of parallel games (Passed down)"
  echo -e "  -f, --force             Overwrite existing results (Disable auto-resume/skip)"
  echo -e "  -d, --dry-run           Show what would be run without executing"
  echo -e "  -h, --help              Show this help message"
  echo ""
  echo -e "${BOLD}Available Discovered Versions in 'historical/':${NC}"
  for name in "${CLEAN_NAMES[@]}"; do
    echo -e "  - ${GREEN}${name}${NC}"
  done
  echo -e "${CYAN}======================================================================${NC}"
  exit 0
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
  case $1 in
  -m | --matches-only)
    MODE_NEIGHBOR=true
    MODE_BASE=true
    shift
    ;;
  -g | --gauntlets-only)
    MODE_GAUNTLET=true
    shift
    ;;
  -n | --neighbor-only)
    MODE_NEIGHBOR=true
    shift
    ;;
  -b | --base-only)
    MODE_BASE=true
    shift
    ;;
  -v | --version)
    FILTER_VERSION="$2"
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
  -f | --force)
    FORCE=true
    shift
    ;;
  -d | --dry-run)
    DRY_RUN=true
    shift
    ;;
  -h | --help)
    SHOW_HELP=true
    shift
    ;;
  *)
    echo -e "${RED}Error: Unknown option $1${NC}"
    echo "Run $0 -h for usage instructions."
    exit 1
    ;;
  esac
done

if [ "$SHOW_HELP" = true ]; then
  show_help
fi

# Validate filter version if specified
if [ -n "$FILTER_VERSION" ]; then
  VERSION_FOUND=false
  for name in "${CLEAN_NAMES[@]}"; do
    if [ "$name" = "$FILTER_VERSION" ]; then
      VERSION_FOUND=true
      break
    fi
  done
  if [ "$VERSION_FOUND" = false ]; then
    echo -e "${RED}Error: Specified version '$FILTER_VERSION' was not found in '$HISTORICAL_DIR'!${NC}"
    echo -e "Available versions are:"
    for name in "${CLEAN_NAMES[@]}"; do
      echo -e "  - $name"
    done
    exit 1
  fi
fi

# Resolve which sections to run
if [ "$MODE_NEIGHBOR" = false ] && [ "$MODE_BASE" = false ] && [ "$MODE_GAUNTLET" = false ]; then
  # Default to running all if no specific mode flags were passed
  RUN_NEIGHBOR=true
  RUN_BASE=true
  RUN_GAUNTLET=true
else
  RUN_NEIGHBOR=$MODE_NEIGHBOR
  RUN_BASE=$MODE_BASE
  RUN_GAUNTLET=$MODE_GAUNTLET
fi

# Build all candidate neighbor matches
NEIGHBOR_MATCHES_A=()
NEIGHBOR_MATCHES_B=()
NEIGHBOR_NAMES_A=()
NEIGHBOR_NAMES_B=()

for ((i = 0; i < ${#VERSIONS[@]} - 1; i++)); do
  NEIGHBOR_MATCHES_A+=("${VERSIONS[i]}")
  NEIGHBOR_MATCHES_B+=("${VERSIONS[i + 1]}")
  NEIGHBOR_NAMES_A+=("${CLEAN_NAMES[i]}")
  NEIGHBOR_NAMES_B+=("${CLEAN_NAMES[i + 1]}")
done

# Build all candidate base matches (all vs base 1.0.0)
BASE_MATCHES_A=()
BASE_MATCHES_B=()
BASE_NAMES_A=()
BASE_NAMES_B=()

BASE_VER="${VERSIONS[0]}"
BASE_NAME="${CLEAN_NAMES[0]}"

for ((i = 1; i < ${#VERSIONS[@]}; i++)); do
  BASE_MATCHES_A+=("${VERSIONS[i]}")
  BASE_MATCHES_B+=("$BASE_VER")
  BASE_NAMES_A+=("${CLEAN_NAMES[i]}")
  BASE_NAMES_B+=("$BASE_NAME")
done

# Build all candidate gauntlets
GAUNTLET_ENGINES=()
GAUNTLET_NAMES=()
for ((i = 0; i < ${#VERSIONS[@]}; i++)); do
  GAUNTLET_ENGINES+=("${VERSIONS[i]}")
  GAUNTLET_NAMES+=("${CLEAN_NAMES[i]}")
done

# Filter neighbor matches
ACTIVE_NEIGHBOR_A=()
ACTIVE_NEIGHBOR_B=()
ACTIVE_NEIGHBOR_NAME_A=()
ACTIVE_NEIGHBOR_NAME_B=()
if [ "$RUN_NEIGHBOR" = true ]; then
  for ((i = 0; i < ${#NEIGHBOR_MATCHES_A[@]}; i++)); do
    na="${NEIGHBOR_NAMES_A[i]}"
    nb="${NEIGHBOR_NAMES_B[i]}"
    if [ -z "$FILTER_VERSION" ] || [ "$na" = "$FILTER_VERSION" ] || [ "$nb" = "$FILTER_VERSION" ]; then
      ACTIVE_NEIGHBOR_A+=("${NEIGHBOR_MATCHES_A[i]}")
      ACTIVE_NEIGHBOR_B+=("${NEIGHBOR_MATCHES_B[i]}")
      ACTIVE_NEIGHBOR_NAME_A+=("$na")
      ACTIVE_NEIGHBOR_NAME_B+=("$nb")
    fi
  done
fi

# Filter base matches
ACTIVE_BASE_A=()
ACTIVE_BASE_B=()
ACTIVE_BASE_NAME_A=()
ACTIVE_BASE_NAME_B=()
if [ "$RUN_BASE" = true ]; then
  for ((i = 0; i < ${#BASE_MATCHES_A[@]}; i++)); do
    na="${BASE_NAMES_A[i]}"
    nb="${BASE_NAMES_B[i]}"
    if [ -z "$FILTER_VERSION" ] || [ "$na" = "$FILTER_VERSION" ] || [ "$nb" = "$FILTER_VERSION" ]; then
      ACTIVE_BASE_A+=("${BASE_MATCHES_A[i]}")
      ACTIVE_BASE_B+=("${BASE_MATCHES_B[i]}")
      ACTIVE_BASE_NAME_A+=("$na")
      ACTIVE_BASE_NAME_B+=("$nb")
    fi
  done
fi

# Filter gauntlets
ACTIVE_GAUNTLET_ENGINES=()
ACTIVE_GAUNTLET_NAMES=()
if [ "$RUN_GAUNTLET" = true ]; then
  for ((i = 0; i < ${#GAUNTLET_ENGINES[@]}; i++)); do
    n="${GAUNTLET_NAMES[i]}"
    if [ -z "$FILTER_VERSION" ] || [ "$n" = "$FILTER_VERSION" ]; then
      ACTIVE_GAUNTLET_ENGINES+=("${GAUNTLET_ENGINES[i]}")
      ACTIVE_GAUNTLET_NAMES+=("$n")
    fi
  done
fi

# Print banner showing plan
echo -e "${CYAN}======================================================================${NC}"
echo -e "${CYAN}${BOLD}       LINGINE HISTORICAL RUNNER ACTIVE PLAN                         ${NC}"
echo -e "${CYAN}======================================================================${NC}"
if [ -n "$FILTER_VERSION" ]; then
  echo -e "Filtering to version: ${GREEN}${FILTER_VERSION}${NC}"
fi
echo -e "Time Control: ${BOLD}${TC:-"10/10+0.1 (default)"}${NC}"
echo -e "Concurrency:  ${BOLD}${CONCURRENCY:-"auto (default)"}${NC}"
echo -e "Force Re-run: ${BOLD}${FORCE}${NC}"
echo -e "Dry Run:      ${BOLD}${DRY_RUN}${NC}"
echo -e "----------------------------------------------------------------------"
echo -e "Matches to Run:"
echo -e "  - Neighbor Matches: ${GREEN}${#ACTIVE_NEIGHBOR_A[@]}${NC}"
echo -e "  - Base Matches:     ${GREEN}${#ACTIVE_BASE_A[@]}${NC}"
echo -e "Gauntlets to Run:     ${GREEN}${#ACTIVE_GAUNTLET_NAMES[@]}${NC}"
echo -e "${CYAN}======================================================================${NC}"
echo ""

# Stats tracking
SUCCESS_COUNT=0
FAILED_COUNT=0
SKIPPED_COUNT=0
RUN_COUNT=0

SUCCESS_ITEMS=()
FAILED_ITEMS=()
SKIPPED_ITEMS=()

TC_ARG=""
if [ -n "$TC" ]; then
  TC_ARG="-t $TC"
fi

CONC_ARG=""
if [ -n "$CONCURRENCY" ]; then
  CONC_ARG="-c $CONCURRENCY"
fi

run_match_task() {
  local eng_a="$1"
  local name_a="$2"
  local eng_b="$3"
  local name_b="$4"
  local out_dir="$5"
  local games=100

  local pgn_file="$out_dir/records.pgn"
  local sum_file="$out_dir/summary.md"

  # Check if summary.md and records.pgn already exist (and FORCE is false)
  if [ "$FORCE" = false ] && [ -f "$sum_file" ] && [ -f "$pgn_file" ]; then
    echo -e "${BLUE}[SKIP]${NC} Match ${GREEN}$name_a${NC} vs ${GREEN}$name_b${NC} already exists in ${CYAN}$out_dir${NC}"
    SKIPPED_COUNT=$((SKIPPED_COUNT + 1))
    SKIPPED_ITEMS+=("Match: $name_a vs $name_b")
    return 0
  fi

  echo -e "${YELLOW}[RUNNING]${NC} Match ${GREEN}$name_a${NC} vs ${GREEN}$name_b${NC} (${BOLD}$games${NC} games)"
  echo -e "          Output: ${CYAN}$out_dir${NC}"

  if [ "$DRY_RUN" = true ]; then
    echo -e "          [DRY-RUN] Would run: ./scripts/run_match.sh -a \"$eng_a\" --name-a \"$name_a\" -b \"$eng_b\" --name-b \"$name_b\" -g \"$games\" -s -o \"$out_dir\" ${TC_ARG} ${CONC_ARG}"
    RUN_COUNT=$((RUN_COUNT + 1))
    return 0
  fi

  # Build command arguments
  local cmd=("./scripts/run_match.sh" "-a" "$eng_a" "--name-a" "$name_a" "-b" "$eng_b" "--name-b" "$name_b" "-g" "$games" "-s" "-o" "$out_dir")
  if [ -n "$TC" ]; then
    cmd+=("-t" "$TC")
  fi
  if [ -n "$CONCURRENCY" ]; then
    cmd+=("-c" "$CONCURRENCY")
  fi

  # Run the match
  set +e
  "${cmd[@]}"
  local status=$?
  set -e

  if [ $status -eq 0 ]; then
    echo -e "${GREEN}[SUCCESS]${NC} Match ${GREEN}$name_a${NC} vs ${GREEN}$name_b${NC} finished successfully."
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    SUCCESS_ITEMS+=("Match: $name_a vs $name_b")
  else
    echo -e "${RED}[FAILED]${NC} Match ${GREEN}$name_a${NC} vs ${GREEN}$name_b${NC} failed with status $status."
    FAILED_COUNT=$((FAILED_COUNT + 1))
    FAILED_ITEMS+=("Match: $name_a vs $name_b (Exit code: $status)")
  fi
}

run_gauntlet_task() {
  local eng="$1"
  local name="$2"
  local out_dir="$3"
  local games=50 # 50 games per ELO level (6 ELO levels = 300 games)

  local pgn_file="$out_dir/records.pgn"
  local sum_file="$out_dir/summary.md"

  # Check if summary.md and records.pgn already exist (and FORCE is false)
  if [ "$FORCE" = false ] && [ -f "$sum_file" ] && [ -f "$pgn_file" ]; then
    echo -e "${BLUE}[SKIP]${NC} Gauntlet for ${GREEN}$name${NC} already exists in ${CYAN}$out_dir${NC}"
    SKIPPED_COUNT=$((SKIPPED_COUNT + 1))
    SKIPPED_ITEMS+=("Gauntlet: $name")
    return 0
  fi

  echo -e "${YELLOW}[RUNNING]${NC} Gauntlet for ${GREEN}$name${NC} (${BOLD}300${NC} games: 6 opponents x $games games)"
  echo -e "          Output: ${CYAN}$out_dir${NC}"

  if [ "$DRY_RUN" = true ]; then
    echo -e "          [DRY-RUN] Would run: python3 scripts/run_gauntlet.py -a \"$eng\" --name \"$name\" -g \"$games\" -s -o \"$out_dir\" ${TC_ARG} ${CONC_ARG}"
    RUN_COUNT=$((RUN_COUNT + 1))
    return 0
  fi

  # Build command arguments
  local cmd=("python3" "scripts/run_gauntlet.py" "-a" "$eng" "--name" "$name" "-g" "$games" "-s" "-o" "$out_dir")
  if [ -n "$TC" ]; then
    cmd+=("-t" "$TC")
  fi
  if [ -n "$CONCURRENCY" ]; then
    cmd+=("-c" "$CONCURRENCY")
  fi

  # Run the gauntlet
  set +e
  "${cmd[@]}"
  local status=$?
  set -e

  if [ $status -eq 0 ]; then
    echo -e "${GREEN}[SUCCESS]${NC} Gauntlet for ${GREEN}$name${NC} finished successfully."
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    SUCCESS_ITEMS+=("Gauntlet: $name")
  else
    echo -e "${RED}[FAILED]${NC} Gauntlet for ${GREEN}$name${NC} failed with status $status."
    FAILED_COUNT=$((FAILED_COUNT + 1))
    FAILED_ITEMS+=("Gauntlet: $name (Exit code: $status)")
  fi
}

START_TIME=$(date +%s)

# ======================================================================
# 1. RUN NEIGHBOR MATCHES
# ======================================================================
if [ ${#ACTIVE_NEIGHBOR_A[@]} -gt 0 ]; then
  echo -e "${CYAN}======================================================================${NC}"
  echo -e "${CYAN}${BOLD}       SECTION 1: NEIGHBOR MATCHES (100 Games Each)                  ${NC}"
  echo -e "${CYAN}======================================================================${NC}"
  for ((i = 0; i < ${#ACTIVE_NEIGHBOR_A[@]}; i++)); do
    eng_a="${ACTIVE_NEIGHBOR_A[i]}"
    name_a="${ACTIVE_NEIGHBOR_NAME_A[i]}"
    eng_b="${ACTIVE_NEIGHBOR_B[i]}"
    name_b="${ACTIVE_NEIGHBOR_NAME_B[i]}"
    out_dir="matches/historical/neighbor_matches/${name_a}-vs-${name_b}"

    echo -e "\n${BOLD}Task $((i + 1))/${#ACTIVE_NEIGHBOR_A[@]}:${NC}"
    run_match_task "$eng_a" "$name_a" "$eng_b" "$name_b" "$out_dir"
  done
  echo ""
fi

# ======================================================================
# 2. RUN BASE MATCHES
# ======================================================================
if [ ${#ACTIVE_BASE_A[@]} -gt 0 ]; then
  echo -e "${CYAN}======================================================================${NC}"
  echo -e "${CYAN}${BOLD}       SECTION 2: BASE MATCHES VS 1.0.0-BASE (100 Games Each)         ${NC}"
  echo -e "${CYAN}======================================================================${NC}"
  for ((i = 0; i < ${#ACTIVE_BASE_A[@]}; i++)); do
    eng_a="${ACTIVE_BASE_A[i]}"
    name_a="${ACTIVE_BASE_NAME_A[i]}"
    eng_b="${ACTIVE_BASE_B[i]}"
    name_b="${ACTIVE_BASE_NAME_B[i]}"
    out_dir="matches/historical/base_matches/${name_a}-vs-${name_b}"

    echo -e "\n${BOLD}Task $((i + 1))/${#ACTIVE_BASE_A[@]}:${NC}"
    run_match_task "$eng_a" "$name_a" "$eng_b" "$name_b" "$out_dir"
  done
  echo ""
fi

# ======================================================================
# 3. RUN GAUNTLETS
# ======================================================================
if [ ${#ACTIVE_GAUNTLET_NAMES[@]} -gt 0 ]; then
  echo -e "${CYAN}======================================================================${NC}"
  echo -e "${CYAN}${BOLD}       SECTION 3: GAUNTLET TOURNAMENTS (300 Games Each)               ${NC}"
  echo -e "${CYAN}======================================================================${NC}"
  for ((i = 0; i < ${#ACTIVE_GAUNTLET_NAMES[@]}; i++)); do
    eng="${ACTIVE_GAUNTLET_ENGINES[i]}"
    name="${ACTIVE_GAUNTLET_NAMES[i]}"
    out_dir="matches/historical/gauntlets/${name}"

    echo -e "\n${BOLD}Task $((i + 1))/${#ACTIVE_GAUNTLET_NAMES[@]}:${NC}"
    run_gauntlet_task "$eng" "$name" "$out_dir"
  done
  echo ""
fi

END_TIME=$(date +%s)
ELAPSED_SEC=$((END_TIME - START_TIME))
ELAPSED_STR=""
if [ $ELAPSED_SEC -ge 3600 ]; then
  ELAPSED_STR="$((ELAPSED_SEC / 3600))h $(((ELAPSED_SEC % 3600) / 60))m $((ELAPSED_SEC % 60))s"
elif [ $ELAPSED_SEC -ge 60 ]; then
  ELAPSED_STR="$((ELAPSED_SEC / 60))m $((ELAPSED_SEC % 60))s"
else
  ELAPSED_STR="${ELAPSED_SEC}s"
fi

# ======================================================================
# FINAL SUMMARY REPORT
# ======================================================================
echo -e "${CYAN}======================================================================${NC}"
echo -e "${CYAN}${BOLD}       EVALUATION RUN COMPLETE                                        ${NC}"
echo -e "${CYAN}======================================================================${NC}"
echo -e "Total Execution Time: ${BOLD}${ELAPSED_STR}${NC}"
echo -e "Results Summary:"
if [ "$DRY_RUN" = true ]; then
  echo -e "  - Dry-Run Tasks:   ${GREEN}${RUN_COUNT}${NC}"
else
  echo -e "  - Succeeded Tasks: ${GREEN}${SUCCESS_COUNT}${NC}"
  echo -e "  - Failed Tasks:    ${RED}${FAILED_COUNT}${NC}"
fi
echo -e "  - Skipped Tasks:   ${BLUE}${SKIPPED_COUNT}${NC}"
echo -e "----------------------------------------------------------------------"

if [ ${#SUCCESS_ITEMS[@]} -gt 0 ]; then
  echo -e "${GREEN}${BOLD}SUCCEEDED TASK DETAILS:${NC}"
  for item in "${SUCCESS_ITEMS[@]}"; do
    echo -e "  [✓] $item"
  done
fi

if [ ${#FAILED_ITEMS[@]} -gt 0 ]; then
  echo -e "\n${RED}${BOLD}FAILED TASK DETAILS:${NC}"
  for item in "${FAILED_ITEMS[@]}"; do
    echo -e "  [✗] $item"
  done
fi

if [ ${#SKIPPED_ITEMS[@]} -gt 0 ]; then
  echo -e "\n${BLUE}${BOLD}SKIPPED/RESUMED TASK DETAILS:${NC}"
  for item in "${SKIPPED_ITEMS[@]}"; do
    echo -e "  [-] $item"
  done
fi
echo -e "${CYAN}======================================================================${NC}"
