#!/usr/bin/env python3
import os
import sys
import subprocess
import re
import math
import argparse


def print_header(title):
    print("=" * 70)
    print(f"   {title.upper()}   ")
    print("=" * 70)


def check_dependencies():
    """Kiểm tra các file công cụ và dữ liệu khai cuộc bắt buộc."""
    required_files = {
        "tools/sylvan-cli": "Công cụ tổ chức giải đấu sylvan-cli",
        "tools/fairy-stockfish_x86-64": "Engine đối thủ Fairy-Stockfish",
        "tools/xqdb_masters_40711_UCI_games.pgn": "Cơ sở dữ liệu khai cuộc Masters PGN",
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
        print("=" * 70)
        sys.exit(1)


def build_engine():
    """Biên dịch phiên bản phát hành mới nhất của Lingine."""
    print("\n[1/3] Đang biên dịch Lingine (cargo build --release)...")
    try:
        result = subprocess.run(
            ["cargo", "build", "--release"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
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


def run_tournament(args):
    """Thực thi giải đấu gauntlet thông qua sylvan-cli với các tùy chọn từ cấu hình."""
    # Phát hiện số core CPU nếu người dùng không chỉ định concurrency
    if args.concurrency is None:
        cores = os.cpu_count() or 4
        # Tự động tối ưu ở mức 1 engine mỗi core (2 engines mỗi game, do đó cores // 2)
        # Giới hạn tối đa 20 để tránh overhead quản lý tiến trình quá lớn của hệ điều hành
        concurrency = min(20, max(1, cores // 2))
        concurrency_msg = f"{concurrency} (Tự động tối ưu hiệu năng từ {cores} cores)"
    else:
        concurrency = args.concurrency
        concurrency_msg = f"{concurrency} (Người dùng cấu hình)"

    # Phân tích danh sách ELO đối thủ
    try:
        elo_list = [int(x.strip()) for x in args.elos.split(",")]
    except ValueError:
        print("Lỗi: Danh sách ELO đối thủ không hợp lệ. Ví dụ đúng: 1000,1200,1400")
        sys.exit(1)

    total_games = len(elo_list) * args.games

    print("\n[2/3] Khởi chạy giải đấu Gauntlet...")
    print(f"  -> Số trận đấu chạy song song (concurrency): {concurrency_msg}")
    print(f"  -> Các mốc ELO đối thủ: {', '.join(map(str, elo_list))}")
    print(
        f"  -> Số ván đấu mỗi cặp đối đầu: {args.games} ván (Tổng cộng: {total_games} ván)"
    )
    print(f"  -> Thiết lập kiểm soát thời gian (Time Control): {args.tc}")
    print(f"  -> Độ sâu khai cuộc: {args.depth} plies")
    print(f"  -> Tệp tin PGN kết quả: {args.pgnout}")

    # Xóa file gauntlet PGN cũ để tránh cộng dồn kết quả cũ
    if os.path.exists(args.pgnout):
        os.remove(args.pgnout)

    cmd = [
        "./tools/sylvan-cli",
        "-engine",
        "cmd=./target/release/lingine",
        "name=Lingine",
        "stderr=lingine_err.log",
    ]

    # Thêm động thực thể bot Fairy-Stockfish cho mỗi mốc ELO cấu hình
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
        print("=> Giải đấu kết thúc hoàn tất!")
    except subprocess.CalledProcessError as e:
        print_header("LỖI THỰC THI Sylvan-CLI")
        print(f"Sylvan-CLI gặp sự cố khi chạy giải đấu. Mã lỗi: {e.returncode}")
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
    """Đọc PGN kết quả và phân tích chỉ số ELO."""
    print("\n[3/3] Đang phân tích kết quả và tính toán ELO...")
    games = parse_pgn(args.pgnout)

    if not games:
        print(
            f"Lỗi: Không tìm thấy dữ liệu giải đấu trong '{args.pgnout}' để tính toán."
        )
        sys.exit(1)

    # Phân tích danh sách ELO đối thủ
    opponent_elos = {}
    for elo_str in args.elos.split(","):
        elo_val = int(elo_str.strip())
        opponent_elos[f"FS-{elo_val}"] = elo_val

    stats = {}
    for game in games:
        w, b, res = game["white"], game["black"], game["result"]
        if w == "Lingine":
            opponent = b
            lingine_color = "white"
        elif b == "Lingine":
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
    print("           BẢNG PHÂN TÍCH HIỆU SUẤT ELO - LINGINE             ")
    print("=" * 70)
    print(
        f"{'Đối thủ':<15}{'Trận':<8}{'Thắng':<8}{'Hòa':<8}{'Thua':<8}{'Tỉ lệ điểm':<12}{'Ước lượng ELO':<15}"
    )
    print("-" * 70)

    elo_estimates = []
    # Chỉ sắp xếp và hiển thị các đối thủ có nằm trong cấu hình ELO
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
            f"\n=> ĐIỂM ELO TRUNG BÌNH ƯỚC TÍNH CỦA LINGINE: {int(round(final_elo))} ELO"
        )
    else:
        print("\nKhông đủ dữ liệu ván đấu phù hợp để ước tính ELO.")
    print("=" * 70 + "\n")


def main():
    parser = argparse.ArgumentParser(
        description="Chương trình tổ chức giải đấu Gauntlet & Ước lượng ELO cho Lingine."
    )
    parser.add_argument(
        "-c",
        "--cores",
        "--concurrency",
        type=int,
        default=None,
        dest="concurrency",
        help="Số trận đấu chạy song song đồng thời (số nhân CPU sử dụng, mặc định: tự động tối ưu)",
    )
    parser.add_argument(
        "-g",
        "--games",
        type=int,
        default=20,
        help="Số ván đấu chơi với mỗi mốc đối thủ ELO (mặc định: 20)",
    )
    parser.add_argument(
        "-t",
        "--tc",
        type=str,
        default="10/10+0.1",
        help="Thiết lập kiểm soát thời gian (Time Control) (mặc định: 10/10+0.1)",
    )
    parser.add_argument(
        "-d",
        "--depth",
        type=int,
        default=12,
        help="Độ sâu nước đi khai cuộc bắt buộc (plies) (mặc định: 12)",
    )
    parser.add_argument(
        "-f",
        "--openings-file",
        type=str,
        default="tools/xqdb_masters_40711_UCI_games.pgn",
        help="Đường dẫn đến tệp tin PGN/EPD khai cuộc (mặc định: tools/xqdb_masters_40711_UCI_games.pgn)",
    )
    parser.add_argument(
        "-o",
        "--pgnout",
        type=str,
        default="gauntlet.pgn",
        help="Đường dẫn lưu tệp PGN kết quả ván đấu (mặc định: gauntlet.pgn)",
    )
    parser.add_argument(
        "-e",
        "--elos",
        type=str,
        default="1000,1200,1400,1600,1800",
        help="Danh sách ELO đối thủ, phân tách bằng dấu phẩy (mặc định: 1000,1200,1400,1600,1800)",
    )
    parser.add_argument(
        "-s",
        "--skip-build",
        action="store_true",
        help="Bỏ qua bước tự động biên dịch 'cargo build --release'",
    )

    args = parser.parse_args()

    print_header("LINGINE GAUNTLET TOURNAMENT & ELO EVALUATOR")
    check_dependencies()

    if not args.skip_build:
        build_engine()
    else:
        print("\n[1/3] Đã bỏ qua bước biên dịch theo yêu cầu (--skip-build).")

    run_tournament(args)
    calculate_and_display_elo(args)


if __name__ == "__main__":
    main()
