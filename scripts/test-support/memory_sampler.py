"""Observed RSS for an owned native fixture and its gateway; no process args/env."""
import os
from pathlib import Path
import threading


class ProcessMemory:
    def __init__(self, native_pid, gateway_executable, include_web_content=False):
        self.native_pid = native_pid
        self.gateway_executable = str(Path(gateway_executable).resolve())
        self.finished = threading.Event()
        self.values = {'nativePeakRssKiB': 0, 'gatewayPeakRssKiB': 0, 'samples': 0}
        self.include_web_content = include_web_content
        self.web_processes = {}
        if include_web_content:
            self.values['webContentPeakRssKiB'] = 0
        self.thread = threading.Thread(target=self._monitor, daemon=True)
        self.thread.start()

    def _sample(self, identity, key):
        try:
            lines = Path('/proc', str(identity), 'status').read_text().splitlines()
            rss = next(int(line.split()[1]) for line in lines if line.startswith('VmRSS:'))
            self.values[key] = max(self.values[key], rss)
        except (OSError, ValueError, StopIteration):
            pass

    def _monitor(self):
        gateway_pid = None
        while not self.finished.is_set():
            self._sample(self.native_pid, 'nativePeakRssKiB')
            descendants = []
            if gateway_pid is None or self.include_web_content:
                pending = [str(self.native_pid)]
                seen = set()
                while pending and len(seen) < 256:
                    identity = pending.pop()
                    if identity in seen:
                        continue
                    seen.add(identity)
                    descendants.append(identity)
                    for task in Path('/proc', identity, 'task').glob('*/children'):
                        try:
                            pending.extend(task.read_text().split())
                        except OSError:
                            pass
            if gateway_pid is None:
                # A fixture may own a credential-checking proxy between the
                # native process and its real gateway. Only inspect descendants.
                for identity in descendants:
                    try:
                        if os.readlink(Path('/proc', identity, 'exe')) == self.gateway_executable:
                            gateway_pid = identity
                            break
                    except OSError:
                        pass
            if gateway_pid is not None:
                self._sample(gateway_pid, 'gatewayPeakRssKiB')
            if self.include_web_content:
                total = 0
                for identity in descendants:
                    try:
                        process = Path('/proc', identity)
                        if not (process/'comm').read_text().startswith('WebKit'):
                            continue
                        self.web_processes[identity] = (process/'stat').read_text().rsplit(')', 1)[1].split()[19]
                        total += next(int(line.split()[1]) for line in (process/'status').read_text().splitlines() if line.startswith('VmRSS:'))
                    except (OSError, ValueError, IndexError, StopIteration):
                        pass
                self.values['webContentPeakRssKiB'] = max(self.values['webContentPeakRssKiB'], total)
            self.values['samples'] += 1
            self.finished.wait(.05)

    def close(self):
        self.finished.set()
        self.thread.join(timeout=2)
        result = dict(self.values)
        if self.include_web_content:
            result['webContentProcessesObserved'] = len(self.web_processes)
        return result

    def web_content_alive(self):
        for identity, started in self.web_processes.items():
            try:
                fields = Path('/proc', identity, 'stat').read_text().rsplit(')', 1)[1].split()
                if fields[19] == started and fields[0] != 'Z':
                    return True
            except (OSError, IndexError):
                pass
        return False
