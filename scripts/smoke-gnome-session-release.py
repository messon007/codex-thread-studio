#!/usr/bin/env python3
"""Installed native session handoff through private Wayland/AT-SPI controls.

Uses deterministic providers and an isolated profile; never contacts a model or
injects input into the user's desktop. No application test hooks are enabled.
"""
import argparse
import ast
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
from private_wayland import OwnedMutterInput, isolated_environment
from native_accessibility import Client

spec = importlib.util.spec_from_file_location('release_scope', Path(__file__).with_name('smoke-gnome-turn-navigation-scope.py'))
scope = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scope)
runtime = scope.runtime
DRAFT = 'Release retains this unsent draft 中文😀 '
QUEUED = 'Queued follow-up must not run during handoff 中文😀'
RELEASE = r'''    elif method=='thread/unsubscribe':
        mode=(root/'release-mode').read_text() if (root/'release-mode').exists() else 'confirmed'
        def release(identity=identity,mode=mode):
            (root/'release-request-held').touch()
            while (root/'hold-release').exists():time.sleep(.02)
            if mode=='error':answer(identity,error='Deterministic release failed 中文😀')
            else:answer(identity,{'status':'unsubscribed' if mode=='confirmed' else 'unexpected-status'})
            (root/'release-acknowledged').touch()
        threading.Thread(target=release,daemon=True).start()
'''


def run(native, root, language):
    assert not any(key.startswith('STUDIO_GNOME_SMOKE') for key in os.environ)
    catalog = json.loads((Path(__file__).resolve().parents[1]/'native/gnome/vendor/i18n/zh-CN.json').read_text()) if language == 'zh-CN' else {}
    tr = lambda value: catalog.get(value, value)

    class LocalizedClient(Client):
        def click(self, name):
            return super().click(tr(name))

        def text(self, value):
            return super().text(tr(value))

    owned = OwnedMutterInput()
    log = (root/'app.log').open('w')
    child = subprocess.Popen([native], cwd=root, env=os.environ, stdout=log, stderr=log)
    client = LocalizedClient(child)
    original = (root/'codex-turns.json').read_bytes()

    def keys(values):
        for key in values:
            owned.call('NotifyKeyboardKeysym', owned.GLib.Variant('(ub)', (key, True)))
        time.sleep(.04)
        for key in reversed(values):
            owned.call('NotifyKeyboardKeysym', owned.GLib.Variant('(ub)', (key, False)))
        time.sleep(.04)

    def control(name):
        return client.find(lambda n: client.showing(n) and n.get_name() == tr(name) and n.get_role_name() in ('button', 'menu item'))

    def sensitive(name, value):
        node = control(name)
        return node and client.sensitive(node) == value

    def blocked_controls():
        # Idle Stop is intentionally hidden by the redesigned composer. A
        # released session must still expose disabled Send and no usable Stop.
        stop = control('Stop')
        return sensitive('Send', False) and (stop is None or not client.sensitive(stop))

    def select(title):
        row = client.wait('session '+title, lambda: client.find(lambda n: client.showing(n) and n.get_role_name() == 'label' and n.get_name().endswith(title)))
        while row.get_role_name() != 'list item':
            row = row.get_parent()
        selection = row.get_parent().get_selection_iface()
        assert selection and client.api.Selection.select_child(selection, row.get_index_in_parent())

    def requests(method):
        return [r for r in runtime.requests(root) if r.get('method') == method and r.get('params', {}).get('threadId') == 'target']

    def handoff(mode='confirmed', held=False):
        (root/'release-mode').write_text(mode)
        (root/'release-request-held').unlink(missing_ok=True)
        (root/'release-acknowledged').unlink(missing_ok=True)
        if held:
            (root/'hold-release').touch()
        client.click('Main menu')
        client.click('Release session to CLI')
        client.wait('unsubscribe sent', lambda: (root/'release-request-held').exists())

    try:
        client.wait('retained conversation', lambda: client.text('Retained source answer'))
        owned.focus_only_window()
        composer = client.wait('composer', lambda: client.find(lambda n: n.get_role_name() == 'text' and n.get_name() == tr('Write a message. Ctrl+Enter sends; Enter starts a new line.')))
        client.edit(composer, DRAFT)
        client.wait('ready before handoff', lambda: sensitive('Send', True))
        client.wait('durable queue loaded', lambda: client.text(QUEUED))
        initial_resumes = len(requests('thread/resume'))
        handoff(held=True)
        client.wait('pending release status', lambda: client.text('Releasing session…'))
        assert sensitive('Send', False) and sensitive('Refresh sessions', False)
        # A delayed acknowledgment must retain its original source, even when
        # another conversation has become selected before it arrives.
        select('Native other')
        client.wait('other conversation', lambda: client.text('Other source answer'))
        client.wait('other source is writable', lambda: sensitive('Send', True))
        (root/'hold-release').unlink()
        client.wait('old-source acknowledgment delivered', lambda: (root/'release-acknowledged').exists())
        # A catalog refresh is a real provider round trip while the other source
        # remains selected; its reply follows the released source's ACK.
        catalogs = sum(r.get('method') == 'thread/list' for r in runtime.requests(root))
        client.click('Refresh sessions')
        client.wait('other-source refresh dispatched', lambda: sum(r.get('method') == 'thread/list' for r in runtime.requests(root)) > catalogs)
        client.wait('other source stays writable', lambda: sensitive('Send', True))
        select('Native target')
        client.wait('confirmed release', lambda: client.text('Session released · Reload to send messages again'))
        client.wait('localized queue release status', lambda: client.text('Session released. Reload before resuming queued messages.'))
        assert client.contents(composer) == DRAFT
        client.wait('released write controls disabled', blocked_controls)
        assert len(requests('thread/unsubscribe')) == 1
        assert len(requests('thread/resume')) == initial_resumes, 'Selecting a released source implicitly resumed it'
        assert client.text('Retained source answer')
        client.click('Resume')
        assert sensitive('Send', False)
        assert len(requests('thread/resume')) == initial_resumes, 'Queue Resume reclaimed a released session'

        # Another client may start working on the released source. Those late
        # notifications must not revive Studio's steer/stop/queue controls.
        (root/'navigation-events.json').write_text(json.dumps([
            {'method':'thread/status/changed','params':{'threadId':'target','status':{'type':'active'}}},
            {'method':'turn/started','params':{'threadId':'target','turn':{'id':'external-turn','status':'inProgress'}}},
        ]))
        client.wait('external notifications emitted', lambda: (root/'navigation-events-emitted').exists())
        # Flush a native action round trip, rather than accepting an emission
        # marker as proof that the frontend consumed all queued notifications.
        client.click('Main menu')
        client.wait('release action remains disabled', lambda: sensitive('Release session to CLI', False))
        keys([0xff1b])
        client.wait('external turn cannot enable write controls', blocked_controls)
        assert client.contents(composer) == DRAFT
        client.click('Refresh sessions')
        client.wait('explicit resume', lambda: len(requests('thread/resume')) == initial_resumes+1)
        client.wait('write controls restored', lambda: sensitive('Send', True))
        assert client.contents(composer) == DRAFT

        for mode in ('error', 'unexpected'):
            handoff(mode)
            client.wait('uncertain release stays blocked', lambda: client.text('Release unconfirmed · Reload to reconnect this session'))
            assert sensitive('Send', False) and client.contents(composer) == DRAFT
            before = len(requests('thread/resume'))
            select('Native other')
            client.wait('other after uncertain handoff', lambda: client.text('Other source answer'))
            select('Native target')
            client.wait('uncertainty retained on return', lambda: client.text('Release unconfirmed · Reload to reconnect this session'))
            assert len(requests('thread/resume')) == before
            client.click('Refresh sessions')
            client.wait('explicit uncertain-release recovery', lambda: len(requests('thread/resume')) == before+1 and sensitive('Send', True))

        # Exercise the production transport's 35-second acknowledgment expiry.
        # A late success after expiry must not undo the uncertainty guard.
        before = len(requests('thread/resume'))
        handoff(held=True)
        client.wait('pending timed handoff', lambda: client.text('Releasing session…'))
        client.wait('transport expiry retains the handoff guard', lambda: client.text('Release unconfirmed · Reload to reconnect this session'), seconds=45)
        assert sensitive('Send', False) and sensitive('Refresh sessions', True)
        (root/'hold-release').unlink()
        client.wait('late acknowledgment after expiry', lambda: (root/'release-acknowledged').exists())
        client.click('Main menu')
        client.wait('late success cannot release the guard', lambda: sensitive('Release session to CLI', False))
        keys([0xff1b])
        assert client.contents(composer) == DRAFT
        assert len(requests('thread/resume')) == before
        client.click('Refresh sessions')
        client.wait('explicit timeout recovery', lambda: len(requests('thread/resume')) == before+1 and sensitive('Send', True))

        assert len(requests('thread/unsubscribe')) == 4
        assert not requests('turn/interrupt') and not runtime.starts(root)
        assert (root/'codex-turns.json').read_bytes() == original
        with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3') as database:
            queued = json.loads(database.execute("SELECT queue_json FROM session_message_queues WHERE session_key='codex:target'").fetchone()[0])
        assert len(queued) == 1 and queued[0]['text'] == QUEUED
        assert client.contents(composer) == DRAFT
        keys([0xffe9, 0xffc1])
        child.wait(timeout=25)
        assert child.returncode == 0
        for name in ('backend.pid', 'opencode.pid'):
            path = root/name
            if path.exists():
                runtime.until(lambda: not Path('/proc', path.read_text().strip()).exists(), seconds=10)
        assert not any(token in (root/'app.log').read_text() for token in ('CRITICAL', 'Gtk-WARNING', 'panicked'))
        result = dict(ok=True, language=language, installedPair=True, applicationHooks=False, privateWayland=True, explicitUnsubscribe=True, sourceScopedAcknowledgment=True, lateNotificationsBlocked=True, explicitReloadResumes=True, uncertainReleaseBlocked=True, acknowledgmentExpiry=True, lateAcknowledgmentBlocked=True, sourceAndDraftPreserved=True, durableQueuePreserved=True, noInference=True, noInterrupt=True, artifacts=str(root))
        (root/'release-result.json').write_text(json.dumps(result, indent=2)+'\n')
        print(json.dumps(result))
    except BaseException:
        (root/'failed-tree.json').write_text(json.dumps(client.describe(), ensure_ascii=False, indent=2))
        states = [dict(name=n.get_name(), sensitive=client.sensitive(n), showing=client.showing(n)) for n in client.tree() if n.get_role_name() in ('button', 'menu item')]
        (root/'failed-controls.json').write_text(json.dumps(states, ensure_ascii=False, indent=2))
        raise
    finally:
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=15)
        owned.close()
        log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--private-root', type=Path)
    parser.add_argument('--language', choices=['en-US', 'zh-CN'], default='en-US')
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    if args.private_root:
        return run(str(args.bundle), args.private_root, args.language)
    artifacts = Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke')
    artifacts.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='session-release-', dir=artifacts))
    runtime.create_profile(root)
    bundle = args.bundle.resolve()
    prefix = root/'installed native 中文 % release'
    subprocess.run(['bash', str(bundle/'install.sh'), str(bundle), str(prefix)], check=True, stdout=subprocess.DEVNULL)
    native = str(prefix/'bin/codex-thread-studio-gnome')
    settings = root/'config/codex-thread-studio/settings.json'
    raw = json.loads(settings.read_text())
    raw['language'] = args.language
    settings.write_text(json.dumps(raw))
    (settings.parent/'gnome.json').write_text(json.dumps(dict(selectedBackend='codex', selectedSession='target', windowWidth=1280, windowHeight=900)))
    turns = {key:[dict(id=key+'-turn',status='completed',items=[dict(id=key+'-user',type='userMessage',content=[dict(type='text',text='Retained source prompt' if key=='target' else 'Other source prompt')]),dict(id=key+'-answer',type='agentMessage',text='Retained source answer' if key=='target' else 'Other source answer')])] for key in ('target','other')}
    (root/'codex-turns.json').write_text(json.dumps(turns))
    with sqlite3.connect(settings.parent/'studio.sqlite3') as database:
        database.execute('CREATE TABLE session_message_queues(session_key TEXT PRIMARY KEY,queue_json TEXT NOT NULL)')
        database.execute('INSERT INTO session_message_queues VALUES(?,?)', ('codex:target',json.dumps([dict(id='retained-queue',text=QUEUED,input=[],createdAt=123)])))
    codex, opencode = scope.providers()
    needle = "    elif method=='thread/list':"
    assert codex.count(needle) == 1
    codex = codex.replace(needle, RELEASE+needle, 1)
    for name, code in [('fake-codex.py',codex), ('fake-opencode.py',opencode)]:
        ast.parse(code)
        path = root/name
        path.write_text(code)
        path.chmod(0o700)
    env = dict(os.environ, CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'), CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'), CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(settings.parent/'backends.json'), CODEX_HOME=str(root/'codex'), XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'), XDG_CACHE_HOME=str(root/'cache'), READER_CACHE_DIR=str(root/'reader-cache'), GDK_BACKEND='wayland', GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix', GTK_A11Y='atspi')
    env.pop('CODEX_THREAD_STUDIO_GATEWAY_BIN', None)
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE'):
            env.pop(key)
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,'--private-root',str(root),'--language',args.language], cwd=root, env=private, check=True, timeout=210)


if __name__ == '__main__':
    main()
