#!/usr/bin/env python3
"""Exercise real Studio connectors + Codex CLI using a private daemon and local mock provider."""
import http.server, json, os, pathlib, shutil, subprocess, tempfile, threading, uuid
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
    directory = pathlib.Path(directory); project = directory / 'harness'; project.mkdir()
    manifest = '''[package]
name="studio-codex-shared-smoke"
version="0.1.0"
edition="2021"
[features]
windows-native=[]
[[bin]]
name="studio-codex-shared-smoke"
path=SOURCE
[dependencies]
axum={version="=0.8.9",features=["ws"]}
futures-util="0.3.31"
serde={version="=1.0.229",features=["derive"]}
serde_json="=1.0.151"
tokio={version="=1.53.3",features=["io-util","net","process","rt-multi-thread","sync","time","macros"]}
tokio-tungstenite={version="=0.29.0",default-features=false,features=["handshake"]}
uuid={version="1.23.5",features=["v4"]}
libc="0.2.174"
'''.replace('SOURCE', json.dumps(str(ROOT / 'scripts/test-support/codex-shared-smoke.rs')))
    (project / 'Cargo.toml').write_text(manifest); shutil.copyfile(ROOT / 'Cargo.lock', project / 'Cargo.lock')
    target = ROOT / 'target/shared-codex-smoke'
    build_env = {**os.environ, 'CARGO_TARGET_DIR': str(target)}
    subprocess.run(['cargo', 'test', '--manifest-path', str(project / 'Cargo.toml'), '--offline'], env=build_env, check=True)
    subprocess.run(['cargo', 'build', '--manifest-path', str(project / 'Cargo.toml'), '--offline'], env=build_env, check=True)
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
        subprocess.run([str(target / 'debug/studio-codex-shared-smoke')], cwd=directory, env=env, check=True, timeout=150)
        assert Provider.requests >= 2, Provider.requests
    finally:
        subprocess.run(['codex', 'app-server', 'daemon', 'stop'], env=env, capture_output=True, timeout=30)
        provider.shutdown(); provider.server_close()
