#!/usr/bin/env python3
import os
import sys
import re
import glob


def extract_elo_diff(md_content, version):
    if not md_content:
        return "N/A"

    # Extract headers
    header_match = re.search(
        r"\|\s*Metric\s*\|\s*([^|]+)\s*\|\s*([^|]+)\s*\|", md_content, re.IGNORECASE
    )
    if not header_match:
        return "N/A"

    eng_first = header_match.group(1).strip()
    eng_second = header_match.group(2).strip()

    # Extract Elo Diff row
    m = re.search(
        r"\|\s*\*\*Elo Diff\*\*\s*\|\s*\*\*([+-]?\d+(?:\.\d+)?)\*\*\s*\|\s*\*\*([+-]?\d+(?:\.\d+)?)\*\*\s*\|",
        md_content,
        re.IGNORECASE,
    )
    if not m:
        # Try without double asterisks in diff values
        m = re.search(
            r"\|\s*\*\*Elo Diff\*\*\s*\|\s*\*?([+-]?\d+(?:\.\d+)?)\*?\s*\|\s*\*?([+-]?\d+(?:\.\d+)?)\*?\s*\|",
            md_content,
            re.IGNORECASE,
        )

    if m:
        val1 = m.group(1)
        val2 = m.group(2)
        if version.lower() in eng_first.lower():
            return val1
        elif version.lower() in eng_second.lower():
            return val2

    return "N/A"


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Compile comprehensive reports for historical versions."
    )
    parser.add_argument(
        "--version", type=str, required=True, help="Clean version name under test"
    )
    args = parser.parse_args()

    version = args.version
    historical_dir = "historical"

    # Discover versions
    files = sorted(glob.glob(os.path.join(historical_dir, "lingine-*")))
    clean_names = [os.path.basename(f).replace("lingine-", "") for f in files]

    if version not in clean_names:
        print(
            f"Error: Version '{version}' not found in discovered historical versions."
        )
        print(f"Discovered: {clean_names}")
        sys.exit(1)

    idx = clean_names.index(version)
    prev_version = clean_names[idx - 1] if idx > 0 else None
    base_version = clean_names[0]

    # Paths
    gauntlet_path = f"matches/historical/gauntlets/{version}/summary.md"
    neighbor_path = (
        f"matches/historical/neighbor_matches/{prev_version}-vs-{version}/summary.md"
        if prev_version
        else None
    )
    base_path = (
        f"matches/historical/base_matches/{version}-vs-{base_version}/summary.md"
        if idx > 0
        else None
    )

    # Parse ELOs
    average_elo = "N/A"
    diff_last = "N/A"
    diff_base = "N/A"

    # Parse Gauntlet
    gauntlet_content = ""
    if os.path.exists(gauntlet_path):
        with open(gauntlet_path, "r", encoding="utf-8") as f:
            gauntlet_content = f.read()
        m = re.search(
            r"Global MLE ELO Rating:.*?\*\*([^-*\s]+(?:\s*ELO)?)\*\*",
            gauntlet_content,
            re.IGNORECASE,
        )
        if m:
            average_elo = m.group(1).strip()
        else:
            # Fallback if double asterisks are slightly different
            m = re.search(
                r"Global MLE ELO Rating:.*?\*\*([^\*]+)\*\*",
                gauntlet_content,
                re.IGNORECASE,
            )
            if m:
                average_elo = m.group(1).strip()
    else:
        gauntlet_content = f"*Gauntlet results not found at `{gauntlet_path}`.*"

    # Parse Neighbor Match
    neighbor_content = ""
    if neighbor_path:
        if os.path.exists(neighbor_path):
            with open(neighbor_path, "r", encoding="utf-8") as f:
                neighbor_content = f.read()
            diff_last = extract_elo_diff(neighbor_content, version)
        else:
            neighbor_content = (
                f"*Neighbor match results not found at `{neighbor_path}`.*"
            )
    else:
        neighbor_content = "*This is the base version; no neighbor match exists.*"

    # Parse Base Match
    base_content = ""
    if base_path:
        if os.path.exists(base_path):
            with open(base_path, "r", encoding="utf-8") as f:
                base_content = f.read()
            diff_base = extract_elo_diff(base_content, version)
        else:
            base_content = f"*Base match results not found at `{base_path}`.*"
    else:
        base_content = "*This is the base version; no base match exists.*"

    # Construct comprehensive summary
    os.makedirs("matches/historical/reports", exist_ok=True)
    report_path = f"matches/historical/reports/{version}_comprehensive_report.md"

    tldr_lines = [
        "## 📊 TL;DR Summary",
        f"- **Calculated Average ELO:** **{average_elo}**",
        f"- **ELO Difference against last version ({prev_version or 'N/A'}):** **{diff_last}**",
        f"- **ELO Difference against base version ({base_version}):** **{diff_base}**",
    ]
    tldr = "\n".join(tldr_lines)

    full_report = f"""# Comprehensive Version Report: {version}

{tldr}

---

## 🛡️ Phase 1: Gauntlet Performance
{gauntlet_content}

---

## ⚔️ Phase 2: Neighbor Match ({prev_version or "N/A"} vs {version})
{neighbor_content}

---

## ⏳ Phase 3: Base Match ({version} vs {base_version})
{base_content}
"""

    with open(report_path, "w", encoding="utf-8") as f:
        f.write(full_report)

    # Print the TLDR in terminal-friendly layout
    CYAN = "\033[0;36m"
    GREEN = "\033[0;32m"
    YELLOW = "\033[0;33m"
    BOLD = "\033[1m"
    NC = "\033[0m"

    print(
        f"\n{CYAN}======================================================================{NC}"
    )
    print(
        f"{CYAN}{BOLD}       COMPREHENSIVE REPORT COMPILED FOR: {version.upper()}{NC}"
    )
    print(
        f"{CYAN}======================================================================{NC}"
    )
    print(f"  -> Calculated Average ELO:               {GREEN}{BOLD}{average_elo}{NC}")
    print(
        f"  -> ELO Diff against last version ({prev_version or 'None'}): {YELLOW}{diff_last}{NC}"
    )
    print(
        f"  -> ELO Diff against base version ({base_version}): {YELLOW}{diff_base}{NC}"
    )
    print("----------------------------------------------------------------------")
    print(f"Full markdown report saved to: {BOLD}{report_path}{NC}")
    print(
        f"{CYAN}======================================================================{NC}\n"
    )


if __name__ == "__main__":
    main()
