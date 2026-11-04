#!/usr/bin/env python3
"""Run Cargo against verified canonical native Mermaid patch directories."""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def main():
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace', type=Path, default=root / 'native/mermaid-worker')
    parser.add_argument('--component', choices=['all', 'merman-render', 'merman-export', 'usvg'],
                        default='all')
    parser.add_argument('cargo', nargs=argparse.REMAINDER, help='Cargo subcommand and arguments')
    args = parser.parse_args()
    if not args.cargo:
        parser.error('a Cargo subcommand is required')
    workspace = args.workspace.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='studio-mermaid-cargo-') as temporary:
        config = Path(temporary) / 'patches.toml'
        command = [sys.executable, str(root / 'scripts/prepare-gnome-mermaid.py'),
                   '--workspace', str(workspace), '--component', args.component,
                   '--cargo-config', str(config)]
        if '--offline' in args.cargo or os.environ.get('CARGO_NET_OFFLINE') == 'true':
            command.append('--offline')
        # Cargo metadata must keep stdout as JSON. Source preparation messages
        # belong to stderr for every command, including its patch-tool output.
        subprocess.run(command, stdout=sys.stderr, check=True)
        result = subprocess.run(['cargo', '--config', str(config), args.cargo[0],
                                 '--manifest-path', str(workspace / 'Cargo.toml'),
                                 *args.cargo[1:]])
    raise SystemExit(result.returncode)


if __name__ == '__main__':
    main()
