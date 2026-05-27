#!/usr/bin/env python3
import os
import sys
import subprocess
import re
import math

def print_header(title):
    print("=" * 65)
    print(f"   {title.upper()}   ")
    print("=" * 65)

def check_dependencies():
    """Kiểm tra các file công cụ và dữ liệu khai cuộc bắt buộc."""
    required_files = {
        "tools/sylvan-cli": "Công cụ tổ chức giải đấu sylvan-cli",
        "tools/fairy-stockfish_x86-64": "Engine đối thủ Fairy-Stockfish",
        "tools/xqdb_masters_40711_UCI_games.pgn": "Cơ sở dữ liệu khai cuộc Masters PGN"
    }
    
    missing = []
    for filepath, description in required_files.items():
        if not os.path.exists(filepath):
            missing.append(f"- {description} ({filepath})")
            
    if missing:
        print_header("LỖI: THIẾU CÔNG CỤ HOẶC DỮ LIỆU")
        print("Vui lòng chạy script cài đặt trước khi chạy giải đấu:")
        print("  ./scripts/setup_tools.sh\n")
        print("Các thành phần còn thiếu hiện tại:")
        for item in missing:
            print(item)
        print("=" * 65)
        sys.exit(1)

def build_engine():
    """Biên dịch phiên bản phát hành mới nhất của Lingine."""
    print("\n[1/3] Đang biên dịch Lingine (cargo build --release)...")
    try:
        # Chạy cargo build --release và ẩn stdout nếu thành công, chỉ hiện khi lỗi
        result = subprocess.run(
            ["cargo", "build", "--release"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        if result.returncode != 0:
            print_header("LỖI BIÊN DỊCH DỰ ÁN RUST")
            print(result.stderr)
            sys.exit(1)
        print("=> Biên dịch thành công: ./target/release/lingine")
    except FileNotFoundError:
        print_header("LỖI: KHÔNG TÌM THẤY RUST/CARGO")
        print("Vui lòng đảm bảo bạn đã cài đặt Rust (cargo).")
        sys.exit(1)

def run_tournament():
    """Thực thi giải đấu gauntlet thông qua sylvan-cli."""
    # Phát hiện số core CPU để tối ưu hóa hiệu năng chạy giải đấu
    cores = os.cpu_count() or 4
    concurrency = max(1, cores - 2)
    
    print("\n[2/3] Khởi chạy giải đấu Gauntlet (100 ván đấu)...")
    print(f"  -> Hệ thống của bạn có: {cores} CPU Cores")
    print(f"  -> Tự động tối ưu hóa số trận đấu chạy song song (concurrency) = {concurrency}")
    
    # Xóa file gauntlet.pgn cũ để tránh cộng dồn kết quả cũ
    if os.path.exists("gauntlet.pgn"):
        os.remove("gauntlet.pgn")

    cmd = [
        "./tools/sylvan-cli",
        "-engine", "cmd=./target/release/lingine", "name=Lingine",
        "-engine", "cmd=./tools/fairy-stockfish_x86-64", "name=FS-1000", "option.UCI_LimitStrength=true", "option.UCI_Elo=1000",
        "-engine", "cmd=./tools/fairy-stockfish_x86-64", "name=FS-1200", "option.UCI_LimitStrength=true", "option.UCI_Elo=1200",
        "-engine", "cmd=./tools/fairy-stockfish_x86-64", "name=FS-1400", "option.UCI_LimitStrength=true", "option.UCI_Elo=1400",
        "-engine", "cmd=./tools/fairy-stockfish_x86-64", "name=FS-1600", "option.UCI_LimitStrength=true", "option.UCI_Elo=1600",
        "-engine", "cmd=./tools/fairy-stockfish_x86-64", "name=FS-1800", "option.UCI_LimitStrength=true", "option.UCI_Elo=1800",
        "-each", "proto=uci", "tc=10/10+0.1", "option.Hash=16",
        "-openings", "file=tools/xqdb_masters_40711_UCI_games.pgn", "format=pgn", "order=random", "-pgndepth", "12",
        "-tournament", "gauntlet", "-games", "20", "-repeat", "-concurrency", str(concurrency),
        "-pgnout", "gauntlet.pgn", "-variant", "xiangqi"
    ]
    
    try:
        # Chạy sylvan-cli và chuyển hướng đầu ra trực tiếp ra màn hình
        subprocess.run(cmd, check=True)
        print("=> Giải đấu kết thúc hoàn tất!")
    except subprocess.CalledProcessError as e:
        print_header("LỖI THỰC THI Sylvan-CLI")
        print(f"Sylvan-CLI gặp sự cố khi chạy giải đấu. Mã lỗi: {e.returncode}")
        sys.exit(1)

def parse_pgn(pgn_path):
    if not os.path.exists(pgn_path):
        return []
    with open(pgn_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    games_raw = content.split('[Event ')
    results = []

    for game in games_raw:
        if not game.strip():
            continue
        white_match = re.search(r'\[White\s+"([^"]+)"\]', game)
        black_match = re.search(r'\[Black\s+"([^"]+)"\]', game)
        result_match = re.search(r'\[Result\s+"([^"]+)"\]', game)

        if white_match and black_match and result_match:
            results.append({
                'white': white_match.group(1),
                'black': black_match.group(1),
                'result': result_match.group(1)
            })
    return results

def calculate_and_display_elo():
    """Đọc PGN kết quả và phân tích chỉ số ELO."""
    print("\n[3/3] Đang phân tích kết quả và tính toán ELO...")
    games = parse_pgn("gauntlet.pgn")
    
    if not games:
        print("Lỗi: Không tìm thấy dữ liệu giải đấu trong gauntlet.pgn để tính toán.")
        sys.exit(1)

    stats = {}
    opponent_elos = {
        'FS-1000': 1000,
        'FS-1200': 1200,
        'FS-1400': 1400,
        'FS-1600': 1600,
        'FS-1800': 1800
    }

    for game in games:
        w, b, res = game['white'], game['black'], game['result']
        if w == 'Lingine':
            opponent = b
            lingine_color = 'white'
        elif b == 'Lingine':
            opponent = w
            lingine_color = 'black'
        else:
            continue
            
        if opponent not in stats:
            stats[opponent] = {'wins': 0, 'draws': 0, 'losses': 0, 'games': 0}
            
        stats[opponent]['games'] += 1
        
        if res == '1-0':
            if lingine_color == 'white':
                stats[opponent]['wins'] += 1
            else:
                stats[opponent]['losses'] += 1
        elif res == '0-1':
            if lingine_color == 'black':
                stats[opponent]['wins'] += 1
            else:
                stats[opponent]['losses'] += 1
        elif res in ['1/2-1/2', '0.5-0.5']:
            stats[opponent]['draws'] += 1
        else:
            stats[opponent]['draws'] += 1

    print("\n" + "=" * 70)
    print("           BẢNG PHÂN TÍCH HIỆU SUẤT ELO - LINGINE             ")
    print("=" * 70)
    print(f"{'Đối thủ':<15}{'Trận':<8}{'Thắng':<8}{'Hòa':<8}{'Thua':<8}{'Tỉ lệ điểm':<12}{'Ước lượng ELO':<15}")
    print("-" * 70)

    elo_estimates = []
    sorted_opponents = sorted(stats.keys(), key=lambda x: opponent_elos.get(x, 1200))

    for opp in sorted_opponents:
        s = stats[opp]
        opp_base_elo = opponent_elos.get(opp)
        if opp_base_elo is None:
            continue
            
        wins, draws, losses, n = s['wins'], s['draws'], s['losses'], s['games']
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
        
        score_str = f"{score_pct*100:.1f}%"
        elo_str = f"{int(round(estimated_elo))} ELO"
        
        print(f"{opp:<15}{n:<8}{wins:<8}{draws:<8}{losses:<8}{score_str:<12}{elo_str:<15}")

    print("-" * 70)
    if elo_estimates:
        final_elo = sum(elo_estimates) / len(elo_estimates)
        print(f"\n=> ĐIỂM ELO TRUNG BÌNH ƯỚC TÍNH CỦA LINGINE: {int(round(final_elo))} ELO")
    else:
        print("\nKhông đủ dữ liệu ván đấu để ước tính ELO.")
    print("=" * 70 + "\n")

def main():
    print_header("LINGINE GAUNTLET TOURNAMENT & ELO EVALUATOR")
    check_dependencies()
    build_engine()
    run_tournament()
    calculate_and_display_elo()

if __name__ == '__main__':
    main()
