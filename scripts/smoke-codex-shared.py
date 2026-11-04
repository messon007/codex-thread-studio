#!/usr/bin/env python3
"""Exercise real Studio connectors + Codex CLI using a private daemon and local mock provider."""
import http.server, json, os, pathlib, subprocess, sys, tempfile, threading, uuid
ROOT = pathlib.Path(__file__).resolve().parents[1]

class Provider(http.server.BaseHTTPRequestHandler):
    requests = 0
    def log_message(self, *_): pass
    def do_POST(self):
        self.rfile.read(int(self.headers.get('Content-Length', 0)))
        Provider.requests += 1
        item = {'id': 'msg_' + uuid.uuid4().hex, 'type': 'message', 'role': 'assistant', 'status': 'completed', 'content': [{'type': 'output_text', 'text': 'local smoke reply', 'annotations': []}]}
        response = {'id': 'resp_' + uuid.uuid4().hex, 'object': 'response', 'status': 'completed', 'output': [item], 'usage': {'input_tokens': 1, 'output_tokens': 1, 'total_tokens': 2}}
        events = [
            {'type': 'response.created', 'response': {**response, 'status': 'in_progress', 'output': []}},
            {'type': 'response.output_item.added', 'output_index': 0, 'item': {**item, 'status': 'in_progress', 'content': []}},
            {'type': 'response.output_text.delta', 'output_index': 0, 'content_index': 0, 'item_id': item['id'], 'delta': 'local smoke reply'},
            {'type': 'response.output_item.done', 'output_index': 0, 'item': item},
            {'type': 'response.completed', 'response': response},
        ]
        data = ''.join('event: ' + event['type'] + '\ndata: ' + json.dumps(event) + '\n\n' for event in events).encode()
        self.send_response(200); self.send_header('Content-Type', 'text/event-stream'); self.send_header('Content-Length', str(len(data))); self.end_headers(); self.wfile.write(data)

with tempfile.TemporaryDirectory(prefix='studio-shared-smoke-') as directory:
    directory = pathlib.Path(directory)
    # Compile the production gateway test crate, including its real native
    # routing hooks. A separate source harness cannot supply those dependencies
    # faithfully. Reuse the caller's Cargo cache/target rather than creating a
    # second dependency tree for this one smoke check.
    build_env = dict(os.environ)
    build_env.setdefault('CARGO_BUILD_JOBS', '2')
    compiled = subprocess.run(['cargo', 'test', '-p', 'codex-thread-studio',
        '--bin', 'codex-thread-studio', '--no-default-features', '--locked',
        '--offline', '--no-run', '--message-format=json'], cwd=ROOT,
        env=build_env, capture_output=True, text=True, timeout=600)
    if compiled.returncode:
        print(compiled.stderr, file=sys.stderr)
        print(compiled.stdout[-16000:], file=sys.stderr)
        raise SystemExit(compiled.returncode)
    artifacts = []
    for line in compiled.stdout.splitlines():
        try: value = json.loads(line)
        except json.JSONDecodeError: continue
        if value.get('reason')=='compiler-artifact' and value.get('profile', {}).get('test') and value.get('target', {}).get('name')=='codex-thread-studio' and value.get('executable'):
            artifacts.append(value['executable'])
    assert len(artifacts)==1, artifacts
    home = directory / 'codex'; home.mkdir(); (home / 'studio-smoke-fixture').touch()
    provider = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Provider)
    threading.Thread(target=provider.serve_forever, daemon=True).start()
    (home / 'config.toml').write_text(f'model_provider="smoke"\nmodel="smoke"\n[model_providers.smoke]\nname="Smoke"\nbase_url="http://127.0.0.1:{provider.server_port}/v1"\nwire_api="responses"\n')
    thread = str(uuid.uuid4()); sessions = home / 'sessions/2026/11/04'; sessions.mkdir(parents=True)
    stamp = '2026-11-04T00:00:00Z'
    records = [
        {'timestamp': stamp, 'type': 'session_meta', 'payload': {'id': thread, 'timestamp': stamp, 'cwd': str(directory), 'originator': 'codex_cli_rs', 'cli_version': '0.166.0', 'source': 'cli', 'model_provider': 'smoke'}},
        {'timestamp': stamp, 'type': 'response_item', 'payload': {'type': 'message', 'role': 'user', 'content': [{'type': 'input_text', 'text': 'Saved fixture'}]}},
    ]
    (sessions / f'rollout-2026-11-04T00-00-00-{thread}.jsonl').write_text(''.join(json.dumps(record)+'\n' for record in records))
    env = {**os.environ, 'CODEX_HOME': str(home), 'STUDIO_CODEX_SMOKE_HOME': str(home), 'STUDIO_TEST_THREAD_ID': thread, 'CODEX_THREAD_STUDIO_CODEX_CONNECTION': 'auto'}
    try:
        subprocess.run([artifacts[0], '--exact', 'tests::codex_shared_daemon_with_real_cli',
            '--ignored', '--nocapture', '--test-threads=1'], cwd=directory, env=env, check=True, timeout=150)
        assert Provider.requests >= 2, Provider.requests
    finally:
        subprocess.run(['codex', 'app-server', 'daemon', 'stop'], env=env, capture_output=True, timeout=30)
        provider.shutdown(); provider.server_close()
