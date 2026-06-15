#!/usr/bin/env python3
import os
import urllib.request
import tarfile
import zipfile

import utils


def download_file(url, dest):
    print(f"Downloading from {url}...")
    urllib.request.urlretrieve(url, dest)


def main():
    utils.print_header("LINGINE TOOLCHAIN & TEST ENVIRONMENT INSTALLER")

    print("\n[1/4] Creating tools/ directory...")
    os.makedirs("tools", exist_ok=True)

    print("\n[2/4] Downloading Sylvan-CLI (Tournament Coordinator)...")
    sylvan_tar = "tools/sylvan.tar.gz"
    download_file(
        "https://github.com/tuasananh/Sylvan/releases/latest/download/sylvan.tar.gz",
        sylvan_tar,
    )
    print("Extracting Sylvan-CLI...")
    with tarfile.open(sylvan_tar, "r:gz") as tar:
        tar.extractall(path="tools/")
    os.remove(sylvan_tar)

    print("\n[3/4] Downloading Fairy-Stockfish (Baseline Opponent Engine)...")
    fs_bin = "tools/fairy-stockfish_x86-64"
    download_file(
        "https://github.com/fairy-stockfish/Fairy-Stockfish-NNUE/releases/download/xiangqi-ae0082262b68/fairy-stockfish_x86-64",
        fs_bin,
    )
    os.chmod(fs_bin, 0o755)

    print("\n[4/4] Downloading & extracting Opening Database (Masters UCI PGN)...")
    db_zip = "tools/xqdb_masters_40711_UCI_games.pgn.zip"
    download_file(
        "https://github.com/maksimKorzh/wukong-xiangqi/raw/refs/heads/main/xqdb/xqdb/xqdb_masters_40711_UCI_games.pgn.zip",
        db_zip,
    )
    print("Extracting Opening Database...")
    with zipfile.ZipFile(db_zip, "r") as zip_ref:
        zip_ref.extractall("tools/")
    os.remove(db_zip)

    utils.print_header("INSTALLATION COMPLETED SUCCESSFULLY!")
    print("You can now launch the ELO gauntlet tournament using:")
    print("  sys.executable scripts/run_gauntlet.py")
    print("=" * 70)


if __name__ == "__main__":
    main()
