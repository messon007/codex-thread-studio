#!/usr/bin/env python3
"""Create inspection sheets from actual benchmark PNGs (requires Pillow).

Only alpha-composites and scales measured output. Original rasters stay intact.
The output contains the same private document content as the benchmark directory.
"""
import argparse
import json
from pathlib import Path

from PIL import Image, ImageDraw


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results', type=Path)
    args = parser.parse_args()
    root = args.results.resolve()
    cases = json.loads((root / 'results.json').read_text())['cases']
    cases = [row for row in cases if all(e.get('ok') for e in row['engines'].values())]
    for page, offset in enumerate(range(0, len(cases), 6), 1):
        rows = cases[offset:offset + 6]
        sheet = Image.new('RGB', (1440, len(rows) * 280), '#ececec')
        draw = ImageDraw.Draw(sheet)
        for number, row in enumerate(rows):
            y = number * 280
            for column, engine in enumerate(['merman', 'pele']):
                x = column * 720
                result = row['engines'][engine]
                label = f"{row['id']} / {engine} / {result['width']} x {result['height']}"
                draw.text((x + 10, y + 6), label, fill='black')
                image = Image.open(root / row['id'] / (engine + '.png')).convert('RGBA')
                canvas = Image.new('RGBA', image.size, '#202020' if row['dark'] else 'white')
                canvas.alpha_composite(image)
                canvas = canvas.convert('RGB')
                canvas.thumbnail((700, 245), Image.Resampling.LANCZOS)
                sheet.paste(canvas, (x + 10, y + 28))
        sheet.save(root / f'gallery-{page:02}.png')


if __name__ == '__main__':
    main()
