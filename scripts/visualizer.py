#!/usr/bin/env python3
"""
Xiangqi Bitboard Visualizer
---------------------------
A dual-mode visualizer for 9x10 Xiangqi bitboards.

Usage:
  1. Web GUI mode (interactive):
     python scripts/visualizer.py
     (Opens a modern interactive browser interface at http://localhost:8080)

  2. CLI mode (print a value directly to terminal):
     python scripts/visualizer.py <value>
     (Where <value> can be decimal, hex like 0x70381C0000000000E07038, or binary like 0b1010)
"""

import sys
import http.server
import socketserver
import webbrowser
import threading
import time
import re

PORT = 8080

# The embedded, single-file HTML/CSS/JS page.
# Styled with a modern glassmorphic dark theme, rich colors, and micro-animations.
HTML_CONTENT = """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Xiangqi Bitboard Visualizer</title>
    <!-- Import Google Fonts -->
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;500;600;700&family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet">
    
    <style>
        :root {
            --bg-gradient: radial-gradient(circle at 50% 0%, #1e1b4b 0%, #090d16 100%);
            --panel-bg: rgba(15, 23, 42, 0.65);
            --panel-border: rgba(255, 255, 255, 0.08);
            --primary: #10b981; /* Emerald */
            --primary-glow: rgba(16, 185, 129, 0.4);
            --primary-hover: #34d399;
            --palace-border: rgba(239, 68, 68, 0.5); /* Rose / Red */
            --palace-bg: rgba(239, 68, 68, 0.04);
            --text-main: #f8fafc;
            --text-muted: #94a3b8;
            --cell-size: 50px;
        }

        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }

        body {
            font-family: 'Outfit', sans-serif;
            background: var(--bg-gradient);
            color: var(--text-main);
            min-height: 100vh;
            display: flex;
            flex-direction: column;
            align-items: center;
            padding: 2rem 1rem;
            overflow-x: hidden;
        }

        header {
            margin-bottom: 2rem;
            text-align: center;
        }

        h1 {
            font-size: 2.2rem;
            font-weight: 700;
            letter-spacing: -0.05em;
            background: linear-gradient(135deg, #38bdf8 0%, #10b981 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            margin-bottom: 0.5rem;
        }

        p.subtitle {
            color: var(--text-muted);
            font-size: 1rem;
        }

        .main-container {
            display: flex;
            gap: 2rem;
            max-width: 1200px;
            width: 100%;
            justify-content: center;
            align-items: flex-start;
            flex-wrap: wrap;
        }

        /* Control Panel Styles */
        .control-panel {
            background: var(--panel-bg);
            backdrop-filter: blur(16px);
            border: 1px solid var(--panel-border);
            border-radius: 20px;
            padding: 1.5rem;
            width: 100%;
            max-width: 480px;
            display: flex;
            flex-direction: column;
            gap: 1.5rem;
            box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
        }

        .section-title {
            font-size: 1.1rem;
            font-weight: 600;
            color: #38bdf8;
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
            padding-bottom: 0.5rem;
            margin-bottom: 0.75rem;
            display: flex;
            align-items: center;
            justify-content: space-between;
        }

        .input-group {
            display: flex;
            flex-direction: column;
            gap: 0.5rem;
        }

        label {
            font-size: 0.85rem;
            font-weight: 500;
            color: var(--text-muted);
        }

        .input-with-button {
            display: flex;
            gap: 0.5rem;
        }

        input[type="text"] {
            flex-grow: 1;
            background: rgba(0, 0, 0, 0.3);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 8px;
            color: #fff;
            padding: 0.6rem 0.8rem;
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.9rem;
            transition: all 0.2s;
        }

        input[type="text"]:focus {
            outline: none;
            border-color: var(--primary);
            box-shadow: 0 0 8px var(--primary-glow);
        }

        .btn {
            background: rgba(255, 255, 255, 0.08);
            border: 1px solid rgba(255, 255, 255, 0.1);
            color: var(--text-main);
            padding: 0.6rem 1rem;
            border-radius: 8px;
            font-weight: 500;
            font-size: 0.85rem;
            cursor: pointer;
            transition: all 0.2s;
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 0.4rem;
        }

        .btn:hover {
            background: rgba(255, 255, 255, 0.15);
            border-color: rgba(255, 255, 255, 0.25);
            transform: translateY(-1px);
        }

        .btn:active {
            transform: translateY(1px);
        }

        .btn-primary {
            background: var(--primary);
            border-color: var(--primary);
            color: #0f172a;
            font-weight: 600;
        }

        .btn-primary:hover {
            background: var(--primary-hover);
            border-color: var(--primary-hover);
            box-shadow: 0 0 12px var(--primary-glow);
        }

        .preset-grid {
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 0.5rem;
        }

        .operation-grid {
            display: grid;
            grid-template-columns: repeat(3, 1fr);
            gap: 0.5rem;
        }

        /* Board Container & Grid */
        .board-card {
            background: var(--panel-bg);
            backdrop-filter: blur(16px);
            border: 1px solid var(--panel-border);
            border-radius: 20px;
            padding: 1.5rem;
            box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
            display: flex;
            flex-direction: column;
            align-items: center;
        }

        .board-grid {
            display: grid;
            /* 11 columns: Rank (Left) + 9 Files + Rank (Right) */
            grid-template-columns: 30px repeat(9, var(--cell-size)) 30px;
            /* 12 rows: File (Top) + 5 Ranks + River + 5 Ranks + File (Bottom) */
            grid-template-rows: 30px repeat(5, var(--cell-size)) 35px repeat(5, var(--cell-size)) 30px;
            gap: 4px;
            align-items: center;
            justify-items: center;
            user-select: none;
        }

        /* Label styling */
        .label {
            color: var(--text-muted);
            font-weight: 600;
            font-size: 0.9rem;
            display: flex;
            align-items: center;
            justify-content: center;
            width: 100%;
            height: 100%;
        }

        .label.file {
            font-family: 'JetBrains Mono', monospace;
            text-transform: uppercase;
        }

        /* River styling */
        .river-row {
            grid-column: span 11;
            width: 100%;
            height: 100%;
            background: rgba(30, 41, 59, 0.35);
            border-top: 1px solid rgba(255, 255, 255, 0.08);
            border-bottom: 1px solid rgba(255, 255, 255, 0.08);
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 0.85rem;
            font-weight: 600;
            color: rgba(255, 255, 255, 0.35);
            letter-spacing: 0.5em;
            text-shadow: 0 0 8px rgba(255, 255, 255, 0.1);
        }

        /* Cell styling */
        .cell {
            width: var(--cell-size);
            height: var(--cell-size);
            background: rgba(30, 41, 59, 0.4);
            border: 1px solid rgba(255, 255, 255, 0.04);
            border-radius: 8px;
            cursor: pointer;
            position: relative;
            transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
            display: flex;
            align-items: center;
            justify-content: center;
        }

        .cell:hover {
            background: rgba(255, 255, 255, 0.08);
            border-color: rgba(255, 255, 255, 0.15);
            transform: scale(1.05);
            z-index: 10;
        }

        /* Palace Cell marking */
        .cell.palace {
            border: 1.5px dashed var(--palace-border);
            background: var(--palace-bg);
        }

        /* Active Cell state */
        .cell.active {
            background: radial-gradient(circle at 35% 35%, #34d399 0%, #059669 100%);
            border-color: #34d399;
            box-shadow: 0 0 15px var(--primary-glow);
            transform: scale(0.98);
        }

        .cell.active:hover {
            box-shadow: 0 0 20px rgba(16, 185, 129, 0.7);
            transform: scale(1.03);
        }

        /* Cell inner visual details */
        .cell::after {
            content: '';
            width: 8px;
            height: 8px;
            border-radius: 50%;
            background: rgba(255, 255, 255, 0.15);
            transition: all 0.2s;
        }

        .cell.active::after {
            background: #fff;
            width: 12px;
            height: 12px;
            box-shadow: 0 0 8px rgba(255, 255, 255, 0.8);
        }

        .cell.palace::after {
            background: rgba(239, 68, 68, 0.25);
        }

        /* Square tooltip / Info panel */
        .hover-info {
            background: rgba(0, 0, 0, 0.35);
            border-radius: 12px;
            padding: 0.75rem 1rem;
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 0.5rem;
            font-size: 0.85rem;
        }

        .info-item {
            display: flex;
            flex-direction: column;
        }

        .info-val {
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.95rem;
            font-weight: 600;
            color: #38bdf8;
            margin-top: 0.15rem;
        }

        .footer {
            margin-top: 3rem;
            color: var(--text-muted);
            font-size: 0.8rem;
            text-align: center;
        }
    </style>
</head>
<body>

    <header>
        <h1>Xiangqi Bitboard Visualizer</h1>
        <p class="subtitle">Interactive 9x10 Bitboard representation helper for Rust engine development</p>
    </header>

    <div class="main-container">
        <!-- Left Side: Controls -->
        <div class="control-panel">
            <!-- Inputs -->
            <div class="input-group">
                <div class="section-title">Values</div>
                
                <div class="input-group" style="margin-bottom: 0.75rem;">
                    <label for="val-hex">Hexadecimal (u128)</label>
                    <input type="text" id="val-hex" value="0x0" placeholder="0x...">
                </div>

                <div class="input-group" style="margin-bottom: 0.75rem;">
                    <label for="val-dec">Decimal</label>
                    <input type="text" id="val-dec" value="0" placeholder="Decimal value">
                </div>

                <div class="input-group" style="margin-bottom: 0.75rem;">
                    <label for="val-rust">Rust Bitboard Code</label>
                    <div class="input-with-button">
                        <input type="text" id="val-rust" readonly value="Bitboard::from_raw(0x0)">
                        <button class="btn btn-primary" id="btn-copy">Copy</button>
                    </div>
                </div>

                <div class="input-group">
                    <label for="val-bin">Binary (grouped by ranks from top 9 down to bottom 0)</label>
                    <input type="text" id="val-bin" value="0" placeholder="Binary string">
                </div>
            </div>

            <!-- Operations -->
            <div>
                <div class="section-title">Bit Operations</div>
                <div class="operation-grid">
                    <button class="btn" id="op-clear">Clear All</button>
                    <button class="btn" id="op-fill">Set All</button>
                    <button class="btn" id="op-invert">Invert</button>
                    <button class="btn" id="op-up">Shift Up</button>
                    <button class="btn" id="op-down">Shift Down</button>
                    <button class="btn" id="op-left">Shift Left</button>
                    <button class="btn" id="op-right" style="grid-column: span 3;">Shift Right</button>
                </div>
            </div>

            <!-- Presets -->
            <div>
                <div class="section-title">Common Masks & Presets</div>
                <div class="preset-grid">
                    <button class="btn" id="preset-palace">Palace Zone</button>
                    <button class="btn" id="preset-pawns">Pawn Files</button>
                    <button class="btn" id="preset-white-side">White Side (R0-R4)</button>
                    <button class="btn" id="preset-black-side">Black Side (R5-R9)</button>
                    <button class="btn" id="preset-white-pawns">White Pawn Range</button>
                    <button class="btn" id="preset-black-pawns">Black Pawn Range</button>
                </div>
            </div>

            <!-- Hover Information -->
            <div>
                <div class="section-title">Square Inspector</div>
                <div class="hover-info">
                    <div class="info-item">
                        <label>Square</label>
                        <div class="info-val" id="info-square">-</div>
                    </div>
                    <div class="info-item">
                        <label>Bit Index</label>
                        <div class="info-val" id="info-index">-</div>
                    </div>
                    <div class="info-item">
                        <label>Rank Index</label>
                        <div class="info-val" id="info-rank">-</div>
                    </div>
                    <div class="info-item">
                        <label>File Index</label>
                        <div class="info-val" id="info-file">-</div>
                    </div>
                </div>
            </div>
        </div>

        <!-- Right Side: The Grid -->
        <div class="board-card">
            <div class="board-grid" id="board-grid">
                <!-- Top File Labels -->
                <div class="label"></div>
                <div class="label file">a</div>
                <div class="label file">b</div>
                <div class="label file">c</div>
                <div class="label file">d</div>
                <div class="label file">e</div>
                <div class="label file">f</div>
                <div class="label file">g</div>
                <div class="label file">h</div>
                <div class="label file">i</div>
                <div class="label"></div>

                <!-- Ranks 9 down to 5 dynamically generated in JS -->
                <!-- River dynamically generated in JS -->
                <!-- Ranks 4 down to 0 dynamically generated in JS -->
                
                <!-- Bottom File Labels populated in JS -->
            </div>
        </div>
    </div>

    <div class="footer">
        Lingine Chess Engine Project Tools &middot; Xiangqi Bitboard visualizer
    </div>

    <script>
        // Use BigInt since u128 exceeds JS safe integer limit (2^53 - 1)
        let bitboardVal = 0n;
        const MASK_90 = (1n << 90n) - 1n;

        // Constants and precomputed masks
        const FILES = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i'];

        // Masks computed on startup
        let FILE_A_MASK = 0n;
        let FILE_I_MASK = 0n;
        let PALACE_MASK = 0n;
        let PAWN_FILES_MASK = 0n;
        let WHITE_SIDE_MASK = 0n;
        let BLACK_SIDE_MASK = 0n;
        let WHITE_PAWNS_MASK = 0n;
        let BLACK_PAWNS_MASK = 0n;

        function initMasks() {
            // Compute File A and File I masks
            for (let r = 0n; r < 10n; r++) {
                FILE_A_MASK |= (1n << (r * 9n + 0n));
                FILE_I_MASK |= (1n << (r * 9n + 8n));
            }

            // Compute Palace mask
            for (let r = 0n; r < 10n; r++) {
                for (let f = 0n; f < 9n; f++) {
                    if ((r <= 2n || r >= 7n) && (f >= 3n && f <= 5n)) {
                        PALACE_MASK |= (1n << (r * 9n + f));
                    }
                }
            }

            // Compute Pawn Files mask (files A, C, E, G, I => 0, 2, 4, 6, 8)
            for (let r = 0n; r < 10n; r++) {
                for (let f of [0n, 2n, 4n, 6n, 8n]) {
                    PAWN_FILES_MASK |= (1n << (r * 9n + f));
                }
            }

            // Sides masks
            for (let idx = 0n; idx < 90n; idx++) {
                if (idx < 45n) {
                    WHITE_SIDE_MASK |= (1n << idx);
                } else {
                    BLACK_SIDE_MASK |= (1n << idx);
                }
            }

            // Pawn starting and valid ranges (prior to river and after)
            // White Pawns: Files A, C, E, G, I on Rank 3 and 4, and ALL files on Black side (Ranks 5-9)
            let whitePawnMySide = 0n;
            for (let r of [3n, 4n]) {
                for (let f of [0n, 2n, 4n, 6n, 8n]) {
                    whitePawnMySide |= (1n << (r * 9n + f));
                }
            }
            WHITE_PAWNS_MASK = whitePawnMySide | BLACK_SIDE_MASK;

            // Black Pawns: Files A, C, E, G, I on Rank 5 and 6, and ALL files on White side (Ranks 0-4)
            let blackPawnMySide = 0n;
            for (let r of [5n, 6n]) {
                for (let f of [0n, 2n, 4n, 6n, 8n]) {
                    blackPawnMySide |= (1n << (r * 9n + f));
                }
            }
            BLACK_PAWNS_MASK = blackPawnMySide | WHITE_SIDE_MASK;
        }

        // Check if square is in Palace
        function isPalaceSquare(fileIdx, rankIdx) {
            return (rankIdx <= 2 || rankIdx >= 7) && (fileIdx >= 3 && fileIdx <= 5);
        }

        // Build grid layout
        const grid = document.getElementById('board-grid');

        function generateBoardDOM() {
            // We need to insert rows in decreasing rank order (9 down to 0)
            for (let r = 9; r >= 0; r--) {
                // If it is the river position (between rank 5 and 4)
                if (r === 4) {
                    const river = document.createElement('div');
                    river.className = 'river-row';
                    river.innerHTML = '楚 河 &nbsp; &nbsp; &nbsp; &nbsp; &nbsp; &nbsp; &nbsp; &nbsp; &nbsp; 漢 界';
                    grid.appendChild(river);
                }

                // Left Rank Label
                const rankLabelLeft = document.createElement('div');
                rankLabelLeft.className = 'label';
                rankLabelLeft.textContent = r;
                grid.appendChild(rankLabelLeft);

                // 9 Files (a-i)
                for (let f = 0; f < 9; f++) {
                    const idx = r * 9 + f;
                    const cell = document.createElement('div');
                    cell.className = 'cell';
                    cell.dataset.index = idx;
                    cell.dataset.square = FILES[f] + r;
                    cell.dataset.rank = r;
                    cell.dataset.file = f;

                    if (isPalaceSquare(f, r)) {
                        cell.classList.add('palace');
                    }

                    // Add hover events
                    cell.addEventListener('mouseenter', () => updateInspector(cell));
                    cell.addEventListener('mouseleave', () => clearInspector());

                    // Add click toggle event
                    cell.addEventListener('click', () => {
                        toggleBit(idx);
                    });

                    grid.appendChild(cell);
                }

                // Right Rank Label
                const rankLabelRight = document.createElement('div');
                rankLabelRight.className = 'label';
                rankLabelRight.textContent = r;
                grid.appendChild(rankLabelRight);
            }

            // Bottom File Labels
            const footerEmptyLeft = document.createElement('div');
            footerEmptyLeft.className = 'label';
            grid.appendChild(footerEmptyLeft);

            for (let f = 0; f < 9; f++) {
                const label = document.createElement('div');
                label.className = 'label file';
                label.textContent = FILES[f];
                grid.appendChild(label);
            }

            const footerEmptyRight = document.createElement('div');
            footerEmptyRight.className = 'label';
            grid.appendChild(footerEmptyRight);
        }

        // Inspectors
        function updateInspector(cell) {
            document.getElementById('info-square').textContent = cell.dataset.square.toUpperCase();
            document.getElementById('info-index').textContent = cell.dataset.index;
            document.getElementById('info-rank').textContent = cell.dataset.rank;
            document.getElementById('info-file').textContent = cell.dataset.file;
        }

        function clearInspector() {
            document.getElementById('info-square').textContent = '-';
            document.getElementById('info-index').textContent = '-';
            document.getElementById('info-rank').textContent = '-';
            document.getElementById('info-file').textContent = '-';
        }

        // Parse user inputs of varying formats safely (handles 0x, 0b, plain digits, Rust wrappers)
        function parseBitboardValue(str) {
            str = str.replace(/u128/gi, '');
            str = str.replace(/_/g, '');
            str = str.trim();
            
            // Hex format
            const hexMatch = str.match(/0x([0-9a-fA-F]+)/);
            if (hexMatch) {
                try {
                    return BigInt('0x' + hexMatch[1]) & MASK_90;
                } catch(e) {}
            }
            
            // Binary format
            const binMatch = str.match(/0b([01\\s]+)/);
            if (binMatch) {
                try {
                    const cleanedBin = binMatch[1].replace(/\\s/g, '');
                    return BigInt('0b' + cleanedBin) & MASK_90;
                } catch(e) {}
            }
            
            // Raw digits inside a wrapper or plain digits
            const numMatch = str.match(/\\d+/);
            if (numMatch) {
                try {
                    return BigInt(numMatch[0]) & MASK_90;
                } catch(e) {}
            }
            
            return 0n;
        }

        // Format binary output in groups of 9 bits (representing the ranks)
        function formatBinary(val) {
            let binStr = val.toString(2).padStart(90, '0');
            let groups = [];
            // Slice into 10 groups of 9 bits (from top Rank 9 to bottom Rank 0)
            for (let i = 0; i < 10; i++) {
                groups.push(binStr.slice(i * 9, (i + 1) * 9));
            }
            return groups.join(' ');
        }

        // Format Hex with underscores every 4 digits for better readability
        function formatHexWithUnderscores(val) {
            let hex = val.toString(16);
            // Pad hex representation to a reasonable length if needed, or leave raw
            let reversed = hex.split('').reverse().join('');
            let chunks = reversed.match(/.{1,4}/g) || [];
            let formatted = chunks.join('_').split('').reverse().join('');
            return '0x' + formatted;
        }

        // Synchronize board UI state with code inputs
        function syncUI() {
            // Update inputs
            document.getElementById('val-hex').value = formatHexWithUnderscores(bitboardVal);
            document.getElementById('val-dec').value = bitboardVal.toString();
            document.getElementById('val-bin').value = formatBinary(bitboardVal);
            document.getElementById('val-rust').value = `unsafe { Bitboard::from_raw(${formatHexWithUnderscores(bitboardVal)}u128) }`;

            // Update board active/inactive classes
            const cells = document.querySelectorAll('.cell');
            cells.forEach(cell => {
                const idx = BigInt(cell.dataset.index);
                const bitSet = (bitboardVal & (1n << idx)) !== 0n;
                if (bitSet) {
                    cell.classList.add('active');
                } else {
                    cell.classList.remove('active');
                }
            });
        }

        // Value manipulations
        function toggleBit(idx) {
            bitboardVal ^= (1n << BigInt(idx));
            syncUI();
        }

        function setBitboardValue(newVal) {
            bitboardVal = newVal & MASK_90;
            syncUI();
        }

        // Initialize event handlers
        function initEvents() {
            // Inputs
            document.getElementById('val-hex').addEventListener('input', (e) => {
                setBitboardValue(parseBitboardValue(e.target.value));
            });
            document.getElementById('val-dec').addEventListener('input', (e) => {
                setBitboardValue(parseBitboardValue(e.target.value));
            });
            document.getElementById('val-bin').addEventListener('input', (e) => {
                setBitboardValue(parseBitboardValue(e.target.value));
            });

            // Clipboard copy
            document.getElementById('btn-copy').addEventListener('click', () => {
                const rustVal = document.getElementById('val-rust').value;
                navigator.clipboard.writeText(rustVal).then(() => {
                    const btn = document.getElementById('btn-copy');
                    const origText = btn.textContent;
                    btn.textContent = 'Copied!';
                    btn.style.background = '#059669';
                    setTimeout(() => {
                        btn.textContent = origText;
                        btn.style.background = '';
                    }, 1500);
                });
            });

            // Operations
            document.getElementById('op-clear').addEventListener('click', () => setBitboardValue(0n));
            document.getElementById('op-fill').addEventListener('click', () => setBitboardValue(MASK_90));
            document.getElementById('op-invert').addEventListener('click', () => setBitboardValue(~bitboardVal & MASK_90));
            
            // Shift operations
            document.getElementById('op-up').addEventListener('click', () => {
                setBitboardValue((bitboardVal << 9n) & MASK_90);
            });
            document.getElementById('op-down').addEventListener('click', () => {
                setBitboardValue(bitboardVal >> 9n);
            });
            document.getElementById('op-left').addEventListener('click', () => {
                // To shift left, clear bits in File A first, then shift down by 1 bit (decreasing index)
                setBitboardValue((bitboardVal & ~FILE_A_MASK) >> 1n);
            });
            document.getElementById('op-right').addEventListener('click', () => {
                // To shift right, clear bits in File I first, then shift up by 1 bit (increasing index)
                setBitboardValue((bitboardVal & ~FILE_I_MASK) << 1n);
            });

            // Presets
            document.getElementById('preset-palace').addEventListener('click', () => setBitboardValue(PALACE_MASK));
            document.getElementById('preset-pawns').addEventListener('click', () => setBitboardValue(PAWN_FILES_MASK));
            document.getElementById('preset-white-side').addEventListener('click', () => setBitboardValue(WHITE_SIDE_MASK));
            document.getElementById('preset-black-side').addEventListener('click', () => setBitboardValue(BLACK_SIDE_MASK));
            document.getElementById('preset-white-pawns').addEventListener('click', () => setBitboardValue(WHITE_PAWNS_MASK));
            document.getElementById('preset-black-pawns').addEventListener('click', () => setBitboardValue(BLACK_PAWNS_MASK));
        }

        // Start
        initMasks();
        generateBoardDOM();
        initEvents();
        syncUI();
    </script>
</body>
</html>
"""


def parse_cli_value(arg):
    """Parses hex, binary, or decimal values safely."""
    # Clean input
    arg = arg.strip().replace("_", "").replace("u128", "")

    if arg.lower().startswith("0x"):
        return int(arg, 16)
    elif arg.lower().startswith("0b"):
        return int(arg, 2)
    else:
        try:
            return int(arg)
        except ValueError:
            # Try matching any hex or decimal digits inside brackets
            match = re.search(r"0x([0-9a-fA-F]+)", arg)
            if match:
                return int(match.group(1), 16)
            match = re.search(r"\d+", arg)
            if match:
                return int(match.group(0))
            return None


def print_ascii_board(val):
    """Renders a colored ASCII bitboard representation to the console."""
    is_tty = sys.stdout.isatty()

    # ANSI escape colors
    GREEN = "\033[1;32m" if is_tty else ""
    RED = "\033[31m" if is_tty else ""
    GRAY = "\033[90m" if is_tty else ""
    RESET = "\033[0m" if is_tty else ""
    CYAN = "\033[1;36m" if is_tty else ""

    print(f"\n{CYAN}--- Xiangqi Bitboard Visualizer ---{RESET}")
    print(f"Value: {hex(val)} ({val})\n")
    print("   +---+---+---+---+---+---+---+---+---+")
    for r in range(9, -1, -1):
        line = f" {r} |"
        for f in range(9):
            idx = r * 9 + f
            bit_set = (val & (1 << idx)) != 0

            # Palace square check
            is_palace = (r <= 2 or r >= 7) and (3 <= f <= 5)

            if bit_set:
                char = f"{GREEN} X {RESET}"
            elif is_palace:
                char = f"{RED} . {RESET}"
            else:
                char = f"{GRAY} . {RESET}"
            line += char + "|"
        print(line)
        print("   +---+---+---+---+---+---+---+---+---+")
    print("     a   b   c   d   e   f   g   h   i\n")


class VisualizerHandler(http.server.BaseHTTPRequestHandler):
    """Serves the interactive web UI page."""

    def log_message(self, format, *args):
        # Prevent spamming the terminal with GET log requests
        pass

    def do_GET(self):
        if self.path == "/":
            self.send_response(200)
            self.send_header("Content-type", "text/html")
            self.end_headers()
            self.wfile.write(HTML_CONTENT.encode("utf-8"))
        else:
            self.send_response(404)
            self.end_headers()


def start_server():
    """Starts the web server in a separate thread and opens the default browser."""
    # Try to find a free port, starting at 8080
    port = PORT
    server = None
    while port < PORT + 10:
        try:
            server = socketserver.TCPServer(("", port), VisualizerHandler)
            break
        except OSError:
            port += 1

    if not server:
        print("Error: Could not bind to any port between 8080 and 8090.")
        sys.exit(1)

    url = f"http://localhost:{port}/"
    print(f"Started Web UI server at {url}")
    print("Opening your web browser automatically...")
    print("Press Ctrl+C to terminate.")

    # Open the webpage in browser
    def open_browser():
        time.sleep(0.5)
        webbrowser.open(url)

    threading.Thread(target=open_browser, daemon=True).start()

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down server...")
        server.server_close()
        sys.exit(0)


def main():
    if len(sys.argv) > 1:
        # CLI Mode
        arg = sys.argv[1]
        if arg in ("-h", "--help", "help"):
            print(__doc__)
            sys.exit(0)

        val = parse_cli_value(arg)
        if val is None:
            print(f"Error: Could not parse input value '{arg}' as an integer.")
            sys.exit(1)

        print_ascii_board(val)
    else:
        # Web GUI Mode
        start_server()


if __name__ == "__main__":
    main()
