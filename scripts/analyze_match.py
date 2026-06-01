#!/usr/bin/env python3
"""
Analyze a head-to-head match PGN between two engines.

Computes:
  - Win / Draw / Loss from each engine's perspective
  - Score percentage and Elo difference (with confidence interval)
  - Likelihood of Superiority (LOS)
  - Per-color (Red/Black) performance breakdown
  - Game length statistics
  - Optional HTML dashboard report (-o/--html)
  - Optional Markdown summary report (-m/--markdown)

Usage:
    python scripts/analyze_match.py match.pgn
    python scripts/analyze_match.py match.pgn -o match_report.html
    python scripts/analyze_match.py match.pgn -m match_report.md
    python scripts/analyze_match.py match.pgn --engine1 lingine-pst
"""

import os
import sys
import re
import math
import argparse
import statistics
from datetime import datetime


# ─── Terminal Colors ──────────────────────────────────────────────────────────

def color(text, code):
    return f"\033[{code}m{text}\033[0m"

def bold(text):    return color(text, "1")
def green(text):   return color(text, "32")
def red(text):     return color(text, "31")
def yellow(text):  return color(text, "33")
def blue(text):    return color(text, "34")
def cyan(text):    return color(text, "36")
def gray(text):    return color(text, "90")
def white_bold(text): return color(text, "1;37")

def print_header(title):
    print("=" * 85)
    print(f"   {bold(title.upper())}   ")
    print("=" * 85)


# ─── PGN Parser ──────────────────────────────────────────────────────────────

def parse_pgn(pgn_path):
    """Parse a PGN file and return a list of game dicts."""
    if not os.path.exists(pgn_path):
        return []

    with open(pgn_path, "r", encoding="utf-8") as f:
        content = f.read()

    games_raw = re.split(r'\[Event\s+', content)
    results = []

    for game in games_raw:
        if not game.strip():
            continue

        # Support both Xiangqi [Red ...] and standard [White ...]
        white_match = re.search(r'\[(?:White|Red)\s+"([^"]+)"\]', game)
        black_match = re.search(r'\[Black\s+"([^"]+)"\]', game)
        result_match = re.search(r'\[Result\s+"([^"]+)"\]', game)
        round_match = re.search(r'\[Round\s+"([^"]+)"\]', game)
        date_match = re.search(r'\[Date\s+"([^"]+)"\]', game)
        tc_match = re.search(r'\[TimeControl\s+"([^"]+)"\]', game)
        plycount_match = re.search(r'\[PlyCount\s+"([^"]+)"\]', game)
        duration_match = re.search(r'\[GameDuration\s+"([^"]+)"\]', game)

        if white_match and black_match and result_match:
            ply = int(plycount_match.group(1)) if plycount_match else None
            results.append({
                "white": white_match.group(1),
                "black": black_match.group(1),
                "result": result_match.group(1),
                "round": round_match.group(1) if round_match else "?",
                "date": date_match.group(1) if date_match else "?",
                "tc": tc_match.group(1) if tc_match else "unknown",
                "plycount": ply,
                "duration": duration_match.group(1) if duration_match else None,
            })
    return results


# ─── Engine detection ─────────────────────────────────────────────────────────

def detect_engines(games):
    """Detect the two engine names from the PGN."""
    players = set()
    for g in games:
        players.add(g["white"])
        players.add(g["black"])

    if len(players) != 2:
        print(yellow(f"Warning: Expected exactly 2 players, found {len(players)}: {players}"))

    return sorted(players)


# ─── Match Statistics ─────────────────────────────────────────────────────────

def compute_match_stats(games, engine1, engine2):
    """Compute comprehensive head-to-head statistics for engine1 vs engine2."""

    stats = {
        "engine1": engine1,
        "engine2": engine2,
        "total_games": 0,
        # From engine1's perspective
        "e1_wins": 0,
        "e1_draws": 0,
        "e1_losses": 0,
        # Color breakdown for engine1
        "e1_as_red_games": 0,
        "e1_as_red_wins": 0,
        "e1_as_red_draws": 0,
        "e1_as_red_losses": 0,
        "e1_as_black_games": 0,
        "e1_as_black_wins": 0,
        "e1_as_black_draws": 0,
        "e1_as_black_losses": 0,
        # Game length data
        "plycounts": [],
        "durations": [],
        # Termination types
        "decisive_count": 0,
        "draw_count": 0,
        # Per-game results for streaks/detail
        "game_results": [],
    }

    skipped = 0
    for game in games:
        w, b, res = game["white"], game["black"], game["result"]

        # Determine engine1's color
        if w == engine1 and b == engine2:
            e1_color = "red"
        elif w == engine2 and b == engine1:
            e1_color = "black"
        else:
            skipped += 1
            continue

        stats["total_games"] += 1

        if game["plycount"] is not None:
            stats["plycounts"].append(game["plycount"])
        if game["duration"] is not None:
            stats["durations"].append(game["duration"])

        # Determine outcome from engine1's perspective
        if res == "1-0":
            if e1_color == "red":
                outcome = "win"
            else:
                outcome = "loss"
        elif res == "0-1":
            if e1_color == "black":
                outcome = "win"
            else:
                outcome = "loss"
        elif res in ["1/2-1/2", "0.5-0.5", "1/2"]:
            outcome = "draw"
        else:
            outcome = "draw"  # Treat unknown as draw

        stats["game_results"].append({
            "round": game["round"],
            "e1_color": e1_color,
            "outcome": outcome,
            "plycount": game["plycount"],
        })

        if outcome == "win":
            stats["e1_wins"] += 1
            stats["decisive_count"] += 1
        elif outcome == "loss":
            stats["e1_losses"] += 1
            stats["decisive_count"] += 1
        else:
            stats["e1_draws"] += 1
            stats["draw_count"] += 1

        # Color breakdown
        if e1_color == "red":
            stats["e1_as_red_games"] += 1
            if outcome == "win":
                stats["e1_as_red_wins"] += 1
            elif outcome == "loss":
                stats["e1_as_red_losses"] += 1
            else:
                stats["e1_as_red_draws"] += 1
        else:
            stats["e1_as_black_games"] += 1
            if outcome == "win":
                stats["e1_as_black_wins"] += 1
            elif outcome == "loss":
                stats["e1_as_black_losses"] += 1
            else:
                stats["e1_as_black_draws"] += 1

    return stats, skipped


def compute_elo_diff(wins, draws, losses):
    """
    Compute Elo difference from score percentage.
    Returns (elo_diff, standard_error, ci_low, ci_high, los).
    """
    n = wins + draws + losses
    if n == 0:
        return 0.0, None, None, None, None

    points = wins + 0.5 * draws
    score_pct = points / n

    # Elo difference from logistic model
    # Use standard half-game correction for extreme scores (0% and 100%) to maintain monotonicity
    if score_pct >= 0.999:
        adjusted_score = (n - 0.5) / n if n > 0 else 0.999
    elif score_pct <= 0.001:
        adjusted_score = 0.5 / n if n > 0 else 0.001
    else:
        adjusted_score = score_pct

    elo_diff = 400.0 * math.log10(adjusted_score / (1.0 - adjusted_score))

    # Standard Error using trinomial variance
    # Var(score) = (W*(1-mu)^2 + D*(0.5-mu)^2 + L*(0-mu)^2) / (n-1)
    # where mu = score_pct
    if n > 1:
        mu = score_pct
        var_score = (wins * (1.0 - mu)**2
                     + draws * (0.5 - mu)**2
                     + losses * (0.0 - mu)**2) / (n - 1)
        se_score = math.sqrt(var_score / n)

        # Convert SE in score-space to SE in Elo-space via derivative using adjusted score to prevent division by zero
        dElo_dS = 400.0 / (adjusted_score * (1.0 - adjusted_score) * math.log(10.0))
        se_elo = abs(dElo_dS) * se_score
        ci_low = elo_diff - 1.96 * se_elo
        ci_high = elo_diff + 1.96 * se_elo
    else:
        se_elo = None
        ci_low = None
        ci_high = None

    # LOS (Likelihood of Superiority) — probability that engine1 is stronger
    # Based on the assumption that the score difference follows a normal distribution
    if n > 0 and se_elo is not None and se_elo > 0:
        los = 0.5 * (1.0 + math.erf(elo_diff / (se_elo * math.sqrt(2.0))))
    elif score_pct > 0.5:
        los = 1.0
    elif score_pct < 0.5:
        los = 0.0
    else:
        los = 0.5

    return elo_diff, se_elo, ci_low, ci_high, los


def parse_duration_to_seconds(dur_str):
    """Parse 'HH:MM:SS' to total seconds."""
    parts = dur_str.split(":")
    if len(parts) == 3:
        return int(parts[0]) * 3600 + int(parts[1]) * 60 + int(parts[2])
    return 0


# ─── Terminal Report ──────────────────────────────────────────────────────────

def print_console_report(stats, elo_diff, se_elo, ci_low, ci_high, los):
    """Print a detailed console report."""
    e1 = stats["engine1"]
    e2 = stats["engine2"]
    n = stats["total_games"]

    w, d, l = stats["e1_wins"], stats["e1_draws"], stats["e1_losses"]
    pts = w + 0.5 * d
    score_pct = (pts / n * 100) if n > 0 else 0

    # Summary header
    print(f"\n{bold('MATCH')}: {green(e1)} vs {red(e2)}")
    print(f"Total Games: {bold(str(n))}")
    if stats["game_results"]:
        tc = None
        # Try to find TC from first game
        for gr in stats["game_results"]:
            break
    print()

    # Main results table
    print(f"{'':─<85}")
    print(f"  {bold('ENGINE COMPARISON (from ' + e1 + ' perspective)')}")
    print(f"{'':─<85}")
    print(f"  {'Metric':<30} {e1:<20} {e2:<20}")
    print(f"  {'-'*70}")
    print(f"  {'Wins':<30} {green(str(w)):<29} {green(str(l)):<20}")
    print(f"  {'Draws':<30} {yellow(str(d)):<29} {yellow(str(d)):<20}")
    print(f"  {'Losses':<30} {red(str(l)):<29} {red(str(w)):<20}")
    print(f"  {'Points':<30} {str(pts):<20} {str(n - pts):<20}")
    print(f"  {'Score %':<30} {f'{score_pct:.1f}%':<20} {f'{100 - score_pct:.1f}%':<20}")
    print()

    # Elo difference
    elo_sign = "+" if elo_diff >= 0 else ""
    elo_color = green if elo_diff >= 0 else red
    print(f"  {bold('ELO DIFFERENCE')}: {elo_color(f'{elo_sign}{elo_diff:.1f}')}")
    if se_elo is not None:
        print(f"  Standard Error:  ± {se_elo:.1f}")
        if ci_low is not None:
            print(f"  95% CI:          [{ci_low:.1f}, {ci_high:.1f}]")
    if los is not None:
        los_pct = los * 100
        los_color = green if los_pct > 75 else (yellow if los_pct > 50 else red)
        print(f"  LOS:             {los_color(f'{los_pct:.1f}%')}")
    print()

    # Draw rate
    draw_rate = (d / n * 100) if n > 0 else 0
    print(f"  Draw Rate:       {yellow(f'{draw_rate:.1f}%')} ({d}/{n})")
    print()

    # Color performance
    print(f"{'':─<85}")
    print(f"  {bold('COLOR PERFORMANCE FOR ' + e1.upper())}")
    print(f"{'':─<85}")
    print(f"  {'Color':<15} {'Games':<8} {'Wins':<8} {'Draws':<8} {'Losses':<8} {'Score %':<10}")
    print(f"  {'-'*57}")

    rg = stats["e1_as_red_games"]
    rw = stats["e1_as_red_wins"]
    rd = stats["e1_as_red_draws"]
    rl = stats["e1_as_red_losses"]
    r_pts = rw + 0.5 * rd
    r_pct = (r_pts / rg * 100) if rg > 0 else 0

    bg = stats["e1_as_black_games"]
    bw = stats["e1_as_black_wins"]
    bd = stats["e1_as_black_draws"]
    bl = stats["e1_as_black_losses"]
    b_pts = bw + 0.5 * bd
    b_pct = (b_pts / bg * 100) if bg > 0 else 0

    print(f"  {red('Red (First)'):<24} {rg:<8} {rw:<8} {rd:<8} {rl:<8} {r_pct:.1f}%")
    print(f"  {'Black (Second)':<15} {bg:<8} {bw:<8} {bd:<8} {bl:<8} {b_pct:.1f}%")
    print()

    # Game length statistics
    if stats["plycounts"]:
        plies = stats["plycounts"]
        avg_ply = statistics.mean(plies)
        med_ply = statistics.median(plies)
        min_ply = min(plies)
        max_ply = max(plies)
        avg_moves = avg_ply / 2

        print(f"{'':─<85}")
        print(f"  {bold('GAME LENGTH STATISTICS')}")
        print(f"{'':─<85}")
        print(f"  Average Ply Count: {avg_ply:.1f}  (≈ {avg_moves:.0f} moves)")
        print(f"  Median Ply Count:  {med_ply:.0f}")
        print(f"  Range:             {min_ply} – {max_ply} plies")
        if len(plies) > 1:
            std_ply = statistics.stdev(plies)
            print(f"  Std Deviation:     {std_ply:.1f}")
        print()

    # Game-by-game result streak
    print(f"{'':─<85}")
    print(f"  {bold('GAME SEQUENCE')} (from {e1} perspective)")
    print(f"{'':─<85}")
    sequence_str = "  "
    for i, gr in enumerate(stats["game_results"]):
        if gr["outcome"] == "win":
            sequence_str += green("W")
        elif gr["outcome"] == "loss":
            sequence_str += red("L")
        else:
            sequence_str += yellow("D")

        if (i + 1) % 50 == 0:
            sequence_str += "\n  "
    print(sequence_str)

    # Winning/losing streaks
    max_win_streak = 0
    max_loss_streak = 0
    cur_win = 0
    cur_loss = 0
    for gr in stats["game_results"]:
        if gr["outcome"] == "win":
            cur_win += 1
            cur_loss = 0
        elif gr["outcome"] == "loss":
            cur_loss += 1
            cur_win = 0
        else:
            cur_win = 0
            cur_loss = 0
        max_win_streak = max(max_win_streak, cur_win)
        max_loss_streak = max(max_loss_streak, cur_loss)

    print(f"\n  Longest Win Streak:  {green(str(max_win_streak))}")
    print(f"  Longest Loss Streak: {red(str(max_loss_streak))}")
    print()

    # Markdown table
    print(f"{'':═<85}")
    print(f"  {bold('COPY-PASTEABLE MARKDOWN TABLE')}")
    print(f"{'':═<85}")
    print(f"| Metric | {e1} | {e2} |")
    print(f"| :--- | :---: | :---: |")
    print(f"| **Wins** | {w} | {l} |")
    print(f"| **Draws** | {d} | {d} |")
    print(f"| **Losses** | {l} | {w} |")
    print(f"| **Score** | {pts}/{n} ({score_pct:.1f}%) | {n - pts}/{n} ({100 - score_pct:.1f}%) |")
    elo_str = f"{elo_sign}{elo_diff:.0f}"
    ci_str = f"[{ci_low:.0f}, {ci_high:.0f}]" if ci_low is not None else "N/A"
    los_str = f"{los * 100:.1f}%" if los is not None else "N/A"
    print(f"| **Elo Diff** | {elo_str} | {'+' if elo_diff <= 0 else ''}{-elo_diff:.0f} |")
    print(f"| **95% CI** | {ci_str} | — |")
    print(f"| **LOS** | {los_str} | — |")
    print(f"| **Draw Rate** | {draw_rate:.1f}% | — |")
    print(f"{'':═<85}")
    print()

# ─── Markdown Report ──────────────────────────────────────────────────────────

def generate_markdown_report(filepath, stats, elo_diff, se_elo, ci_low, ci_high, los):
    """Generate a clean, beautiful Markdown report for the match."""
    now_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    e1 = stats["engine1"]
    e2 = stats["engine2"]
    n = stats["total_games"]
    w, d, l = stats["e1_wins"], stats["e1_draws"], stats["e1_losses"]
    pts = w + 0.5 * d
    score_pct = (pts / n * 100) if n > 0 else 0
    draw_rate = (d / n * 100) if n > 0 else 0

    elo_sign = "+" if elo_diff >= 0 else ""
    elo_str = f"{elo_sign}{elo_diff:.1f}"
    ci_str = f"[{ci_low:.1f}, {ci_high:.1f}]" if ci_low is not None else "N/A"
    se_str = f"± {se_elo:.1f}" if se_elo is not None else "N/A"
    los_str = f"{los * 100:.1f}%" if los is not None else "N/A"

    # Color performance
    rg = stats["e1_as_red_games"]
    rw = stats["e1_as_red_wins"]
    rd = stats["e1_as_red_draws"]
    rl = stats["e1_as_red_losses"]
    r_pts = rw + 0.5 * rd
    r_pct = (r_pts / rg * 100) if rg > 0 else 0

    bg = stats["e1_as_black_games"]
    bw = stats["e1_as_black_wins"]
    bd = stats["e1_as_black_draws"]
    bl = stats["e1_as_black_losses"]
    b_pts = bw + 0.5 * bd
    b_pct = (b_pts / bg * 100) if bg > 0 else 0

    # Longest streaks
    max_win_streak = 0
    max_loss_streak = 0
    cur_win = 0
    cur_loss = 0
    for gr in stats["game_results"]:
        if gr["outcome"] == "win":
            cur_win += 1
            cur_loss = 0
        elif gr["outcome"] == "loss":
            cur_loss += 1
            cur_win = 0
        else:
            cur_win = 0
            cur_loss = 0
        max_win_streak = max(max_win_streak, cur_win)
        max_loss_streak = max(max_loss_streak, cur_loss)

    # Game length
    game_len_str = "N/A"
    if stats["plycounts"]:
        plies = stats["plycounts"]
        avg_ply = statistics.mean(plies)
        med_ply = statistics.median(plies)
        min_ply = min(plies)
        max_ply = max(plies)
        game_len_str = f"Average: {avg_ply:.1f} plies (≈ {avg_ply/2:.0f} moves), Median: {med_ply:.0f}, Range: {min_ply} - {max_ply} plies"

    md_content = f"""# Match Report: {e1} vs {e2}

Generated on: {now_str}
Total Games Played: **{n}**

## Engine Comparison (from {e1}'s perspective)

| Metric | {e1} | {e2} |
| :--- | :---: | :---: |
| **Wins** | {w} | {l} |
| **Draws** | {d} | {d} |
| **Losses** | {l} | {w} |
| **Points** | {pts} / {n} ({score_pct:.1f}%) | {n - pts} / {n} ({100 - score_pct:.1f}%) |
| **Elo Diff** | **{elo_str}** | **{'-' if elo_diff >= 0 else '+'}{abs(elo_diff):.1f}** |
| **Standard Error** | {se_str} | — |
| **95% Confidence Interval** | {ci_str} | — |
| **LOS (Likelihood of Superiority)** | {los_str} | — |
| **Draw Rate** | {draw_rate:.1f}% | — |

## Color Performance (from {e1}'s perspective)

| Color | Games | Wins | Draws | Losses | Score % |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Red (First)** | {rg} | {rw} | {rd} | {rl} | {r_pct:.1f}% |
| **Black (Second)** | {bg} | {bw} | {bd} | {bl} | {b_pct:.1f}% |

## Performance Streaks

- **Longest Win Streak:** {max_win_streak} games
- **Longest Loss Streak:** {max_loss_streak} games

## Game Length Statistics

- **Ply Count:** {game_len_str}
"""
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(md_content)


# ─── HTML Report ──────────────────────────────────────────────────────────────

def generate_html_report(filepath, stats, elo_diff, se_elo, ci_low, ci_high, los):
    """Generate a premium responsive HTML dashboard for the match."""
    now_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    e1 = stats["engine1"]
    e2 = stats["engine2"]
    n = stats["total_games"]
    w, d, l = stats["e1_wins"], stats["e1_draws"], stats["e1_losses"]
    pts = w + 0.5 * d
    score_pct = (pts / n * 100) if n > 0 else 0
    draw_rate = (d / n * 100) if n > 0 else 0
    e2_pts = n - pts
    e2_score_pct = 100 - score_pct

    elo_sign = "+" if elo_diff >= 0 else ""
    elo_str = f"{elo_sign}{elo_diff:.0f}"
    ci_str = f"[{ci_low:.0f}, {ci_high:.0f}]" if ci_low is not None else "N/A"
    se_str = f"± {se_elo:.1f}" if se_elo is not None else "N/A"
    los_pct = (los * 100) if los is not None else 0

    # Color performance
    rg = stats["e1_as_red_games"]
    rw = stats["e1_as_red_wins"]
    rd = stats["e1_as_red_draws"]
    rl = stats["e1_as_red_losses"]
    r_pts = rw + 0.5 * rd
    r_pct = (r_pts / rg * 100) if rg > 0 else 0

    bg = stats["e1_as_black_games"]
    bw = stats["e1_as_black_wins"]
    bd = stats["e1_as_black_draws"]
    bl = stats["e1_as_black_losses"]
    b_pts = bw + 0.5 * bd
    b_pct = (b_pts / bg * 100) if bg > 0 else 0

    # Game length stats
    avg_ply = statistics.mean(stats["plycounts"]) if stats["plycounts"] else 0
    med_ply = statistics.median(stats["plycounts"]) if stats["plycounts"] else 0
    min_ply = min(stats["plycounts"]) if stats["plycounts"] else 0
    max_ply = max(stats["plycounts"]) if stats["plycounts"] else 0
    avg_moves = avg_ply / 2

    # Game sequence visualization
    game_dots = ""
    for gr in stats["game_results"]:
        if gr["outcome"] == "win":
            game_dots += '<span class="dot dot-win" title="Win">●</span>'
        elif gr["outcome"] == "loss":
            game_dots += '<span class="dot dot-loss" title="Loss">●</span>'
        else:
            game_dots += '<span class="dot dot-draw" title="Draw">●</span>'

    # Win rate bar chart segments
    win_bar = (w / n * 100) if n > 0 else 0
    draw_bar = (d / n * 100) if n > 0 else 0
    loss_bar = (l / n * 100) if n > 0 else 0

    # Elo difference color
    elo_color = "#10b981" if elo_diff >= 0 else "#ef4444"
    los_color = "#10b981" if los_pct > 75 else ("#eab308" if los_pct > 50 else "#ef4444")

    # Determine winner for highlight
    if score_pct > 50:
        winner_name = e1
        winner_pct = score_pct
    elif score_pct < 50:
        winner_name = e2
        winner_pct = e2_score_pct
    else:
        winner_name = "Tied"
        winner_pct = 50.0

    html_content = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{e1} vs {e2} — Match Analysis</title>
    <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;500;600;700;800&family=Plus+Jakarta+Sans:wght@300;400;500;600;700;800&display=swap" rel="stylesheet">
    <style>
        :root {{
            --bg-color: #0b0f19;
            --card-bg: rgba(17, 24, 39, 0.7);
            --card-border: rgba(255, 255, 255, 0.08);
            --text-main: #f3f4f6;
            --text-muted: #9ca3af;
            --accent-color: #3b82f6;
            --accent-glow: rgba(59, 130, 246, 0.35);
            --emerald: #10b981;
            --rose: #ef4444;
            --amber: #eab308;
            --purple: #8b5cf6;
            --font-display: 'Outfit', 'Plus Jakarta Sans', sans-serif;
            --font-sans: 'Plus Jakarta Sans', sans-serif;
        }}

        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}

        body {{
            background-color: var(--bg-color);
            background-image:
                radial-gradient(at 0% 0%, rgba(29, 78, 216, 0.15) 0px, transparent 50%),
                radial-gradient(at 100% 100%, rgba(16, 185, 129, 0.08) 0px, transparent 50%),
                radial-gradient(at 50% 50%, rgba(139, 92, 246, 0.06) 0px, transparent 50%);
            background-attachment: fixed;
            color: var(--text-main);
            font-family: var(--font-sans);
            line-height: 1.6;
            padding: 2rem 1.5rem;
        }}

        .container {{
            max-width: 1200px;
            margin: 0 auto;
        }}

        header {{
            margin-bottom: 2.5rem;
            text-align: center;
        }}

        .header-pre {{
            font-family: var(--font-display);
            text-transform: uppercase;
            letter-spacing: 0.15em;
            font-size: 0.85rem;
            color: var(--accent-color);
            font-weight: 700;
            margin-bottom: 0.5rem;
        }}

        h1 {{
            font-family: var(--font-display);
            font-size: 2.75rem;
            font-weight: 800;
            background: linear-gradient(135deg, #ffffff 30%, #a5b4fc 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            letter-spacing: -0.02em;
            margin-bottom: 0.5rem;
        }}

        .vs-badge {{
            display: inline-block;
            background: rgba(139, 92, 246, 0.15);
            border: 1px solid rgba(139, 92, 246, 0.3);
            padding: 0.15rem 0.6rem;
            border-radius: 6px;
            font-size: 0.8rem;
            font-weight: 700;
            color: var(--purple);
            margin: 0 0.5rem;
            vertical-align: middle;
        }}

        .meta-tag {{
            display: inline-block;
            background: rgba(255, 255, 255, 0.04);
            border: 1px solid var(--card-border);
            padding: 0.35rem 1rem;
            border-radius: 9999px;
            font-size: 0.85rem;
            color: var(--text-muted);
            margin-top: 0.5rem;
        }}

        /* Grid layout */
        .dashboard-grid {{
            display: grid;
            grid-template-columns: repeat(4, 1fr);
            gap: 1.5rem;
            margin-bottom: 2rem;
        }}

        .dashboard-grid-3 {{
            display: grid;
            grid-template-columns: repeat(3, 1fr);
            gap: 1.5rem;
            margin-bottom: 2rem;
        }}

        @media (max-width: 1024px) {{
            .dashboard-grid {{
                grid-template-columns: repeat(2, 1fr);
            }}
            .dashboard-grid-3 {{
                grid-template-columns: 1fr;
            }}
        }}

        @media (max-width: 640px) {{
            .dashboard-grid {{
                grid-template-columns: 1fr;
            }}
        }}

        /* Card styles */
        .card {{
            background: var(--card-bg);
            border: 1px solid var(--card-border);
            border-radius: 1.25rem;
            padding: 1.5rem;
            backdrop-filter: blur(16px);
            -webkit-backdrop-filter: blur(16px);
            transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
            position: relative;
            overflow: hidden;
        }}

        .card:hover {{
            transform: translateY(-4px);
            border-color: rgba(255, 255, 255, 0.15);
            box-shadow: 0 12px 24px -10px rgba(0, 0, 0, 0.5);
        }}

        .card-elo {{
            grid-column: span 2;
            background: radial-gradient(circle at 100% 0%, rgba(59, 130, 246, 0.12) 0%, transparent 70%), var(--card-bg);
            border-color: rgba(59, 130, 246, 0.2);
            box-shadow: 0 0 40px -10px var(--accent-glow);
        }}

        .card-elo::before {{
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            width: 4px;
            height: 100%;
            background: linear-gradient(to bottom, var(--accent-color), #818cf8);
        }}

        @media (max-width: 640px) {{
            .card-elo {{
                grid-column: span 1;
            }}
        }}

        .card-label {{
            font-size: 0.85rem;
            text-transform: uppercase;
            letter-spacing: 0.08em;
            color: var(--text-muted);
            font-weight: 600;
            margin-bottom: 0.5rem;
        }}

        .card-value {{
            font-family: var(--font-display);
            font-size: 2.25rem;
            font-weight: 800;
            line-height: 1.2;
            color: #ffffff;
        }}

        .card-elo .card-value {{
            font-size: 3.5rem;
            background: linear-gradient(135deg, #ffffff 40%, #60a5fa 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }}

        .card-subtext {{
            font-size: 0.85rem;
            color: var(--text-muted);
            margin-top: 0.5rem;
        }}

        /* Score bar */
        .score-bar-container {{
            width: 100%;
            height: 40px;
            border-radius: 12px;
            overflow: hidden;
            display: flex;
            margin: 1.5rem 0;
            position: relative;
        }}

        .score-bar-win {{
            background: linear-gradient(135deg, #10b981, #059669);
            height: 100%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-weight: 700;
            font-size: 0.85rem;
            color: #fff;
            transition: width 1s ease;
        }}

        .score-bar-draw {{
            background: linear-gradient(135deg, #eab308, #ca8a04);
            height: 100%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-weight: 700;
            font-size: 0.85rem;
            color: #fff;
            transition: width 1s ease;
        }}

        .score-bar-loss {{
            background: linear-gradient(135deg, #ef4444, #dc2626);
            height: 100%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-weight: 700;
            font-size: 0.85rem;
            color: #fff;
            transition: width 1s ease;
        }}

        .score-bar-labels {{
            display: flex;
            justify-content: space-between;
            font-size: 0.8rem;
            color: var(--text-muted);
            margin-top: 0.25rem;
        }}

        /* Table */
        .section-title {{
            font-family: var(--font-display);
            font-size: 1.5rem;
            font-weight: 700;
            margin-bottom: 1.25rem;
            display: flex;
            align-items: center;
            gap: 0.5rem;
        }}

        .section-title::before {{
            content: '';
            display: inline-block;
            width: 8px;
            height: 20px;
            background: var(--accent-color);
            border-radius: 99px;
        }}

        table {{
            width: 100%;
            border-collapse: collapse;
            text-align: left;
        }}

        th, td {{
            padding: 1rem 1.25rem;
            border-bottom: 1px solid rgba(255, 255, 255, 0.05);
            font-size: 0.95rem;
        }}

        th {{
            font-family: var(--font-display);
            font-weight: 600;
            text-transform: uppercase;
            font-size: 0.75rem;
            letter-spacing: 0.08em;
            color: var(--text-muted);
            background: rgba(255, 255, 255, 0.02);
        }}

        tr:hover td {{
            background-color: rgba(255, 255, 255, 0.015);
        }}

        /* Game sequence dots */
        .game-sequence {{
            display: flex;
            flex-wrap: wrap;
            gap: 4px;
            padding: 1rem 0;
        }}

        .dot {{
            width: 16px;
            height: 16px;
            font-size: 16px;
            line-height: 1;
            cursor: default;
            transition: transform 0.15s ease;
        }}

        .dot:hover {{
            transform: scale(1.5);
        }}

        .dot-win {{ color: var(--emerald); }}
        .dot-loss {{ color: var(--rose); }}
        .dot-draw {{ color: var(--amber); }}

        /* Details grid */
        .details-grid {{
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 1.5rem;
            margin-bottom: 2rem;
        }}

        @media (max-width: 768px) {{
            .details-grid {{
                grid-template-columns: 1fr;
            }}
        }}

        .main-section {{
            margin-bottom: 2rem;
        }}

        .formula-card {{
            font-family: monospace;
            background: rgba(0, 0, 0, 0.2);
            border-radius: 8px;
            padding: 1rem;
            border: 1px solid rgba(255, 255, 255, 0.03);
            margin: 1rem 0;
            overflow-x: auto;
            color: #a5b4fc;
        }}

        .bullet-list {{
            list-style-type: none;
        }}

        .bullet-list li {{
            margin-bottom: 0.75rem;
            padding-left: 1.5rem;
            position: relative;
        }}

        .bullet-list li::before {{
            content: '→';
            position: absolute;
            left: 0;
            color: var(--accent-color);
            font-weight: 700;
        }}

        footer {{
            margin-top: 4rem;
            text-align: center;
            font-size: 0.85rem;
            color: var(--text-muted);
            border-top: 1px solid rgba(255, 255, 255, 0.05);
            padding-top: 1.5rem;
        }}

        .badge {{
            display: inline-block;
            padding: 0.2rem 0.65rem;
            border-radius: 6px;
            font-size: 0.8rem;
            font-weight: 600;
            text-align: center;
        }}

        .los-bar {{
            width: 100%;
            height: 8px;
            background: rgba(255, 255, 255, 0.08);
            border-radius: 4px;
            overflow: hidden;
            margin-top: 0.75rem;
        }}

        .los-fill {{
            height: 100%;
            border-radius: 4px;
            transition: width 1s ease;
        }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <div class="header-pre">HEAD-TO-HEAD MATCH ANALYSIS</div>
            <h1>{e1} <span class="vs-badge">VS</span> {e2}</h1>
            <div class="meta-tag">Generated: {now_str} • {n} games analyzed</div>
        </header>

        <!-- Score Bar -->
        <div class="card main-section">
            <h2 class="section-title">Result Distribution</h2>
            <div class="score-bar-container">
                <div class="score-bar-win" style="width: {win_bar:.1f}%;">{w}W</div>
                <div class="score-bar-draw" style="width: {draw_bar:.1f}%;">{d}D</div>
                <div class="score-bar-loss" style="width: {loss_bar:.1f}%;">{l}L</div>
            </div>
            <div class="score-bar-labels">
                <span style="color: var(--emerald);">{e1} wins: {w} ({win_bar:.1f}%)</span>
                <span style="color: var(--amber);">Draws: {d} ({draw_rate:.1f}%)</span>
                <span style="color: var(--rose);">{e2} wins: {l} ({loss_bar:.1f}%)</span>
            </div>
        </div>

        <!-- Dashboard Widgets -->
        <div class="dashboard-grid">
            <div class="card card-elo">
                <div class="card-label">Elo Difference ({e1})</div>
                <div class="card-value" style="color: {elo_color}; -webkit-text-fill-color: {elo_color};">{elo_str}</div>
                <div class="card-subtext">{se_str} SE • 95% CI: {ci_str}</div>
            </div>

            <div class="card">
                <div class="card-label">Likelihood of Superiority</div>
                <div class="card-value" style="color: {los_color}; font-size: 2.5rem;">{los_pct:.1f}%</div>
                <div class="los-bar"><div class="los-fill" style="width: {los_pct:.1f}%; background: {los_color};"></div></div>
                <div class="card-subtext">Probability {e1} is stronger</div>
            </div>

            <div class="card">
                <div class="card-label">{e1} Score</div>
                <div class="card-value" style="color: {'var(--emerald)' if score_pct > 50 else ('var(--rose)' if score_pct < 50 else 'var(--amber)')}; font-size: 2.5rem;">{score_pct:.1f}%</div>
                <div class="card-subtext">{pts}/{n} points</div>
            </div>
        </div>

        <!-- Head-to-head Comparison Table -->
        <div class="card main-section" style="padding: 0; overflow: hidden;">
            <div style="padding: 1.5rem 1.5rem 0.5rem 1.5rem;">
                <h2 class="section-title" style="margin-bottom: 0;">Head-to-Head Comparison</h2>
            </div>
            <table>
                <thead>
                    <tr>
                        <th>Metric</th>
                        <th>{e1}</th>
                        <th>{e2}</th>
                    </tr>
                </thead>
                <tbody>
                    <tr>
                        <td style="font-weight: 600;">Wins</td>
                        <td style="color: var(--emerald); font-weight: 700;">{w}</td>
                        <td style="color: var(--emerald); font-weight: 700;">{l}</td>
                    </tr>
                    <tr>
                        <td style="font-weight: 600;">Draws</td>
                        <td style="color: var(--amber); font-weight: 700;">{d}</td>
                        <td style="color: var(--amber); font-weight: 700;">{d}</td>
                    </tr>
                    <tr>
                        <td style="font-weight: 600;">Losses</td>
                        <td style="color: var(--rose); font-weight: 700;">{l}</td>
                        <td style="color: var(--rose); font-weight: 700;">{w}</td>
                    </tr>
                    <tr>
                        <td style="font-weight: 600;">Points</td>
                        <td style="font-weight: 600;">{pts}</td>
                        <td style="font-weight: 600;">{e2_pts}</td>
                    </tr>
                    <tr>
                        <td style="font-weight: 600;">Score %</td>
                        <td style="font-weight: 700; color: {'var(--emerald)' if score_pct > 50 else 'var(--rose)'};">{score_pct:.1f}%</td>
                        <td style="font-weight: 700; color: {'var(--emerald)' if e2_score_pct > 50 else 'var(--rose)'};">{e2_score_pct:.1f}%</td>
                    </tr>
                </tbody>
            </table>
        </div>

        <!-- Color Performance + Game Stats -->
        <div class="details-grid">
            <div class="card">
                <h2 class="section-title">Color Performance ({e1})</h2>
                <table style="width: 100%; border: none;">
                    <thead>
                        <tr style="background: transparent;">
                            <th style="padding: 0.5rem 0; border: none;">Color</th>
                            <th style="padding: 0.5rem 0; border: none;">Games</th>
                            <th style="padding: 0.5rem 0; border: none;">W</th>
                            <th style="padding: 0.5rem 0; border: none;">D</th>
                            <th style="padding: 0.5rem 0; border: none;">L</th>
                            <th style="padding: 0.5rem 0; border: none;">Score %</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td style="border: none; padding: 0.5rem 0; font-weight: 600; color: var(--rose);">Red (First)</td>
                            <td style="border: none; padding: 0.5rem 0;">{rg}</td>
                            <td style="border: none; padding: 0.5rem 0; color: var(--emerald); font-weight: 600;">{rw}</td>
                            <td style="border: none; padding: 0.5rem 0; color: var(--amber); font-weight: 600;">{rd}</td>
                            <td style="border: none; padding: 0.5rem 0; color: var(--rose); font-weight: 600;">{rl}</td>
                            <td style="border: none; padding: 0.5rem 0; font-weight: 600;">{r_pct:.1f}%</td>
                        </tr>
                        <tr>
                            <td style="border: none; padding: 0.5rem 0; font-weight: 600; color: #6b7280;">Black (Second)</td>
                            <td style="border: none; padding: 0.5rem 0;">{bg}</td>
                            <td style="border: none; padding: 0.5rem 0; color: var(--emerald); font-weight: 600;">{bw}</td>
                            <td style="border: none; padding: 0.5rem 0; color: var(--amber); font-weight: 600;">{bd}</td>
                            <td style="border: none; padding: 0.5rem 0; color: var(--rose); font-weight: 600;">{bl}</td>
                            <td style="border: none; padding: 0.5rem 0; font-weight: 600;">{b_pct:.1f}%</td>
                        </tr>
                    </tbody>
                </table>
            </div>

            <div class="card">
                <h2 class="section-title">Game Length Statistics</h2>
                <table style="width: 100%; border: none;">
                    <tbody>
                        <tr>
                            <td style="border: none; padding: 0.5rem 0; color: var(--text-muted);">Average Length</td>
                            <td style="border: none; padding: 0.5rem 0; font-weight: 600;">{avg_ply:.0f} plies (≈ {avg_moves:.0f} moves)</td>
                        </tr>
                        <tr>
                            <td style="border: none; padding: 0.5rem 0; color: var(--text-muted);">Median Length</td>
                            <td style="border: none; padding: 0.5rem 0; font-weight: 600;">{med_ply:.0f} plies</td>
                        </tr>
                        <tr>
                            <td style="border: none; padding: 0.5rem 0; color: var(--text-muted);">Range</td>
                            <td style="border: none; padding: 0.5rem 0; font-weight: 600;">{min_ply} – {max_ply} plies</td>
                        </tr>
                        <tr>
                            <td style="border: none; padding: 0.5rem 0; color: var(--text-muted);">Draw Rate</td>
                            <td style="border: none; padding: 0.5rem 0; font-weight: 600; color: var(--amber);">{draw_rate:.1f}%</td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </div>

        <!-- Game Sequence -->
        <div class="card main-section">
            <h2 class="section-title">Game Sequence ({e1} perspective)</h2>
            <div class="game-sequence">
                {game_dots}
            </div>
            <div style="margin-top: 0.5rem; font-size: 0.8rem; color: var(--text-muted);">
                <span style="color: var(--emerald);">●</span> Win &nbsp;
                <span style="color: var(--amber);">●</span> Draw &nbsp;
                <span style="color: var(--rose);">●</span> Loss
            </div>
        </div>

        <!-- Methodology -->
        <div class="card main-section">
            <h2 class="section-title">Statistical Methodology</h2>
            <ul class="bullet-list" style="margin-top: 0.5rem;">
                <li><strong>Elo Difference</strong>: Computed from the logistic model:
                    <div class="formula-card">Δ_Elo = 400 × log₁₀(Score% / (1 − Score%))</div>
                </li>
                <li><strong>Standard Error</strong>: Derived from the trinomial variance of {{wins, draws, losses}}, then transformed to Elo-space via the logistic derivative dElo/dS = 400 / (S(1−S) × ln10).</li>
                <li><strong>Confidence Interval</strong>: 95% CI computed as Elo ± 1.96 × SE.</li>
                <li><strong>LOS (Likelihood of Superiority)</strong>: The probability that the engine with the higher score is truly stronger, computed from the normalized Elo difference: LOS = Φ(Δ_Elo / SE).</li>
            </ul>
        </div>

        <footer>
            Lingine Match Analyzer • Generated {now_str}
        </footer>
    </div>
</body>
</html>
"""
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(html_content)


# ─── Main ─────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="Analyze a head-to-head match PGN between two engines."
    )
    parser.add_argument(
        "pgn_file",
        type=str,
        nargs="?",
        default="match.pgn",
        help="Path to the PGN file to analyze (default: match.pgn)",
    )
    parser.add_argument(
        "--engine1",
        type=str,
        default=None,
        help="Name of engine 1 (default: auto-detect — the engine with more wins, or alphabetically first)",
    )
    parser.add_argument(
        "--engine2",
        type=str,
        default=None,
        help="Name of engine 2 (default: auto-detect)",
    )
    parser.add_argument(
        "-o", "--html",
        type=str,
        default=None,
        help="Output filepath for an HTML dashboard report",
    )
    parser.add_argument(
        "-m", "--markdown",
        type=str,
        default=None,
        help="Output filepath for a clean Markdown report",
    )

    args = parser.parse_args()

    print_header("Head-to-Head Match Analyzer")

    if not os.path.exists(args.pgn_file):
        print(red(f"Error: PGN file '{args.pgn_file}' does not exist!"))
        sys.exit(1)

    print(f"Reading PGN: {cyan(args.pgn_file)} ...")
    games = parse_pgn(args.pgn_file)

    if not games:
        print(red("Error: No valid games found in PGN."))
        sys.exit(1)

    print(f"Parsed {green(str(len(games)))} games.")

    # Detect engines
    engines = detect_engines(games)

    if len(engines) < 2:
        print(red(f"Error: Need at least 2 players, found: {engines}"))
        sys.exit(1)

    if args.engine1 and args.engine2:
        e1, e2 = args.engine1, args.engine2
    elif args.engine1:
        e1 = args.engine1
        e2 = [e for e in engines if e != e1][0]
    else:
        # Auto-detect: pick engine with more wins as engine1 for a more natural display
        # (the "challenger" or "new" engine is typically listed first)
        e1, e2 = engines[0], engines[1]

        # Count wins for each to put the winner/stronger engine as engine1
        e1_wins_count = 0
        e2_wins_count = 0
        for g in games:
            if g["result"] == "1-0":
                if g["white"] == e1:
                    e1_wins_count += 1
                else:
                    e2_wins_count += 1
            elif g["result"] == "0-1":
                if g["black"] == e1:
                    e1_wins_count += 1
                else:
                    e2_wins_count += 1

        if e2_wins_count > e1_wins_count:
            e1, e2 = e2, e1

    print(f"Engine 1: {green(e1)}")
    print(f"Engine 2: {cyan(e2)}")

    # Compute stats
    stats, skipped = compute_match_stats(games, e1, e2)

    if stats["total_games"] == 0:
        print(red(f"Error: No games found between '{e1}' and '{e2}'."))
        sys.exit(1)

    if skipped > 0:
        print(gray(f"Skipped {skipped} games not involving both engines."))

    # Compute Elo difference
    elo_diff, se_elo, ci_low, ci_high, los = compute_elo_diff(
        stats["e1_wins"], stats["e1_draws"], stats["e1_losses"]
    )

    # Print console report
    print_console_report(stats, elo_diff, se_elo, ci_low, ci_high, los)

    # Generate HTML if requested
    if args.html:
        try:
            generate_html_report(args.html, stats, elo_diff, se_elo, ci_low, ci_high, los)
            print(f"=> {green('HTML Report Generated')}: {bold(args.html)}")
        except Exception as e:
            print(red(f"Error generating HTML report: {e}"))

    # Generate Markdown if requested
    if args.markdown:
        try:
            generate_markdown_report(args.markdown, stats, elo_diff, se_elo, ci_low, ci_high, los)
            print(f"=> {green('Markdown Report Generated')}: {bold(args.markdown)}")
        except Exception as e:
            print(red(f"Error generating Markdown report: {e}"))

    print()


if __name__ == "__main__":
    main()
