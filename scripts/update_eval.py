#!/usr/bin/env python3
import sys
import os
import re


def main():
    # Allow specifying tuner results file path as first argument
    results_path = sys.argv[1] if len(sys.argv) > 1 else "tuner_results.txt"

    if not os.path.exists(results_path):
        print(f"Error: {results_path} does not exist.")
        sys.exit(1)

    print(f"Reading tuner results from: {results_path}")

    with open(results_path, "r", encoding="utf-8") as f:
        content = f.read()

    # Split content by paste markers:
    # // --- Paste this into src/eval/defender_bonus.rs ---
    pattern = r"//\s*---\s*Paste\s+this\s+into\s+([^\s\-]+)\s*---"
    parts = re.split(pattern, content)

    if len(parts) < 3:
        print("Error: No paste markers found in the file.")
        sys.exit(1)

    # parts[0] is the header before the first marker.
    # parts[1] is the first file path.
    # parts[2] is the first block content, and so on.
    blocks = {}
    for i in range(1, len(parts), 2):
        file_path = parts[i].strip()
        block_content = parts[i + 1]
        blocks[file_path] = block_content

    # Define starting anchors for each file to locate the exact place to replace.
    anchors = {
        "src/eval/defender_bonus.rs": "pub(in crate::eval) const ADVISOR_COUNT_BONUS",
        "src/eval/piece_material_value.rs": "pub(in crate::eval) struct PieceMaterialValue;",
        "src/eval/mobility_tables.rs": "pub(in crate::eval) const KNIGHT_MOBILITY_BONUS",
        "src/eval/piece_square_tables.rs": "pub(in crate::eval) const PIECE_SQUARE_TABLE_KING_TAPERED",
    }

    for file_path, block in blocks.items():
        # Clean block content: strip empty lines from start and end
        block_lines = block.splitlines()
        # Find first non-empty line
        start_idx = 0
        while start_idx < len(block_lines) and not block_lines[start_idx].strip():
            start_idx += 1
        end_idx = len(block_lines)
        while end_idx > start_idx and not block_lines[end_idx - 1].strip():
            end_idx -= 1

        clean_block = (
            "\n".join([x.rstrip() for x in block_lines[start_idx:end_idx]]) + "\n"
        )

        if not os.path.exists(file_path):
            print(f"Warning: Target file {file_path} not found.")
            continue

        # Get anchor for this file
        anchor = anchors.get(file_path)
        if not anchor:
            print(f"Warning: No anchor defined for {file_path}. Skipping.")
            continue

        print(f"Updating {file_path} using anchor '{anchor}'...")

        with open(file_path, "r", encoding="utf-8") as f:
            target_content = f.read()

        target_lines = target_content.splitlines()

        # Locate the anchor in the target file
        anchor_line_idx = -1
        for idx, line in enumerate(target_lines):
            if anchor in line:
                anchor_line_idx = idx
                break

        if anchor_line_idx == -1:
            print(f"Error: Anchor '{anchor}' not found in {file_path}.")
            sys.exit(1)

        # For defender_bonus.rs and piece_square_tables.rs, we want to start from the preceding comment/decorator if possible.
        # Let's check if there is a comment/rustfmt::skip line before it
        replace_start_line = anchor_line_idx

        if file_path == "src/eval/defender_bonus.rs":
            # The line before is '// Tapered bonuses for having 0, 1, or 2 Advisors'
            if (
                anchor_line_idx > 0
                and "Tapered bonuses for having 0, 1, or 2 Advisors"
                in target_lines[anchor_line_idx - 1]
            ):
                replace_start_line = anchor_line_idx - 1
        elif file_path == "src/eval/piece_square_tables.rs":
            # The line before is '#[rustfmt::skip]'
            if (
                anchor_line_idx > 0
                and "#[rustfmt::skip]" in target_lines[anchor_line_idx - 1]
            ):
                replace_start_line = anchor_line_idx - 1

        # Reconstruct the file with the replaced block
        new_lines = target_lines[:replace_start_line]
        print(f"The new lines are: {new_lines}")
        new_content = "\n".join(new_lines) + "\n" + clean_block

        with open(file_path, "w", encoding="utf-8") as f:
            f.write(new_content)

        print(f"Successfully updated {file_path}")

    print("All updates completed successfully!")


if __name__ == "__main__":
    main()
