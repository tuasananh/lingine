#!/bin/bash
set -e

echo "=============================================================="
echo "   CÀI ĐẶT CÔNG CỤ & MÔI TRƯỜNG THỬ NGHIỆM ELO (LINGINE)   "
echo "=============================================================="

# 1. Chuẩn bị thư mục tools
echo -e "\n[1/4] Tạo thư mục tools/..."
mkdir -p tools

# 2. Cài đặt Sylvan-CLI
echo -e "\n[2/4] Đang tải Sylvan-CLI (Công cụ tổ chức giải đấu)..."
curl -L -o tools/sylvan-cli https://github.com/tuasananh/lingine/releases/download/v0.1.0-alpha/sylvan-cli
chmod +x tools/sylvan-cli

# 3. Cài đặt Fairy-Stockfish
echo -e "\n[3/4] Đang tải Fairy-Stockfish (Engine đối thủ tiêu chuẩn)..."
curl -L -o tools/fairy-stockfish_x86-64 https://github.com/fairy-stockfish/Fairy-Stockfish-NNUE/releases/download/xiangqi-ae0082262b68/fairy-stockfish_x86-64
chmod +x tools/fairy-stockfish_x86-64

# 4. Tải Cơ sở dữ liệu Khai cuộc (Opening Database)
echo -e "\n[4/4] Đang tải & giải nén Cơ sở dữ liệu Khai cuộc (Masters UCI PGN)..."
curl -O -L https://github.com/maksimKorzh/wukong-xiangqi/raw/refs/heads/main/xqdb/xqdb/xqdb_masters_40711_UCI_games.pgn.zip
unzip -o xqdb_masters_40711_UCI_games.pgn.zip -d tools/
rm xqdb_masters_40711_UCI_games.pgn.zip

echo -e "\n=============================================================="
echo "   CÀI ĐẶT THÀNH CÔNG!   "
echo "=============================================================="
echo "Bây giờ bạn có thể khởi chạy giải đấu kiểm thử ELO bằng lệnh:"
echo "  ./scripts/run_gauntlet.py"
echo "=============================================================="
