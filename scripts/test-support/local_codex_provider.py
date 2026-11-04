"""Shared loopback Responses fixture for real Codex integration tests."""
import datetime
import http.server
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import threading
import time
import uuid
from native_accessibility import ProcessPeer


class Provider(http.server.BaseHTTPRequestHandler):
    requests = 0
    lock = threading.Lock()

    def log_message(self, *_):
        pass

    def do_POST(self):
        size = int(self.headers.get('Content-Length', 0))
        if size > 8 * 1024 * 1024:
            self.send_error(413)
            return
        body = json.loads(self.rfile.read(size))
        with Provider.lock:
            Provider.requests += 1
            sequence = Provider.requests
            ledger = getattr(self.server, 'ledger', None)
            if ledger:
                with Path(ledger).open('a') as log:
                    log.write(json.dumps({'sequence': sequence, 'body': body}) + '\n')
        prefix = getattr(self.server, 'reply_prefix', None)
        text = f'{prefix} {sequence} 中文😀' if prefix else 'local smoke reply'
        item = {'id': 'msg_' + uuid.uuid4().hex, 'type': 'message', 'role': 'assistant',
                'status': 'completed', 'content': [{'type': 'output_text', 'text': text, 'annotations': []}]}
        response = {'id': 'resp_' + uuid.uuid4().hex, 'object': 'response', 'status': 'completed',
                    'output': [item], 'usage': {'input_tokens': 1, 'output_tokens': 1, 'total_tokens': 2}}
        events = [
            {'type': 'response.created', 'response': {**response, 'status': 'in_progress', 'output': []}},
            {'type': 'response.output_item.added', 'output_index': 0,
             'item': {**item, 'status': 'in_progress', 'content': []}},
            {'type': 'response.output_text.delta', 'output_index': 0, 'content_index': 0,
             'item_id': item['id'], 'delta': text[:-3] if prefix else text},
        ]
        tail = ([{'type': 'response.output_text.delta', 'output_index': 0, 'content_index': 0,
                  'item_id': item['id'], 'delta': text[-3:]}] if prefix else []) + [
            {'type': 'response.output_item.done', 'output_index': 0, 'item': item},
            {'type': 'response.completed', 'response': response},
        ]

        def encode(values):
            return ''.join('event: ' + event['type'] + '\ndata: ' + json.dumps(event) + '\n\n'
                           for event in values).encode()
        first, last = encode(events), encode(tail)
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.send_header('Content-Length', str(len(first) + len(last)))
        self.end_headers()
        self.wfile.write(first)
        self.wfile.flush()
        time.sleep(getattr(self.server, 'stream_delay', 0))
        self.wfile.write(last)


def prepare_profile(directory, port, workspace=None):
    """Seed only the caller-owned profile; return CODEX_HOME and its thread ID."""
    directory = Path(directory)
    workspace = Path(workspace or directory)
    home = directory / 'codex'
    home.mkdir()
    (home / 'studio-smoke-fixture').touch()
    (home / 'config.toml').write_text(
        f'model_provider="smoke"\nmodel="smoke"\n'
        f'[model_providers.smoke]\nname="Smoke"\nbase_url="http://127.0.0.1:{port}/v1"\nwire_api="responses"\n')
    thread = str(uuid.uuid4())
    now = datetime.datetime.now(datetime.timezone.utc)
    sessions = home / 'sessions' / now.strftime('%Y/%m/%d')
    sessions.mkdir(parents=True)
    stamp = now.isoformat().replace('+00:00', 'Z')
    records = [
        {'timestamp': stamp, 'type': 'session_meta', 'payload': {
            'id': thread, 'timestamp': stamp, 'cwd': str(workspace), 'originator': 'codex_cli_rs',
            'cli_version': '0.166.0', 'source': 'cli', 'model_provider': 'smoke'}},
        {'timestamp': stamp, 'type': 'response_item', 'payload': {
            'type': 'message', 'role': 'user', 'content': [{'type': 'input_text', 'text': 'Saved fixture'}]}},
    ]
    (sessions / f'rollout-{now.strftime("%Y-%m-%dT%H-%M-%S")}-{thread}.jsonl').write_text(
        ''.join(json.dumps(record) + '\n' for record in records))
    return home, thread


def fixture_homes(home, env):
    home = Path(home).resolve(strict=True)
    mounted = Path(env['CODEX_HOME']).resolve(strict=True)
    assert home.samefile(mounted) and (home / 'studio-smoke-fixture').is_file()
    return home, mounted


def private_daemon_processes(home, env):
    """Inspect only Codex executables installed under this marked fixture home."""
    home, mounted = fixture_homes(home, env)
    roots = [home / 'packages', mounted / 'packages']
    rows = []
    for p in Path('/proc').iterdir():
        if not p.name.isdecimal():
            continue
        try:
            target = (p / 'exe').resolve(strict=True)
            if target.name != 'codex' or not any(target.is_relative_to(root) for root in roots):
                continue
            fields = (p / 'stat').read_text().rsplit(')', 1)[1].split()
            if fields[0] == 'Z':
                continue
            argv = (p / 'cmdline').read_bytes().decode().split('\0')
            role = [value.split('=')[0] for value in argv[1:]
                    if value.startswith('--') or re.fullmatch('[a-z][a-z_-]{1,29}', value)]
            rows.append({'pid': int(p.name), 'started': fields[19], 'executable': str(target), 'roleArguments': role})
        except OSError:
            pass
    return rows


def stop_private_daemon(binary, home, env, output):
    """Stop the real server, then reap only its fixture-owned PID update helpers."""
    fixture_homes(home, env)  # Reject unmarked/foreign profiles first.
    stopped = subprocess.run([str(binary), 'app-server', 'daemon', 'stop'], env=env,
                             stdout=output, stderr=output, timeout=30)
    if sys.platform != 'linux':
        # Preserve the existing cross-platform CLI smoke command. /proc and
        # pidfds qualify Linux helper cleanup only, not macOS/Windows behavior.
        return {'ok': stopped.returncode == 0, 'stopExitCode': stopped.returncode,
                'pidHelperCleanupAvailable': False}
    helper_role = ['app-server', 'daemon', 'pid-update-loop']
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        rows = private_daemon_processes(home, env)
        if all(row['roleArguments'] == helper_role for row in rows):
            break
        time.sleep(.1)
    rows = private_daemon_processes(home, env)
    server_stopped = stopped.returncode == 0 and all(row['roleArguments'] == helper_role for row in rows)
    reaped = []
    if server_stopped:
        for row in rows:
            # pidfd pins the process across the identity check and signal; a
            # reused PID can never receive the cleanup signal.
            fd = os.pidfd_open(row['pid'])
            try:
                peer = ProcessPeer(row['pid'], row['executable'])
                assert peer.started == row['started'] and peer.poll() is None
                signal.pidfd_send_signal(fd, signal.SIGTERM)
                reaped.append(row)
            finally:
                os.close(fd)
    deadline = time.monotonic() + 10
    while private_daemon_processes(home, env) and time.monotonic() < deadline:
        time.sleep(.1)
    remaining = private_daemon_processes(home, env)
    return {'ok': server_stopped and not remaining, 'stopExitCode': stopped.returncode,
            'pidHelperCleanupAvailable': True,
            'appServerStoppedByCLI': server_stopped, 'fixturePidHelpersStopped': reaped,
            'remainingProcesses': remaining}
