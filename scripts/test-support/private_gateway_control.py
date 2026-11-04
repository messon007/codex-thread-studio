"""Owned fixture gateway parent-pipe control; no process discovery or host signals."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time


def main(gateway, argv):
    root = Path(os.environ['STUDIO_GNOME_SMOKE_DIR'])
    child = subprocess.Popen([gateway, *argv], stdin=subprocess.PIPE)
    with (root / 'diagnostics-launches.jsonl').open('a') as log:
        log.write(json.dumps(dict(wrapper=os.getpid(), gateway=child.pid)) + '\n')

    def stop(*_):
        try:
            child.stdin.close()
        except (BrokenPipeError, ValueError):
            pass

    def parent():
        while os.read(sys.stdin.fileno(), 8192):
            pass
        stop()

    threading.Thread(target=parent, daemon=True).start()
    signal.signal(signal.SIGTERM, stop)
    while child.poll() is None:
        if (root / 'stop-gateway').exists():
            stop()
        time.sleep(.02)
    return child.wait()
