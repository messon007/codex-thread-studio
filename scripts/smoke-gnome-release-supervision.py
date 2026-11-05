#!/usr/bin/env python3
"""Installed supervision controls through AT-SPI, private providers, no app hooks."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from native_accessibility import Client
from private_wayland import OwnedMutterInput, isolated_environment
from memory_sampler import ProcessMemory

spec = importlib.util.spec_from_file_location('installed_supervision_fixture', Path(__file__).with_name('smoke-gnome-supervision.py'))
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)
runtime = fixture.runtime
DRAFT = 'Unsent installed supervision draft 中文😀 '
QUEUED = 'Queued installed follow-up'
PAUSED = 'Queued after installed Stop'


def run(prefix, root):
    native, gateway = prefix / 'bin/codex-thread-studio-gnome', prefix / 'bin/codex-thread-studio-gateway'
    owned = OwnedMutterInput()
    log = (root / 'app.log').open('w')
    child = subprocess.Popen([str(native)], cwd=root, env=os.environ, stdout=log, stderr=log)
    memory = ProcessMemory(child.pid, gateway)
    desktop = Client(child)

    def keys(values):
        for value in values:
            owned.call('NotifyKeyboardKeysym', owned.GLib.Variant('(ub)', (value, True)))
        time.sleep(.06)
        for value in reversed(values):
            owned.call('NotifyKeyboardKeysym', owned.GLib.Variant('(ub)', (value, False)))

    def open_supervision():
        desktop.click('More actions')
        desktop.click('Turn supervision')

    def close_supervision():
        # Supervision is nested inside More actions. Dismiss the child first,
        # then the parent if GTK leaves it open, using normal keyboard input.
        keys([0xff1b])
        desktop.wait('supervision popover closed', lambda: not desktop.find(lambda n:
            n.get_name() in ('Supervise this turn', 'Stop supervision') and desktop.showing(n)))
        if desktop.find(lambda n: n.get_name() == 'Turn supervision' and desktop.showing(n)):
            keys([0xff1b])
        desktop.wait('composer options popover closed', lambda: not desktop.find(lambda n:
            n.get_name() == 'Turn supervision' and desktop.showing(n)))

    def composer():
        return desktop.wait('native composer', lambda: desktop.find(lambda n:
            n.get_role_name() == 'text' and n.get_name().startswith('Write a message.')
            and desktop.showing(n) and n.get_editable_text_iface() is not None))

    def source_starts():
        return runtime.starts(root, 'target')

    def queue():
        with sqlite3.connect(f'file:{root / "config/codex-thread-studio/studio.sqlite3"}?mode=ro', uri=True) as database:
            row = database.execute("SELECT queue_json FROM session_message_queues WHERE session_key='codex:target'").fetchone()
        return json.loads(row[0]) if row else []

    def enqueue(text):
        desktop.edit(composer(), text)
        desktop.click('Queue')
        desktop.wait('persisted queued message', lambda: [m['text'] for m in queue()] == [text])

    def flag(name):
        (root / name).write_text('1')

    def source_completed(turn):
        path = root / 'codex-turns.json'
        try:
            return any(t['id'] == turn and t['status'] == 'completed' for t in json.loads(path.read_text()).get('target', []))
        except json.JSONDecodeError:
            return False  # Fake-provider ledger replacement can overlap a read.

    try:
        desktop.wait('loaded restored source', lambda: desktop.text('Native target') and desktop.find(lambda n:
            n.get_name() == 'Stop' and desktop.showing(n) and desktop.sensitive(n)))
        owned.focus_only_window()
        assert not any(r.get('method') == 'thread/start' for r in runtime.requests(root)), 'Supervision started without opt-in'
        open_supervision()
        desktop.click('Supervise this turn')
        desktop.wait('native supervision waiting', lambda: desktop.text('Waiting') and desktop.text('0/5'))
        close_supervision()
        enqueue(QUEUED)
        desktop.edit(composer(), DRAFT)
        flag('release-source-1')
        desktop.wait('owned evaluator and visible progress', lambda:
            (root / 'evaluation-held-1').exists() and desktop.text('Turn supervision · Evaluating · 0/5'))
        assert len(source_starts()) == 1 and [m['text'] for m in queue()] == [QUEUED], 'Installed queue escaped evaluation'
        assert desktop.contents(composer()) == DRAFT
        open_supervision()
        desktop.wait('named Stop action', lambda: desktop.find(lambda n:
            n.get_name() == 'Stop supervision' and desktop.showing(n) and desktop.sensitive(n)))
        close_supervision()
        flag('release-evaluation-1')
        desktop.wait('exact automatic continuation', lambda: (root / 'source-held-2').exists())
        assert len(source_starts()) == 2
        assert source_starts()[1]['params']['input'][0]['text'] == '[Studio turn supervisor]\nFinish B'
        assert [m['text'] for m in queue()] == [QUEUED] and desktop.contents(composer()) == DRAFT
        flag('release-source-2')
        desktop.wait('queued follow-up after complete', lambda: len(source_starts()) == 3 and source_completed('target-turn-3') and not queue())
        desktop.wait('both owned evaluators cleaned', lambda: (root / 'utility-cleaned-2').exists())
        assert desktop.contents(composer()) == DRAFT

        # The previous terminal status must not describe this new running turn.
        desktop.edit(composer(), 'Installed Stop source')
        desktop.click('Send')
        desktop.wait('fourth source accepted', lambda: (root / 'source-held-4').exists())
        open_supervision()
        next_action = desktop.wait('new turn opt-in action', lambda: desktop.find(lambda n:
            n.get_name() == 'Supervise this turn' and desktop.showing(n) and desktop.sensitive(n)))
        # A completion toast from the preceding task remains legitimate. The
        # new turn's menu row must describe only its own current supervision.
        group = next_action.get_parent()
        assert not desktop.find(lambda n: desktop.within(n, group) and desktop.showing(n)
            and 'Supervision complete' in n.get_name()), 'Previous terminal status leaked into a newer turn'
        desktop.click('Supervise this turn')
        desktop.wait('new supervisor waiting', lambda: desktop.text('Waiting') and desktop.text('0/5'))
        close_supervision()
        enqueue(PAUSED)
        open_supervision()
        desktop.click('Stop supervision')
        desktop.wait('explicit supervision stop', lambda: desktop.text('Supervision stopped'))
        close_supervision()
        desktop.wait('queue paused by Stop', lambda: desktop.text('Supervision stopped by user')
            and desktop.find(lambda n: n.get_name() == 'Resume' and desktop.showing(n)
                and desktop.sensitive(n) and 'button' in n.get_role_name()))
        flag('release-source-4')
        desktop.wait('provider turn finishes independently', lambda: source_completed('target-turn-4') and desktop.find(lambda n:
            n.get_name() == 'Send' and desktop.showing(n) and desktop.sensitive(n)))
        assert [m['text'] for m in queue()] == [PAUSED]
        assert len(source_starts()) == 4
        assert len([r for r in runtime.requests(root) if r.get('method') == 'thread/start']) == 2, 'Stopped supervision restarted an evaluator'
        owned.focus_only_window()
        keys([0xffe9, 0xffc1])
        child.wait(timeout=25)
        assert child.returncode == 0
    except BaseException:
        (root / 'accessibility.json').write_text(json.dumps(desktop.describe(), indent=2, ensure_ascii=False))
        raise
    finally:
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=10)
        measured = memory.close()
        owned.close()
        log.close()

    requests = runtime.requests(root)
    utilities = [r for r in requests if r.get('method') == 'thread/start']
    deletions = [r for r in requests if r.get('method') == 'thread/delete']
    assert len(source_starts()) == 4 and len(utilities) == 2 and len(deletions) == 2
    assert all(r['params'].get('serviceTier') == 'fast' for r in source_starts()[1:])
    with sqlite3.connect(root / 'config/codex-thread-studio/studio.sqlite3') as database:
        jobs = [json.loads(row[0]) for row in database.execute("SELECT json FROM native_router_jobs WHERE controller='codex:router'")]
    assert len(jobs) == 1 and jobs[0]['status'] == 'completed' and jobs[0]['targetTurnId'] == 'target-turn-2', jobs
    assert jobs[0]['supervision']['previousTurnId'] == 'target-turn-1'
    for name in ['backend.pid', 'opencode.pid']:
        if (root / name).exists():
            assert not Path('/proc', (root / name).read_text().strip()).exists(), 'Owned provider survived'
    assert not any(token in (root / 'app.log').read_text() for token in ['CRITICAL', 'panicked', 'Gtk-WARNING'])
    result = dict(ok=True, installedPair=True, noApplicationHooks=True, noGatewayOverride=True,
        defaultOff=True, coldTargetOptIn=True, visibleProgress=True, actualUtilityEvaluation=True,
        exactContinuation=True, queueHeld=True, queueReleasedAfterComplete=True, draftPreserved=True,
        durableRouterHandoff=True, staleTerminalStatusAbsent=True, stopPausesQueue=True,
        noResumeAfterStop=True, sourceTurns=4, utilityEvaluations=2, utilityCleanup=2,
        memory=measured, artifacts=str(root))
    (root / 'installed-supervision-result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle')
    parser.add_argument('--private-root')
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    bundle = Path(args.bundle).resolve()
    if args.private_root:
        return run(bundle, Path(args.private_root))
    base = Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1] / 'target/gnome-native-smoke')
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='release-supervision-', dir=base))
    prefix = root / 'installed native 中文 % test'
    subprocess.run(['bash', str(bundle / 'install.sh'), str(bundle), str(prefix)], check=True, stdout=subprocess.DEVNULL)
    runtime.create_profile(root)
    with sqlite3.connect(root / 'config/codex-thread-studio/studio.sqlite3') as database:
        database.execute("INSERT INTO session_turn_options VALUES('codex:router','controller-model','high','default')")
        database.execute("UPDATE session_turn_options SET service_tier='fast' WHERE session_key='codex:target'")
    runtime.fixture.BACKEND = fixture.source()
    runtime.fixture.OPENCODE_BACKEND = runtime.OPENCODE
    seed = runtime.fixture.Gateway(prefix / 'bin/codex-thread-studio-gateway', root, 'opencode')
    try:
        identity = runtime.submit(seed, runtime.body(seed, 'Installed restored routed supervision A and B'))
        runtime.until(lambda: runtime.get(seed, identity)['status'] == 'running')
    finally:
        seed.close()
    (root / 'restore-mode').touch()
    for name, code in [('fake-codex.py', fixture.source()), ('fake-opencode.py', runtime.OPENCODE)]:
        path = root / name
        path.write_text(code)
        path.chmod(0o700)
    (root / 'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex', selectedSession='target')))
    env = dict(os.environ, CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root / 'config/codex-thread-studio/backends.json'),
        CODEX_THREAD_STUDIO_CODEX_BIN=str(root / 'fake-codex.py'),
        CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root / 'fake-opencode.py'), CODEX_HOME=str(root / 'codex'),
        XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'), XDG_CACHE_HOME=str(root / 'cache'),
        READER_CACHE_DIR=str(root / 'reader-cache'), GDK_BACKEND='wayland', GIO_USE_VFS='local',
        GIO_USE_VOLUME_MONITOR='unix', GTK_A11Y='atspi')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE') or key == 'CODEX_THREAD_STUDIO_GATEWAY_BIN':
            env.pop(key)
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', shutil.which('mutter') or 'mutter', '--headless',
            '--wayland', '--no-x11', '--wayland-display', 'studio-test', '--virtual-monitor', '1600x1000',
            '--', sys.executable, str(Path(__file__).resolve()), str(prefix), '--private-root', str(root)],
            cwd=root, env=private, check=True, timeout=200)


if __name__ == '__main__':
    main()
