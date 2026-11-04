"""Own one private Codex daemon and the shared loopback Responses fixture."""
import http.server
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time

sys.dont_write_bytecode = True
from local_codex_provider import Provider, prepare_profile, private_daemon_processes, stop_private_daemon


def main():
    root = Path(sys.argv[1]).resolve(strict=True)
    if os.environ.get('STUDIO_REAL_BACKEND_NAMESPACE') != str(root):
        raise RuntimeError('Real Codex fixture requires its private desktop/profile')
    binary = Path(os.environ['STUDIO_CODEX_BIN']).resolve(strict=True)
    assert binary == (root / 'tools/codex').resolve(strict=True)
    profile = root / 'studio-codex-smoke-profile'
    assert profile.is_dir() and not any(profile.iterdir())
    for name in ['home', 'config', 'data', 'cache', 'state', 'work']:
        (profile / name).mkdir()
    provider = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Provider)
    provider.reply_prefix = 'local Codex smoke reply'
    provider.ledger = root / 'model-requests.jsonl'
    provider.stream_delay = .6
    threading.Thread(target=provider.serve_forever, daemon=True).start()
    home, thread = prepare_profile(profile, provider.server_port, profile / 'work')
    mounted_home = Path('/tmp/studio-codex-profile/codex')
    assert home.samefile(mounted_home)
    env = dict(os.environ, HOME=str(profile / 'home'), CODEX_HOME=str(mounted_home),
               XDG_CONFIG_HOME=str(profile / 'config'), XDG_DATA_HOME=str(profile / 'data'),
               XDG_CACHE_HOME=str(profile / 'cache'), XDG_STATE_HOME=str(profile / 'state'))
    log = (root / 'codex-controller.log').open('w')

    def cli(*args):
        p = subprocess.run([str(binary), *args], env=env, cwd=profile / 'work',
                           stdout=subprocess.PIPE, stderr=log, text=True, timeout=45)
        assert p.returncode == 0, (args, p.returncode)
        log.write(p.stdout); log.flush()
        return p.stdout.strip()

    def processes():
        return private_daemon_processes(home, env)

    try:
        version = cli('--version')
        cli('app-server', 'daemon', '--help')
        cli('app-server', 'daemon', 'start')
        cli('queue', '--thread', thread, '--message', 'Seed from real Codex CLI', '--resume')
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if any('local Codex smoke reply 1 中文😀' in p.read_text(errors='replace')
                   for p in home.glob('sessions/**/*.jsonl')):
                break
            time.sleep(.1)
        else:
            raise RuntimeError('Real Codex CLI seed turn did not complete')
        assert Provider.requests == 1
        persistent = processes()
        assert persistent, 'Private shared daemon was not observed'
        print(json.dumps({'nativeFixtureReady': True, 'temporary': str(profile), 'threadId': thread,
                          'work': str(profile / 'work'), 'version': version, 'modelRequests': Provider.requests,
                          'replyPrefix': provider.reply_prefix, 'persistentProcesses': persistent,
                          'environment': {k: v for k, v in env.items() if k == 'HOME' or k == 'CODEX_HOME' or k.startswith('XDG_')}}), flush=True)
        for _ in sys.stdin:
            pass
    finally:
        assert home.parent == profile and (home / 'studio-smoke-fixture').is_file()
        cleanup = stop_private_daemon(binary, home, env, log)
        (root / 'codex-cleanup.json').write_text(json.dumps(cleanup, indent=2))
        provider.shutdown(); provider.server_close(); log.close()
        assert cleanup['ok'], cleanup


if __name__ == '__main__':
    main()
