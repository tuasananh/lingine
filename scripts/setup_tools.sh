#!/bin/bash
set -e

echo "=============================================================="
echo "   LINGINE TOOLCHAIN & TEST ENVIRONMENT INSTALLER   "
echo "=============================================================="

# 1. Prepare tools directory
echo -e "\n[1/4] Creating tools/ directory..."
mkdir -p tools

# 2. Install Sylvan-CLI
echo -e "\n[2/4] Downloading Sylvan-CLI (Tournament Coordinator)..."
curl -L -o tools/sylvan.tar.gz https://github.com/tuasananh/Sylvan/releases/download/v1.1.0/sylvan.tar.gz
tar -xf tools/sylvan.tar.gz -C tools/
rm tools/sylvan.tar.gz

# 3. Install Fairy-Stockfish
echo -e "\n[3/4] Downloading Fairy-Stockfish (Baseline Opponent Engine)..."
curl -L -o tools/fairy-stockfish_x86-64 https://github.com/fairy-stockfish/Fairy-Stockfish-NNUE/releases/download/xiangqi-ae0082262b68/fairy-stockfish_x86-64
chmod +x tools/fairy-stockfish_x86-64

# 4. Download Opening Database
echo -e "\n[4/4] Downloading & extracting Opening Database (Masters UCI PGN)..."
curl -O -L https://github.com/maksimKorzh/wukong-xiangqi/raw/refs/heads/main/xqdb/xqdb/xqdb_masters_40711_UCI_games.pgn.zip
unzip -o xqdb_masters_40711_UCI_games.pgn.zip -d tools/
rm xqdb_masters_40711_UCI_games.pgn.zip

echo -e "\n=============================================================="
echo "   INSTALLATION COMPLETED SUCCESSFULLY!   "
echo "=============================================================="
echo "You can now launch the ELO gauntlet tournament using:"
echo "  ./scripts/run_gauntlet.py"
echo "=============================================================="
