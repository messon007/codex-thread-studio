#!/usr/bin/env python3
"""Exercise real Studio connectors + Codex CLI using a private daemon and local mock provider."""
import http.server, json, os, pathlib, subprocess, sys, tempfile, threading
ROOT = pathlib.Path(__file__).resolve().parents[1]

sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / 'scripts/test-support'))
from local_codex_provider import Provider, prepare_profile, stop_private_daemon

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
    provider = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Provider)
    threading.Thread(target=provider.serve_forever, daemon=True).start()
    home, thread = prepare_profile(directory, provider.server_port)
    env = {**os.environ, 'CODEX_HOME': str(home), 'STUDIO_CODEX_SMOKE_HOME': str(home), 'STUDIO_TEST_THREAD_ID': thread, 'CODEX_THREAD_STUDIO_CODEX_CONNECTION': 'auto'}
    try:
        subprocess.run([artifacts[0], '--exact', 'tests::codex_shared_daemon_with_real_cli',
            '--ignored', '--nocapture', '--test-threads=1'], cwd=directory, env=env, check=True, timeout=150)
        assert Provider.requests >= 2, Provider.requests
    finally:
        cleanup = stop_private_daemon('codex', home, env, sys.stdout)
        provider.shutdown(); provider.server_close()
        assert cleanup['ok'], cleanup
