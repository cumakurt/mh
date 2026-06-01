#!/usr/bin/env python3
"""Render docs/examples/*.txt terminal transcripts as lightweight SVG screenshots."""

from __future__ import annotations

import html
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EXAMPLES = ROOT / "docs" / "examples"
SCREENSHOTS = ROOT / "docs" / "screenshots"

FONT_SIZE = 14
LINE_HEIGHT = 20
CHAR_WIDTH = 8.4
PADDING_X = 18
PADDING_Y = 16
RADIUS = 8


def svg_for_text(text: str) -> str:
    lines = text.rstrip("\n").splitlines() or [""]
    width = int(max(len(line) for line in lines) * CHAR_WIDTH + PADDING_X * 2)
    height = PADDING_Y * 2 + LINE_HEIGHT * len(lines)
    text_lines = []
    for index, line in enumerate(lines):
        y = PADDING_Y + FONT_SIZE + index * LINE_HEIGHT
        fill = "#8fd694" if line.startswith("$ ") else "#d8dee9"
        text_lines.append(
            f'  <text x="{PADDING_X}" y="{y}" fill="{fill}">{html.escape(line)}</text>'
        )

    return "\n".join(
        [
            '<?xml version="1.0" encoding="UTF-8"?>',
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" role="img">',
            "  <title>mh command output</title>",
            f'  <rect width="100%" height="100%" rx="{RADIUS}" fill="#101418"/>',
            f'  <rect x="0.5" y="0.5" width="{width - 1}" height="{height - 1}" rx="{RADIUS}" fill="none" stroke="#2f3b45"/>',
            f'  <g font-family="JetBrains Mono, Fira Code, Menlo, Consolas, monospace" font-size="{FONT_SIZE}" xml:space="preserve">',
            *text_lines,
            "  </g>",
            "</svg>",
            "",
        ]
    )


def main() -> None:
    SCREENSHOTS.mkdir(parents=True, exist_ok=True)
    for example in sorted(EXAMPLES.glob("*.txt")):
        target = SCREENSHOTS / f"{example.stem}.svg"
        target.write_text(svg_for_text(example.read_text(encoding="utf-8")), encoding="utf-8")
        print(target.relative_to(ROOT))


if __name__ == "__main__":
    main()
