#!/usr/bin/env python3
"""Alternate fresh processes; do not clear shared OS caches or other apps' state."""
import argparse
import json
import os
import selectors
import statistics
import subprocess
import time
from pathlib import Path


def probe(binary, source):
    started = time.perf_counter()
    deadline = started + 15
    process = subprocess.Popen([binary, 'rust'], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    channels = selectors.DefaultSelector()
    buffers = {'stdout': bytearray(), 'stderr': bytearray()}
    ready_ms = None
    try:
        process.stdin.write(source)
        process.stdin.close()
        for name in buffers:
            channels.register(getattr(process, name), selectors.EVENT_READ, name)
        while channels.get_map():
            remaining = deadline - time.perf_counter()
            if remaining <= 0:
                raise TimeoutError('Highlight probe exceeded 15 seconds')
            for key, _ in channels.select(remaining):
                chunk = os.read(key.fd, 65536)
                if not chunk:
                    channels.unregister(key.fileobj)
                    continue
                buffers[key.data].extend(chunk)
                if len(buffers[key.data]) > 131072:
                    raise ValueError('Highlight probe output exceeds 128 KiB')
                if key.data == 'stdout' and ready_ms is None and b'\n' in buffers['stdout']:
                    ready_ms = (time.perf_counter() - started) * 1000
        process.wait(timeout=max(0.001, deadline - time.perf_counter()))
        assert process.returncode == 0, buffers['stderr'].decode(errors='replace')
        lines = buffers['stdout'].splitlines()
        assert len(lines) == 2 and ready_ms is not None
        ready, result = (json.loads(line) for line in lines)
        assert ready['phase'] == 'ready'
        return ready, result, ready_ms
    finally:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=3)
        channels.close()
        for stream in [process.stdin, process.stdout, process.stderr]:
            stream.close()


parser = argparse.ArgumentParser()
parser.add_argument('full')
parser.add_argument('reduced')
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()
source = '// 中文😀\r\nfn main() { let answer = 42; println!("hello <world>"); }\r\n'.encode()
rows = []
load_before = os.getloadavg()
for run in range(12):
    for profile in (['full', 'reduced'] if run % 2 == 0 else ['reduced', 'full']):
        ready, result, startup = probe(getattr(args, profile), source)
        assert ready['grammarCount'] == (110 if profile == 'full' else 97)
        assert result['complete'] and not result['fallback'] and result['runs'] > 1
        rows.append({'profile': profile, 'run': run, 'processToReadyMs': startup,
                     **{key: result[key] for key in ['firstHighlightMs', 'warmHighlightMs', 'sourceBytes']}})
report = {
    'method': 'Fresh process to flushed ready report (includes input transport); first production highlighter call; mean of 32 subsequent calls. OS caches are not purged. 12 runs/profile, alternating order. No builds concurrently.',
    'loadBefore': load_before, 'loadAfter': os.getloadavg(), 'samples': rows,
    'median': {profile: {key: statistics.median(row[key] for row in rows if row['profile'] == profile)
                         for key in ['processToReadyMs', 'firstHighlightMs', 'warmHighlightMs']}
               for profile in ['full', 'reduced']}}
args.output.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({key: value for key, value in report.items() if key != 'samples'}))
