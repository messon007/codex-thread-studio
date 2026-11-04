#!/usr/bin/env python3
"""Actual OpenCode, shared loopback model, and installed native Wayland controls.

The outer process creates private network/PID/filesystem and desktop namespaces.
No login profile, real model credential, external network, or app hooks are used.
Node, Python/GI and Mutter are acceptance tooling, not application dependencies.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import queue
import resource
import shutil
import subprocess
import sys
import tempfile
import threading
import time

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / 'scripts/test-support'))
from native_accessibility import Client
from private_wayland import OwnedMutterInput, isolated_environment


def native_case(bundle, root):
    # This guard runs before loading accessibility or starting any application.
    if (os.environ.get('STUDIO_REAL_OPENCODE_NAMESPACE') != str(root)
            or os.environ.get('XDG_CONFIG_HOME') != str(root / 'config')
            or any(k.startswith('STUDIO_GNOME_SMOKE') for k in os.environ)):
        raise RuntimeError('Real OpenCode acceptance requires its private desktop/profile')
    owned = OwnedMutterInput()
    node = child = output_thread = None
    node_log = (root / 'provider.log').open('w')
    app_logs = []
    result = {}
    try:
        node = subprocess.Popen([os.environ['STUDIO_NODE_BIN'], str(REPO / 'scripts/smoke-opencode-session.mjs'),
                                 '--native-fixture', str(root)], stdin=subprocess.PIPE,
                                stdout=subprocess.PIPE, stderr=node_log, text=True)
        lines = queue.Queue()

        def output():
            for line in node.stdout:
                node_log.write(line)
                node_log.flush()
                lines.put(line)
        output_thread = threading.Thread(target=output, daemon=True)
        output_thread.start()
        ready = None
        deadline = time.monotonic() + 65
        while time.monotonic() < deadline:
            if node.poll() is not None:
                raise RuntimeError(('Shared real OpenCode fixture exited', node.returncode))
            try:
                value = json.loads(lines.get(timeout=.2))
                if value.get('nativeFixtureReady'):
                    ready = value
                    break
            except queue.Empty:
                continue
        assert ready, 'Shared real OpenCode fixture did not become ready'
        (root / 'provider-ready.json').write_text(json.dumps(ready, indent=2))
        profile = Path(ready['temporary'])
        assert profile.parent == root and profile.name.startswith('studio-opencode-smoke-')
        config = profile / 'config/codex-thread-studio'
        config.mkdir(exist_ok=True)
        (config / 'settings.json').write_text(json.dumps({'language': 'en-US', 'queueDepth': 1}))
        (config / 'backends.json').write_text('{"version":1,"backends":[]}')
        (config / 'gnome.json').write_text(json.dumps({
            'selectedBackend': 'opencode', 'selectedSession': ready['threadId']}))
        spec = importlib.util.spec_from_file_location('shared_gateway_fixture', REPO / 'scripts/smoke-gnome-router.py')
        fixture = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(fixture)
        fake = root / 'unused-codex.py'
        fake.write_text(fixture.BACKEND)
        fake.chmod(0o700)
        env = dict(os.environ, **ready['environment'])
        env.update(CODEX_HOME=str(profile / 'codex'),
                   CODEX_THREAD_STUDIO_CODEX_BIN=str(fake),
                   CODEX_THREAD_STUDIO_CODEX_CONNECTION='embedded',
                   CODEX_THREAD_STUDIO_OPENCODE_BIN=os.environ['STUDIO_OPENCODE_BIN'],
                   CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(config / 'backends.json'),
                   READER_CACHE_DIR=str(profile / 'reader-cache'))
        env.pop('CODEX_THREAD_STUDIO_GATEWAY_BIN', None)
        expected_children = {Path(env['CODEX_THREAD_STUDIO_OPENCODE_BIN']).resolve(),
                             (bundle / 'codex-thread-studio-gateway').resolve()}
        observed_children = []
        (root / 'desktop-environment.json').write_text(json.dumps({
            'variables': {k: env.get(k) for k in ['WAYLAND_DISPLAY', 'XDG_RUNTIME_DIR', 'GDK_BACKEND']},
            'runtimeEntries': [p.name for p in Path(env['XDG_RUNTIME_DIR']).iterdir()],
        }, indent=2))

        def keys(values):
            for k in values:
                owned.call('NotifyKeyboardKeysym', owned.GLib.Variant('(ub)', (k, True)))
            time.sleep(.04)
            for k in reversed(values):
                owned.call('NotifyKeyboardKeysym', owned.GLib.Variant('(ub)', (k, False)))
            time.sleep(.04)

        def requests():
            return [json.loads(line) for line in (root / 'model-requests.jsonl').read_text().splitlines()]

        def launch(phase):
            log = (root / (phase + '.log')).open('w')
            app_logs.append(log)
            process = subprocess.Popen([str(bundle / 'codex-thread-studio-gnome')], cwd=ready['work'],
                                       env=env, stdout=log, stderr=log)
            return process, Client(process)

        def composer():
            return client.wait('OpenCode composer', lambda: client.find(lambda n:
                n.get_role_name() == 'text' and n.get_name().startswith('Write a message.')))

        def usable_send():
            return client.find(lambda n: n.get_name() == 'Send' and n.get_role_name() == 'button'
                               and client.showing(n) and client.sensitive(n))

        def send(prompt):
            before = len(requests())
            client.edit(composer(), prompt)
            client.wait('Send ready without Stop', usable_send)
            client.click('Send')
            client.wait('authoritative busy controls', lambda: client.find(lambda n:
                n.get_name() == 'Stop' and n.get_role_name() == 'button'
                and client.showing(n) and client.sensitive(n)))
            prefix = f'local OpenCode smoke reply {before + 1} '
            client.wait('live streamed prefix', lambda: client.contains_text(prefix))
            client.wait('complete streamed reply', lambda: client.contains_text(prefix + '中文😀'))
            client.wait('idle Send restored without manual Stop', usable_send)
            after = requests()
            assert len(after) == before + 1, 'Unexpected extra or replayed model request'
            assert after[-1]['body']['model'] == 'smoke'
            content = after[-1]['body']['messages'][-1]['content']
            if isinstance(content, list):
                assert all(part.get('type') == 'text' for part in content)
                content = ''.join(part['text'] for part in content)
            assert content == prompt
            assert client.contents(composer()) == ''
            return prefix + '中文😀'

        def provider_processes():
            found = []
            for process in Path('/proc').iterdir():
                if not process.name.isdecimal():
                    continue
                try:
                    target = (process / 'exe').resolve(strict=True)
                    if target in expected_children:
                        state = (process / 'stat').read_text().rsplit(')', 1)[1].split()[0]
                        if state != 'Z':
                            found.append({'pid': int(process.name), 'executable': str(target)})
                except (OSError, ValueError):
                    pass
            return found

        def close(process):
            children = provider_processes()
            assert {Path(row['executable']) for row in children} == expected_children, 'Gateway/OpenCode ownership was not observed'
            observed_children.append(children)
            owned.focus_only_window()
            keys([0xffe9, 0xffc1])  # Alt+F4: normal desktop window close.
            process.wait(timeout=20)
            assert process.returncode == 0
            end = time.monotonic() + 10
            while provider_processes() and time.monotonic() < end:
                time.sleep(.1)
            assert not provider_processes(), 'Native gateway/OpenCode children survived window close'

        child, client = launch('native')
        client.wait('seed history from actual OpenCode', lambda: client.contains_text(
            f'local OpenCode smoke reply {ready["modelRequests"]} 中文😀'))
        owned.focus_only_window()
        first = send('First native request 中文😀')
        second = send('Second native request without manual Stop 中文😀')
        draft = 'Preserved draft while refreshing real OpenCode 中文😀 '
        client.edit(composer(), draft)
        client.click('Refresh sessions')
        client.wait('refreshed exact history', lambda: client.contains_text(second))
        client.wait('refreshed composer', lambda: client.contents(composer()) == draft)
        count = len(requests())
        close(child)
        child, client = launch('restart')
        client.wait('restarted persisted history', lambda: client.contains_text(second))
        client.wait('restarted idle composer', usable_send)
        assert len(requests()) == count, 'Restart replayed a model request'
        third = send('After native restart 中文😀')
        close(child)
        result = dict(ok=True, installedPair=True, gatewayOverride=False, applicationHooks=False,
                      realOpenCode=ready['version'], externalModel=False, privateNetwork=True,
                      baselineWebAdapter=True, seedHistory=True, liveSse=True,
                      consecutiveTurnsWithoutStop=True, draftAcrossRefresh=True,
                      restartHistory=True, restartNoReplay=True, modelRequests=len(requests()),
                      exactPrompts=True, ownedChildrenReaped=True, observedOwnedChildren=observed_children,
                      replies=[first, second, third])
    except BaseException:
        if child is not None and child.poll() is None:
            try:
                (root / 'failure-tree.json').write_text(json.dumps(Client(child).describe(), ensure_ascii=False))
            except BaseException:
                pass
        raise
    finally:
        if child is not None and child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=15)
            except subprocess.TimeoutExpired:
                child.kill(); child.wait(timeout=5)
        if node is not None:
            if node.poll() is None:
                node.stdin.close()
            try:
                node.wait(timeout=10)
            except subprocess.TimeoutExpired:
                node.kill(); node.wait(timeout=5)
        if output_thread is not None:
            output_thread.join(timeout=2)
        for log in app_logs:
            log.close()
        node_log.close()
        owned.close()
    (root / 'result.json').write_text(json.dumps(result, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps(dict(result, artifacts=str(root)), ensure_ascii=False))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('bundle', type=Path)
    p.add_argument('--opencode', type=Path)
    p.add_argument('--private-root', type=Path, help=argparse.SUPPRESS)
    p.add_argument('--sandbox-root', type=Path, help=argparse.SUPPRESS)
    args = p.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    bundle = args.bundle.resolve(strict=True)
    if args.private_root:
        return native_case(bundle, args.private_root.resolve(strict=True))
    if args.sandbox_root:
        root = args.sandbox_root.resolve(strict=True)
        if (os.environ.get('STUDIO_REAL_OPENCODE_NAMESPACE') != str(root)
                or not (root / 'tools/node').is_file()
                or not (root / 'tools/opencode').is_file()):
            raise RuntimeError('Real OpenCode acceptance requires its private desktop/profile')
        prefix = root / 'installed native 中文'
        with (root / 'install.log').open('w') as log:
            subprocess.run(['bash', str(bundle / 'install.sh'), str(bundle), str(prefix)],
                           stdout=log, stderr=subprocess.STDOUT, check=True, timeout=60)
        bundle = prefix / 'bin'
        env = dict(os.environ, XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'),
                   XDG_CACHE_HOME=str(root / 'cache'), GTK_A11Y='atspi', GDK_BACKEND='wayland',
                   GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix')
        # The host security control filesystem is deliberately read-only here.
        # Configure only this private bus; do not query/change the host policy.
        bus_config = root / 'private-bus.conf'
        bus_config.write_text('<busconfig><include>/usr/share/dbus-1/session.conf</include>'
                              '<apparmor mode="disabled"/></busconfig>\n')
        with isolated_environment(env, headless=True) as private:
            subprocess.run(['dbus-run-session', '--config-file=' + str(bus_config), '--',
                            'mutter', '--headless', '--wayland', '--no-x11',
                            '--wayland-display', 'studio-test', '--virtual-monitor', '1600x1000', '--',
                            sys.executable, str(Path(__file__).resolve()), str(bundle),
                            '--private-root', str(root)], cwd=root, env=private, check=True, timeout=240)
        return
    base = Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or REPO / 'target/gnome-native-smoke')
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='real-opencode-', dir=base)).resolve()
    for name in ['home', 'tmp', 'config', 'data', 'cache', 'tools']:
        (root / name).mkdir()
    node = Path(shutil.which('node')).resolve(strict=True)
    opencode = (args.opencode or Path(shutil.which('opencode'))).resolve(strict=True)
    with opencode.open('rb') as source:
        assert source.read(4) == b'\x7fELF', 'Use the installed standalone Linux OpenCode executable'
    accessibility_config = Path('/usr/share/defaults/at-spi2/accessibility.conf')
    private_accessibility = root / 'private-accessibility.conf'
    raw = accessibility_config.read_text()
    assert raw.count('</busconfig>') == 1
    private_accessibility.write_text(raw.replace('</busconfig>', '<apparmor mode="disabled"/></busconfig>'))
    cmd = ['bwrap', '--die-with-parent', '--unshare-pid', '--unshare-net', '--ro-bind', '/', '/',
           '--proc', '/proc', '--dev', '/dev', '--tmpfs', '/dev/shm', '--tmpfs', '/home', '--tmpfs', '/tmp',
           '--ro-bind', str(REPO), str(REPO), '--bind', str(root), str(root),
           '--ro-bind', str(node), str(root / 'tools/node'),
           '--ro-bind', str(opencode), str(root / 'tools/opencode'),
           '--ro-bind', str(private_accessibility), str(accessibility_config),
           '--chdir', str(root), '--clearenv']
    env = dict(HOME=str(root / 'home'), TMPDIR=str(root / 'tmp'), PATH=str(root / 'tools') + ':/usr/bin:/bin',
               LANG='C.UTF-8', XDG_DATA_DIRS='/usr/local/share:/usr/share',
               STUDIO_NODE_BIN=str(root / 'tools/node'), STUDIO_OPENCODE_BIN=str(root / 'tools/opencode'),
               STUDIO_REAL_OPENCODE_NAMESPACE=str(root))
    for name, value in env.items():
        cmd.extend(['--setenv', name, value])
    cmd.extend(['--', '/usr/bin/python3', str(Path(__file__).resolve()), str(bundle), '--sandbox-root', str(root)])
    (root / 'command.json').write_text(json.dumps(cmd, indent=2))
    with (root / 'desktop.log').open('w') as log:
        completed = subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, timeout=280)
    print(json.dumps({'artifacts': str(root), 'exitCode': completed.returncode}))
    raise SystemExit(completed.returncode)


if __name__ == '__main__':
    main()
