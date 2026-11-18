#!/usr/bin/env python3
"""Run a GUI check on an owned headless GNOME Shell display and private session bus.

Usage: python3 tests/native/owned-wayland-run.py --artifacts target/wayland-checks \
  -- node tests/native/gtk-reader-baseline.mjs

The command inherits its working directory, but cannot connect to the login
Wayland socket or session bus. Profiles/logs belong to this one invocation.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import sys
import time

# Internal mode runs only on the fresh bus established below.
if sys.argv[1:2] == ['--private-session']:
    root = Path(sys.argv[2]).resolve(strict=True)
    runtime = Path(os.environ['XDG_RUNTIME_DIR']).resolve(strict=True)
    if (Path('/proc', str(os.getppid()), 'exe').resolve() != Path('/usr/bin/dbus-run-session').resolve()
            or runtime.parent != Path('/tmp') or not runtime.name.startswith('mn-wayland-')
            or os.environ.get('XDG_CONFIG_HOME') != str(root / 'config')):
        raise RuntimeError('Private mode requires the owned dbus-run-session, runtime and profile')
    os.environ['WAYLAND_DISPLAY'] = 'marknoria-check'
    with (root / 'gnome-shell.log').open('w') as shell_log:
        shell = subprocess.Popen(['gnome-shell', '--headless', '--wayland', '--no-x11',
            '--wayland-display', 'marknoria-check', '--virtual-monitor', '1800x1200'],
            stdout=shell_log, stderr=subprocess.STDOUT)
        os.environ['MARKNORIA_TEST_COMPOSITOR_PID'] = str(shell.pid)
        try:
            deadline = time.monotonic() + 30
            while not (runtime / 'marknoria-check').is_socket():
                if shell.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError('Owned GNOME Shell did not become ready; inspect gnome-shell.log')
                time.sleep(.1)
            time.sleep(2)
            code = subprocess.run(sys.argv[3:], check=False).returncode
        finally:
            shell.terminate()
            try:
                shell.wait(timeout=10)
            except subprocess.TimeoutExpired:
                shell.kill()
                shell.wait(timeout=5)
    raise SystemExit(code)

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--artifacts', default='target/wayland-checks')
parser.add_argument('command', nargs=argparse.REMAINDER)
args = parser.parse_args()
command = args.command[1:] if args.command[:1] == ['--'] else args.command
if not command:
    parser.error('A command after -- is required')
base = Path(args.artifacts).resolve()
base.mkdir(parents=True, exist_ok=True)
root = Path(tempfile.mkdtemp(prefix='run-', dir=base))
env = dict(os.environ)
for key in ['DISPLAY', 'WAYLAND_DISPLAY', 'XAUTHORITY', 'AT_SPI_BUS_ADDRESS',
            'DBUS_SESSION_BUS_ADDRESS', 'DBUS_SESSION_BUS_PID', 'SESSION_MANAGER']:
    env.pop(key, None)
for key, folder in [('XDG_CONFIG_HOME', 'config'), ('XDG_CACHE_HOME', 'cache'),
                    ('XDG_DATA_HOME', 'data'), ('XDG_STATE_HOME', 'state')]:
    path = root / folder
    path.mkdir()
    env[key] = str(path)
env.update(GDK_BACKEND='wayland', XDG_SESSION_TYPE='wayland',
           GSETTINGS_BACKEND='memory', GTK_USE_PORTAL='0', GVFS_DISABLE_FUSE='1')
version = subprocess.run(['gnome-shell', '--version'], capture_output=True, text=True, check=True).stdout.strip()
env['MARKNORIA_TEST_COMPOSITOR'] = version
(root / 'session.json').write_text(json.dumps({'compositor': version,
    'virtualMonitor': '1800x1200', 'ownedHeadlessWayland': True,
    'rendererOverride': env.get('GSK_RENDERER')}, indent=2) + '\n')
with tempfile.TemporaryDirectory(prefix='mn-wayland-', dir='/tmp') as runtime:
    env['XDG_RUNTIME_DIR'] = runtime
    argv = ['dbus-run-session', '--', sys.executable, str(Path(__file__).resolve()),
            '--private-session', str(root), *command]
    with (root / 'session.log').open('w') as log:
        child = subprocess.Popen(argv, env=env, stdout=log, stderr=subprocess.STDOUT,
                                 start_new_session=True)
        try:
            code = child.wait(timeout=900)
        except (KeyboardInterrupt, subprocess.TimeoutExpired):
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=5)
            raise
        finally:
            # The private bus may leave child services after its command exits.
            # Terminate only the process group created by this invocation.
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            # A private portal can leave its disconnected FUSE mount briefly
            # after the bus exits. Detach only known mounts inside this run's
            # freshly created runtime before TemporaryDirectory removes it.
            for name in ('doc', 'gvfs'):
                mount = str(Path(runtime) / name)
                for _ in range(3):
                    mounts = {line.split()[4] for line in Path('/proc/self/mountinfo').read_text().splitlines()}
                    if mount not in mounts:
                        break
                    unmount = subprocess.run(['fusermount3', '-uz', mount], check=False,
                                             stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
                    time.sleep(.1)
                    remaining = {line.split()[4] for line in Path('/proc/self/mountinfo').read_text().splitlines()}
                    if unmount.returncode and mount in remaining:
                        raise RuntimeError(f'Could not detach owned runtime mount {mount}: {unmount.stderr.decode(errors="replace")}')
print(f'Owned Wayland artifacts: {root}')
if code:
    with (root / 'session.log').open('rb') as log:
        log.seek(0, 2)
        log.seek(max(0, log.tell() - 4096))
        print(log.read().decode('utf-8', errors='replace'))
raise SystemExit(code)
