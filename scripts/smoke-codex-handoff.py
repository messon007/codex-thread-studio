#!/usr/bin/env python3
"""Verify writer handoff with two real Codex app servers; no inference requests."""
import json, os, pathlib, queue, subprocess, tempfile, threading, time, uuid

class Server:
    def __init__(self, home, cwd):
        self.process = subprocess.Popen(['codex', 'app-server', '--stdio', '-c', 'thread_unload_delay_secs=0',
            '-c', 'model_provider="smoke"', '-c', 'model="smoke"',
            '-c', 'model_providers.smoke.name="Smoke"', '-c', 'model_providers.smoke.base_url="http://127.0.0.1:1/v1"',
            '-c', 'model_providers.smoke.wire_api="responses"'],
            env={**os.environ, 'CODEX_HOME': str(home)}, cwd=cwd,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        self.messages = queue.Queue(); self.next_id = 0
        def read():
            for line in self.process.stdout:
                self.messages.put(json.loads(line))
        threading.Thread(target=read, daemon=True).start()
        self.request('initialize', {'clientInfo': {'name': 'studio-handoff-smoke', 'version': '1'}})
        self.process.stdin.write('{"method":"initialized"}\n'); self.process.stdin.flush()
    def request(self, method, params, error=False):
        self.next_id += 1; request_id = self.next_id
        self.process.stdin.write(json.dumps({'id': request_id, 'method': method, 'params': params}) + '\n'); self.process.stdin.flush()
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            message = self.messages.get(timeout=max(.1, deadline-time.monotonic()))
            if message.get('id') == request_id:
                if error: assert 'error' in message, message
                else: assert 'error' not in message, message
                return message.get('error') if error else message['result']
        raise AssertionError('RPC timeout: ' + method)
    def close(self):
        self.process.terminate()
        try: self.process.wait(timeout=5)
        except subprocess.TimeoutExpired: self.process.kill(); self.process.wait()

with tempfile.TemporaryDirectory(prefix='studio-codex-handoff-') as root:
    root = pathlib.Path(root); home = root / 'codex'; home.mkdir()
    thread_id = str(uuid.uuid4()); timestamp = '2026-11-04T00:00:00Z'
    sessions = home / 'sessions/2026/11/04'; sessions.mkdir(parents=True)
    records = [
        {'timestamp': timestamp, 'type': 'session_meta', 'payload': {'id': thread_id, 'timestamp': timestamp, 'cwd': str(root), 'originator': 'codex_cli_rs', 'cli_version': '0.166.0', 'source': 'cli', 'model_provider': 'smoke'}},
        {'timestamp': timestamp, 'type': 'response_item', 'payload': {'type': 'message', 'role': 'user', 'content': [{'type': 'input_text', 'text': 'Saved fixture; do not run inference.'}]}},
    ]
    (sessions / f'rollout-2026-11-04T00-00-00-{thread_id}.jsonl').write_text(''.join(json.dumps(record)+'\n' for record in records))
    first = Server(home, root); second = None
    try:
        first.request('thread/resume', {'threadId': thread_id})
        second = Server(home, root)
        locked = second.request('thread/resume', {'threadId': thread_id}, error=True)
        assert 'writer' in json.dumps(locked).lower() or 'active' in json.dumps(locked).lower(), locked
        first.request('thread/unsubscribe', {'threadId': thread_id})
        deadline = time.monotonic() + 10
        while True:
            try:
                second.request('thread/resume', {'threadId': thread_id})
                break
            except AssertionError:
                if time.monotonic() > deadline: raise
                time.sleep(.1)
        print('Codex writer lock released and acquired by second app server; no inference requested')
    finally:
        first.close()
        if second: second.close()
