#!/usr/bin/env python3
import os
import sys
import re
import math
import argparse
import json
from datetime import datetime

# Term color helper functions
def color(text, code):
    return f"\033[{code}m{text}\033[0m"

def bold(text):
    return color(text, "1")

def green(text):
    return color(text, "32")

def red(text):
    return color(text, "31")

def yellow(text):
    return color(text, "33")

def blue(text):
    return color(text, "34")

def cyan(text):
    return color(text, "36")

def gray(text):
    return color(text, "90")

def print_header(title):
    print("=" * 80)
    print(f"   {bold(title.upper())}   ")
    print("=" * 80)

def parse_pgn(pgn_path):
    if not os.path.exists(pgn_path):
        return []
    
    with open(pgn_path, "r", encoding="utf-8") as f:
        content = f.read()

    # Split by PGN tag [Event to robustly parse separate games
    games_raw = re.split(r'\[Event\s+', content)
    results = []

    for game in games_raw:
        if not game.strip():
            continue
        
        # Parse standard PGN tags (handling Red/White/Black)
        white_match = re.search(r'\[(?:White|Red)\s+"([^"]+)"\]', game)
        black_match = re.search(r'\[Black\s+"([^"]+)"\]', game)
        result_match = re.search(r'\[Result\s+"([^"]+)"\]', game)
        round_match = re.search(r'\[Round\s+"([^"]+)"\]', game)
        date_match = re.search(r'\[Date\s+"([^"]+)"\]', game)
        tc_match = re.search(r'\[TimeControl\s+"([^"]+)"\]', game)

        if white_match and black_match and result_match:
            results.append({
                "white": white_match.group(1),
                "black": black_match.group(1),
                "result": result_match.group(1),
                "round": round_match.group(1) if round_match else "?",
                "date": date_match.group(1) if date_match else "?",
                "tc": tc_match.group(1) if tc_match else "unknown",
            })
    return results

def detect_bot_name(games):
    """Finds the most frequent player name in the tournament as the default bot."""
    player_counts = {}
    for g in games:
        for p in [g["white"], g["black"]]:
            player_counts[p] = player_counts.get(p, 0) + 1
    
    if not player_counts:
        return "Lingine"
    
    # Sort by frequency
    sorted_players = sorted(player_counts.items(), key=lambda x: x[1], reverse=True)
    return sorted_players[0][0]

def extract_elo_from_name(name, custom_map=None):
    """Tries to extract Elo rating from player name using digits, or uses a custom map."""
    if custom_map and name in custom_map:
        return custom_map[name]
    
    # Check if name contains a number (e.g. FS-1200 or Bot_1800)
    match = re.search(r'\d+', name)
    if match:
        return int(match.group(0))
    return None

def calculate_opponent_stats(games, bot_name, custom_map=None, default_opp_elo=1500, smoothing=False):
    stats = {}
    skipped_count = 0

    for game in games:
        w, b, res = game["white"], game["black"], game["result"]
        
        if w == bot_name:
            opponent = b
            bot_color = "white" # Red/White
        elif b == bot_name:
            opponent = w
            bot_color = "black"
        else:
            skipped_count += 1
            continue

        if opponent not in stats:
            opp_base_elo = extract_elo_from_name(opponent, custom_map)
            is_estimated_opp_elo = False
            if opp_base_elo is None:
                opp_base_elo = default_opp_elo
                is_estimated_opp_elo = True
                
            stats[opponent] = {
                "name": opponent,
                "opponent_elo": opp_base_elo,
                "is_estimated_opp_elo": is_estimated_opp_elo,
                "wins": 0,
                "draws": 0,
                "losses": 0,
                "games": 0,
                "bot_as_white_wins": 0,
                "bot_as_white_losses": 0,
                "bot_as_white_draws": 0,
                "bot_as_black_wins": 0,
                "bot_as_black_losses": 0,
                "bot_as_black_draws": 0
            }

        s = stats[opponent]
        s["games"] += 1

        if res == "1-0":
            if bot_color == "white":
                s["wins"] += 1
                s["bot_as_white_wins"] += 1
            else:
                s["losses"] += 1
                s["bot_as_black_losses"] += 1
        elif res == "0-1":
            if bot_color == "black":
                s["wins"] += 1
                s["bot_as_black_wins"] += 1
            else:
                s["losses"] += 1
                s["bot_as_white_losses"] += 1
        elif res in ["1/2-1/2", "0.5-0.5", "1/2"]:
            s["draws"] += 1
            if bot_color == "white":
                s["bot_as_white_draws"] += 1
            else:
                s["bot_as_black_draws"] += 1
        else:
            # Treat unknown as draws to be safe, or skip. Here we treat as draw
            s["draws"] += 1
            if bot_color == "white":
                s["bot_as_white_draws"] += 1
            else:
                s["bot_as_black_draws"] += 1

    # Post-process point stats and per-opponent Elo
    for opp, s in stats.items():
        wins, draws, losses, n = s["wins"], s["draws"], s["losses"], s["games"]
        
        # Laplace Smoothing: add virtual 0.5 win and 0.5 loss
        if smoothing:
            s["points"] = wins + 0.5 * draws + 0.5
            s["total_games_smoothed"] = n + 1
            s["score_pct"] = s["points"] / s["total_games_smoothed"]
        else:
            s["points"] = wins + 0.5 * draws
            s["total_games_smoothed"] = n
            s["score_pct"] = s["points"] / n if n > 0 else 0.5

        # Standard Elo Difference against this opponent
        score_pct = s["score_pct"]
        if score_pct >= 0.999:
            delta_elo = 400.0
        elif score_pct <= 0.001:
            delta_elo = -400.0
        else:
            delta_elo = 400.0 * math.log10(score_pct / (1.0 - score_pct))
            
        s["delta_elo"] = delta_elo
        s["estimated_elo"] = s["opponent_elo"] + delta_elo

    return stats, skipped_count

def estimate_global_elo(opponents_stats_list):
    """
    Computes global MLE Elo rating by solving: sum_i N_i * E(R, R_i) = TotalPoints
    where E(R, R_i) = 1 / (1 + 10^-((R - R_i)/400))
    Uses Newton-Raphson iteration.
    """
    total_games = sum(s["games"] for s in opponents_stats_list)
    total_points = sum(s["wins"] + 0.5 * s["draws"] for s in opponents_stats_list)
    
    if total_games == 0:
        return None, None, None
        
    # Edge Cases: All wins or All losses
    if total_points == 0:
        min_opp = min(s["opponent_elo"] for s in opponents_stats_list)
        return min_opp - 400.0, None, None
    if total_points == total_games:
        max_opp = max(s["opponent_elo"] for s in opponents_stats_list)
        return max_opp + 400.0, None, None
        
    # Start with weighted average of opponent Elos as guess
    r = sum(s["opponent_elo"] * s["games"] for s in opponents_stats_list) / total_games
    
    # Newton-Raphson iteration
    for _ in range(100):
        f_val = 0.0
        f_prime = 0.0
        for s in opponents_stats_list:
            opp_elo = s["opponent_elo"]
            n = s["games"]
            
            # expected score
            diff = (r - opp_elo) / 400.0
            if diff < -100:
                expected = 0.0
            elif diff > 100:
                expected = 1.0
            else:
                expected = 1.0 / (1.0 + 10.0**(-diff))
                
            f_val += n * expected
            f_prime += n * expected * (1.0 - expected) * (math.log(10.0) / 400.0)
            
        f_val -= total_points
        
        if abs(f_val) < 1e-6 or f_prime == 0:
            break
            
        r_new = r - f_val / f_prime
        # Cap change per step to ensure convergence stability
        if abs(r_new - r) > 100:
            r += 100.0 if r_new > r else -100.0
        else:
            r = r_new
            
    # Calculate Standard Error (SE) using Fisher Information
    sum_terms = 0.0
    for s in opponents_stats_list:
        opp_elo = s["opponent_elo"]
        n = s["games"]
        diff = (r - opp_elo) / 400.0
        if -100 <= diff <= 100:
            expected = 1.0 / (1.0 + 10.0**(-diff))
            sum_terms += n * expected * (1.0 - expected)
            
    se = 1.0 / ((math.log(10.0) / 400.0) * math.sqrt(sum_terms)) if sum_terms > 0 else None
    
    # 95% Confidence Interval
    ci = (r - 1.96 * se, r + 1.96 * se) if se else (None, None)
    
    return r, se, ci

def generate_html_report(filepath, bot_name, games_count, stats, global_elo, se, ci):
    """Generates a premium responsive HTML dashboard visual report."""
    now_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    
    # Overall summary stats
    tot_wins = sum(s["wins"] for s in stats.values())
    tot_draws = sum(s["draws"] for s in stats.values())
    tot_losses = sum(s["losses"] for s in stats.values())
    tot_games = sum(s["games"] for s in stats.values())
    tot_pts = tot_wins + 0.5 * tot_draws
    win_rate = (tot_pts / tot_games) * 100 if tot_games > 0 else 0
    
    ci_str = f"{int(round(ci[0]))} - {int(round(ci[1]))} ELO" if ci[0] else "N/A"
    se_str = f"± {int(round(se * 1.96))} ELO (95% CI)" if se else "N/A"
    
    # Generate opponent table rows
    table_rows = ""
    sorted_opponents = sorted(stats.values(), key=lambda x: x["opponent_elo"])
    
    for s in sorted_opponents:
        points = s["wins"] + 0.5 * s["draws"]
        raw_pct = (points / s["games"]) * 100 if s["games"] > 0 else 0
        diff_color = "#10b981" if s["delta_elo"] >= 0 else "#ef4444"
        diff_sign = "+" if s["delta_elo"] >= 0 else ""
        
        table_rows += f"""
        <tr>
            <td style="font-weight: 600;">{s["name"]}</td>
            <td><span class="badge badge-gray">{s["opponent_elo"]} ELO</span></td>
            <td style="font-weight: 500;">{s["games"]}</td>
            <td style="color: #10b981; font-weight: 600;">{s["wins"]}</td>
            <td style="color: #eab308; font-weight: 600;">{s["draws"]}</td>
            <td style="color: #ef4444; font-weight: 600;">{s["losses"]}</td>
            <td style="font-weight: 600;">{raw_pct:.1f}%</td>
            <td style="color: {diff_color}; font-weight: 700;">{diff_sign}{int(round(s["delta_elo"]))}</td>
            <td><span class="badge badge-accent" style="background-color: {diff_color}15; color: {diff_color}; border: 1px solid {diff_color}30;">{int(round(s["estimated_elo"]))} ELO</span></td>
        </tr>
        """
        
    # Generate charts & tables in beautiful glassmorphism-themed page
    html_content = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{bot_name} - ELO Performance Analysis</title>
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
                radial-gradient(at 50% 50%, rgba(99, 102, 241, 0.05) 0px, transparent 50%);
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
            margin-bottom: 0.75rem;
        }}

        .meta-tag {{
            display: inline-block;
            background: rgba(255, 255, 255, 0.04);
            border: 1px solid var(--card-border);
            padding: 0.35rem 1rem;
            border-radius: 9999px;
            font-size: 0.85rem;
            color: var(--text-muted);
            margin-top: 0.25rem;
        }}

        /* Grid layout */
        .dashboard-grid {{
            display: grid;
            grid-template-columns: repeat(4, 1fr);
            gap: 1.5rem;
            margin-bottom: 2rem;
        }}

        @media (max-width: 1024px) {{
            .dashboard-grid {{
                grid-template-columns: repeat(2, 1fr);
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

        .card-rating {{
            grid-column: span 2;
            background: radial-gradient(circle at 100% 0%, rgba(59, 130, 246, 0.12) 0%, transparent 70%), var(--card-bg);
            border-color: rgba(59, 130, 246, 0.2);
            box-shadow: 0 0 40px -10px var(--accent-glow);
        }}

        @media (max-width: 640px) {{
            .card-rating {{
                grid-column: span 1;
            }}
        }}

        .card-rating::before {{
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            width: 4px;
            height: 100%;
            background: linear-gradient(to bottom, var(--accent-color), #818cf8);
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

        .card-rating .card-value {{
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

        /* Table Card Section */
        .main-section {{
            margin-bottom: 2.5rem;
        }}

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

        .table-container {{
            width: 100%;
            overflow-x: auto;
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

        tr {{
            transition: background-color 0.2s ease;
        }}

        tr:hover td {{
            background-color: rgba(255, 255, 255, 0.015);
        }}

        /* Badges */
        .badge {{
            display: inline-block;
            padding: 0.2rem 0.65rem;
            border-radius: 6px;
            font-size: 0.8rem;
            font-weight: 600;
            text-align: center;
        }}

        .badge-gray {{
            background: rgba(255, 255, 255, 0.06);
            color: #d1d5db;
            border: 1px solid rgba(255, 255, 255, 0.05);
        }}

        /* Details / Explanation grid */
        .details-grid {{
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 1.5rem;
        }}

        @media (max-width: 768px) {{
            .details-grid {{
                grid-template-columns: 1fr;
            }}
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

        .rating-glow {{
            position: relative;
        }}
        
        .rating-glow::after {{
            content: 'ESTIMATED';
            position: absolute;
            top: -12px;
            right: 0;
            font-size: 0.6rem;
            font-weight: 800;
            letter-spacing: 0.1em;
            background: var(--accent-color);
            color: #ffffff;
            padding: 2px 6px;
            border-radius: 4px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <div class="header-pre">GAUNTLET PROFILE ANALYSIS</div>
            <h1>{bot_name} Performance Analysis</h1>
            <div class="meta-tag">Generated: {now_str} • Source PGN: gauntlet.pgn</div>
        </header>

        <!-- Dashboard Widgets -->
        <div class="dashboard-grid">
            <div class="card card-rating rating-glow">
                <div class="card-label">Global MLE Rating</div>
                <div class="card-value">{int(round(global_elo))} ELO</div>
                <div class="card-subtext">{se_str} | Interval: {ci_str}</div>
            </div>

            <div class="card">
                <div class="card-label">Overall Record</div>
                <div class="card-value" style="font-size: 1.85rem; margin-top: 0.25rem;">
                    {tot_wins}<span style="color: var(--text-muted); font-size: 1.2rem; font-weight: 400;">W</span> 
                    {tot_draws}<span style="color: var(--text-muted); font-size: 1.2rem; font-weight: 400;">D</span> 
                    {tot_losses}<span style="color: var(--text-muted); font-size: 1.2rem; font-weight: 400;">L</span>
                </div>
                <div class="card-subtext">Total Played: {tot_games} matches</div>
            </div>

            <div class="card">
                <div class="card-label">Win / Score Rate</div>
                <div class="card-value" style="color: var(--emerald);">{win_rate:.1f}%</div>
                <div class="card-subtext">Points: {tot_pts} / {tot_games}</div>
            </div>
        </div>

        <!-- Performance Table Section -->
        <div class="card main-section" style="padding: 0; overflow: hidden;">
            <div style="padding: 1.5rem 1.5rem 0.5rem 1.5rem;">
                <h2 class="section-title" style="margin-bottom: 0;">Performance vs. Specific Opponent ELO Targets</h2>
            </div>
            <div class="table-container">
                <table>
                    <thead>
                        <tr>
                            <th>Opponent Name</th>
                            <th>Target ELO</th>
                            <th>Matches</th>
                            <th>Wins</th>
                            <th>Draws</th>
                            <th>Losses</th>
                            <th>Score Rate</th>
                            <th>ELO Diff</th>
                            <th>Estimated Performance</th>
                        </tr>
                    </thead>
                    <tbody>
                        {table_rows}
                    </tbody>
                </table>
            </div>
        </div>

        <!-- Details Grid -->
        <div class="details-grid">
            <div class="card">
                <h2 class="section-title">Statistical methodology</h2>
                <ul class="bullet-list" style="margin-top: 0.5rem;">
                    <li><strong>Bradley-Terry Maximum Likelihood Estimation (MLE)</strong>: Rather than just averaging the ELO estimates against each opponent, we solve the global system of logistic equations for expected scores. This is the mathematically correct method used by international rating agencies like FIDE and USCF for rating calculations.</li>
                    <li><strong>Fisher Information Standard Error (SE)</strong>: The error bars are derived from the curvature of the log-likelihood function (second derivative). More games played shrink the standard error and widen our confidence in the rating.</li>
                    <li><strong>Standard ELO Difference</strong>: Based on the classic logistics curve formula:
                        <div class="formula-card">Δ_ELO = 400 * log10(Score% / (1 - Score%))</div>
                    </li>
                </ul>
            </div>

            <div class="card">
                <h2 class="section-title">Color Performance Insights</h2>
                <div style="margin-top: 0.5rem;">
                    <p style="margin-bottom: 1rem; color: var(--text-muted);">
                        Analyzing color performance provides details about potential biases in opening books, engine evaluations, or time management when playing Red vs. Black.
                    </p>
                    <table style="width: 100%; border: none;">
                        <thead>
                            <tr style="background: transparent;">
                                <th style="padding: 0.5rem 0; border: none;">Color</th>
                                <th style="padding: 0.5rem 0; border: none;">Wins</th>
                                <th style="padding: 0.5rem 0; border: none;">Draws</th>
                                <th style="padding: 0.5rem 0; border: none;">Losses</th>
                                <th style="padding: 0.5rem 0; border: none;">Score %</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr>
                                <td style="border: none; padding: 0.5rem 0; font-weight: 600; color: var(--rose);">Red (First)</td>
                                <td style="border: none; padding: 0.5rem 0; font-weight: 600; color: var(--emerald);">{sum(s["bot_as_white_wins"] for s in stats.values())}</td>
                                <td style="border: none; padding: 0.5rem 0; font-weight: 600; color: var(--amber);">{sum(s["bot_as_white_draws"] for s in stats.values())}</td>
                                <td style="border: none; padding: 0.5rem 0; font-weight: 600; color: var(--rose);">{sum(s["bot_as_white_losses"] for s in stats.values())}</td>
                                <td style="border: none; padding: 0.5rem 0; font-weight: 600;">
                                    {((sum(s["bot_as_white_wins"] for s in stats.values()) + 0.5*sum(s["bot_as_white_draws"] for s in stats.values())) / max(1, sum(s["bot_as_white_wins"]+s["bot_as_white_draws"]+s["bot_as_white_losses"] for s in stats.values())) * 100):.1f}%
                                </td>
                            </tr>
                            <tr>
                                <td style="border: none; padding: 0.5rem 0; font-weight: 600; color: #6b7280;">Black (Second)</td>
                                <td style="border: none; padding: 0.5rem 0; font-weight: 600; color: var(--emerald);">{sum(s["bot_as_black_wins"] for s in stats.values())}</td>
                                <td style="border: none; padding: 0.5rem 0; font-weight: 600; color: var(--amber);">{sum(s["bot_as_black_draws"] for s in stats.values())}</td>
                                <td style="border: none; padding: 0.5rem 0; font-weight: 600; color: var(--rose);">{sum(s["bot_as_black_losses"] for s in stats.values())}</td>
                                <td style="border: none; padding: 0.5rem 0; font-weight: 600;">
                                    {((sum(s["bot_as_black_wins"] for s in stats.values()) + 0.5*sum(s["bot_as_black_draws"] for s in stats.values())) / max(1, sum(s["bot_as_black_wins"]+s["bot_as_black_draws"]+s["bot_as_black_losses"] for s in stats.values())) * 100):.1f}%
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>

        <footer>
            Created by Antigravity AI Coding Assistant • DeepMind Team
        </footer>
    </div>
</body>
</html>
"""
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(html_content)

def main():
    parser = argparse.ArgumentParser(
        description="Comprehensive PGN Gauntlet Result Analyzer and Elo Estimator."
    )
    parser.add_argument(
        "pgn_file",
        type=str,
        nargs="?",
        default="gauntlet.pgn",
        help="Path to the PGN file to analyze (default: gauntlet.pgn)",
    )
    parser.add_argument(
        "-b",
        "--bot",
        type=str,
        default=None,
        help="Name of our bot in PGN (default: auto-detect)",
    )
    parser.add_argument(
        "-s",
        "--smoothing",
        action="store_true",
        help="Apply Laplace smoothing (+0.5 Win/+0.5 Loss) to handle small sample sizes & extreme scores",
    )
    parser.add_argument(
        "--default-opp-elo",
        type=int,
        default=1500,
        help="Default rating for opponents whose name doesn't contain a number (default: 1500)",
    )
    parser.add_argument(
        "-m",
        "--elo-map",
        type=str,
        default=None,
        help="JSON string map for custom opponent ratings (e.g. '{\"Pikafish\":2500}')",
    )
    parser.add_argument(
        "-o",
        "--html",
        type=str,
        default=None,
        help="Output filepath to generate a beautiful responsive HTML report dashboard",
    )

    args = parser.parse_args()

    print_header("PGN Tournament Gauntlet & ELO Estimator")

    if not os.path.exists(args.pgn_file):
        print(red(f"Error: PGN file '{args.pgn_file}' does not exist!"))
        print("Please run a tournament first or supply a valid path.")
        sys.exit(1)

    print(f"Reading and parsing PGN file: {cyan(args.pgn_file)} ...")
    games = parse_pgn(args.pgn_file)
    
    if not games:
        print(red("Error: No valid games parsed from PGN. Verify file structure."))
        sys.exit(1)
        
    print(f"Successfully parsed {green(len(games))} games from tournament.")

    # Determine bot name
    bot_name = args.bot
    if not bot_name:
        bot_name = detect_bot_name(games)
        print(f"Auto-detected bot name in PGN: {green(bot_name)}")
    else:
        print(f"Using user-specified bot name: {green(bot_name)}")

    # Parse custom ELO map if provided
    custom_map = {}
    if args.elo_map:
        try:
            custom_map = json.loads(args.elo_map)
            print(f"Loaded custom opponent ELO overrides: {gray(custom_map)}")
        except Exception as e:
            print(red(f"Error parsing --elo-map JSON: {e}"))
            sys.exit(1)

    # Process stats
    stats, skipped = calculate_opponent_stats(
        games, 
        bot_name, 
        custom_map=custom_map, 
        default_opp_elo=args.default_opp_elo,
        smoothing=args.smoothing
    )

    if not stats:
        print(red(f"Error: No games found where bot '{bot_name}' played!"))
        print(f"Available players in PGN: {gray(list(set(g['white'] for g in games) | set(g['black'] for g in games)))}")
        sys.exit(1)

    if skipped > 0:
        print(gray(f"Skipped {skipped} games where '{bot_name}' did not participate."))

    # Perform calculations
    opponents_list = list(stats.values())
    global_elo, se, ci = estimate_global_elo(opponents_list)

    # Console display formatting
    print("\n" + "=" * 85)
    print(f"           {bold('PERFORMANCE SUMMARY FOR ' + bot_name.upper())}             ")
    print("=" * 85)
    print(f"{'Opponent':<15}{'Base ELO':<10}{'Games':<8}{'Wins':<7}{'Draws':<7}{'Losses':<7}{'Score %':<10}{'ELO Diff':<11}{'Est. ELO':<10}")
    print("-" * 85)

    tot_wins = 0
    tot_draws = 0
    tot_losses = 0
    tot_games = 0

    # Sort opponents by Elo
    sorted_opponents = sorted(opponents_list, key=lambda x: x["opponent_elo"])

    for s in sorted_opponents:
        wins, draws, losses, n = s["wins"], s["draws"], s["losses"], s["games"]
        tot_wins += wins
        tot_draws += draws
        tot_losses += losses
        tot_games += n
        
        points = wins + 0.5 * draws
        raw_pct = (points / n) * 100 if n > 0 else 0
        
        diff_val = int(round(s["delta_elo"]))
        diff_str = f"+{diff_val}" if diff_val >= 0 else f"{diff_val}"
        diff_color_func = green if diff_val >= 0 else red
        
        est_elo_val = int(round(s["estimated_elo"]))
        
        pct_str = f"{raw_pct:.1f}%"
        opp_rating_str = f"{s['opponent_elo']}"
        if s["is_estimated_opp_elo"]:
            opp_rating_str += "*"

        print(
            f"{s['name']:<15}"
            f"{opp_rating_str:<10}"
            f"{n:<8}"
            f"{wins:<7}"
            f"{draws:<7}"
            f"{losses:<7}"
            f"{pct_str:<10}"
            f"{diff_color_func(f'{diff_str:<11}')}"
            f"{bold(f'{est_elo_val:<10}')}"
        )

    print("-" * 85)
    total_pts = tot_wins + 0.5 * tot_draws
    overall_win_rate = (total_pts / tot_games) * 100 if tot_games > 0 else 0
    
    print(
        f"{bold('OVERALL TOTAL'):<15}"
        f"{'-':<10}"
        f"{tot_games:<8}"
        f"{tot_wins:<7}"
        f"{tot_draws:<7}"
        f"{tot_losses:<7}"
        f"{overall_win_rate:.1f}%"
        f"{'-':<11}"
        f"{'-':<10}"
    )
    print("=" * 85)

    # Global ratings display
    if global_elo is not None:
        print(f"\n=> {bold('GLOBAL MLE ELO RATING')}: {green(f'{int(round(global_elo))} ELO')}")
        if se is not None and ci[0] is not None:
            print(f"   -> Standard Error: {cyan(f'± {se:.1f} ELO')}")
            print(f"   -> 95% Confidence Interval: {cyan(f'{int(round(ci[0]))} - {int(round(ci[1]))} ELO')}")
            print(gray("      (Reflects statistical uncertainty based on match volume & scores)"))
        else:
            print(yellow("   -> Standard error / Confidence Interval could not be calculated (perfect or extreme score)."))
    else:
        print(red("\nCould not estimate global ELO due to invalid stats."))

    # Copy-pasteable Markdown table for release
    print("\n" + "=" * 85)
    print(f"      {bold('COPY-PASTEABLE MARKDOWN TABLE FOR RELEASE')}      ")
    print("=" * 85)
    print("| Opponent | Base ELO | Games | Wins | Draws | Losses | Score % | ELO Diff | Est. Performance |")
    print("| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |")
    for s in sorted_opponents:
        wins, draws, losses, n = s["wins"], s["draws"], s["losses"], s["games"]
        points = wins + 0.5 * draws
        raw_pct = (points / n) * 100 if n > 0 else 0
        diff_val = int(round(s["delta_elo"]))
        diff_str = f"+{diff_val}" if diff_val >= 0 else f"{diff_val}"
        est_elo_val = int(round(s["estimated_elo"]))
        print(f"| {s['name']} | {s['opponent_elo']} | {n} | {wins} | {draws} | {losses} | {raw_pct:.1f}% | {diff_str} | **{est_elo_val} ELO** |")
    print(f"| **OVERALL TOTAL** | — | **{tot_games}** | **{tot_wins}** | **{tot_draws}** | **{tot_losses}** | **{overall_win_rate:.1f}%** | — | **{int(round(global_elo))} ELO** |")
    print("=" * 85)

    # Print notes
    print("\n" + gray("Notes:"))
    print(gray(" * Base ELOs ending in '*' are fallbacks because no digits were found in their name."))
    print(gray(" * Global MLE uses the FIDE / Bradley-Terry standard system to optimize ratings globally."))
    if args.smoothing:
        print(gray(" * Laplace smoothing is ENABLED (+0.5 wins/losses applied to each opponent's pool)."))
    else:
        print(gray(" * Run with '-s' or '--smoothing' to handle infinity bounds on 100% or 0% score pools."))

    # Generate HTML if requested
    if args.html:
        try:
            generate_html_report(args.html, bot_name, len(games), stats, global_elo, se, ci)
            print(f"\n=> {green('Beautiful HTML Report Generated')}: {bold(args.html)}")
        except Exception as e:
            print(red(f"Error generating HTML report: {e}"))

    print()

if __name__ == "__main__":
    main()
