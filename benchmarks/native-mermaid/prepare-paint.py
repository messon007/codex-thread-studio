#!/usr/bin/env python3
"""Prepare the shared native worker patch for this independent evaluation workspace."""
from pathlib import Path
import runpy
import sys

if __name__ == '__main__':
    here = Path(__file__).resolve().parent
    sys.argv[1:1] = ['--workspace', str(here / 'paint'), '--component', 'merman-render']
    runpy.run_path(str(here.parent.parent / 'scripts' / 'prepare-gnome-mermaid.py'), run_name='__main__')
