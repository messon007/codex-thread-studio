#!/usr/bin/env python3
"""Installed Favorites: language changes, dirty dialogs, native Copy/save/export.

Uses only private profiles/providers/files, the owned compositor and AT-SPI.
There are no application test hooks, model turns or gateway-location overrides.
"""
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
from private_file_portal import FilePortal
from private_wayland import OwnedMutterInput, isolated_environment
from memory_sampler import ProcessMemory

spec = importlib.util.spec_from_file_location('favorites_runtime', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)
NOTE = 'Note Save Export failed 中文😀 {count}\n'
CONTENT = '# Save\n\nSettings Source Question 中文😀 {count}  \n'


def run(bundle, root):
    owned = OwnedMutterInput()
    owned.enable_clipboard_reader()
    invalid = root / 'Export failed <b>Save & {source}'
    invalid.mkdir()
    exported = root / 'exported favorites 中文.md'
    options = dict(multiple=False, directory=False, current_name='codex-thread-studio-favorites.md')
    portal = FilePortal(root, [
        dict(interface='FileChooser', method='SaveFile', title='导出收藏', options=options, status=1),
        dict(interface='FileChooser', method='SaveFile', title='导出收藏', options=options, uris=[invalid.as_uri()]),
        dict(interface='FileChooser', method='SaveFile', title='导出收藏', options=options, uris=[exported.as_uri()]),
    ])
    native, gateway = bundle / 'codex-thread-studio-gnome', bundle / 'codex-thread-studio-gateway'
    log = (root / 'app.log').open('w')
    child = subprocess.Popen([str(native)], cwd=root, env=os.environ, stdout=log, stderr=log)
    memory = ProcessMemory(child.pid, gateway)

    def iterate():
        portal.iterate()
        if portal.failure:
            raise RuntimeError(('Private portal rejected request', portal.failure))

    desktop = Client(child, iterate=iterate)

    def keys(values):
        for value in values:
            owned.call('NotifyKeyboardKeysym', owned.GLib.Variant('(ub)', (value, True)))
        time.sleep(.06)
        for value in reversed(values):
            owned.call('NotifyKeyboardKeysym', owned.GLib.Variant('(ub)', (value, False)))
        iterate()

    def focus(node):
        for _ in range(80):
            current = desktop.find(lambda n: n.get_state_set().contains(desktop.api.StateType.FOCUSED))
            parent = node.get_parent()
            if current and (desktop.within(current, node) or (parent and parent.get_role_name() == 'list item' and desktop.within(current, parent))):
                return
            keys([0xff09])
        raise RuntimeError(('Native control not keyboard reachable', node.get_name(), node.get_role_name()))

    def activate(node):
        interface = node.get_action_iface()
        names = [desktop.api.Action.get_action_name(interface, i) for i in range(desktop.api.Action.get_n_actions(interface))] if interface else []
        if any(name in names for name in ['click', 'activate']):
            desktop.activate(node)
        else:
            focus(node)
            keys([0xff0d])

    def editor(name):
        return desktop.wait(name, lambda: desktop.find(lambda n: n.get_name() == name and desktop.showing(n) and n.get_editable_text_iface() is not None))

    def language(chinese):
        keys([0xffe3, ord(',')])
        desktop.wait('language controls', lambda: desktop.text('Apply language') or desktop.text('应用语言'))
        combo = desktop.wait('language picker', lambda: desktop.find(lambda n: n.get_role_name() == 'combo box' and n.get_name() in ['Language', '语言'] and desktop.showing(n)))
        focus(combo)
        keys([0x20])
        desktop.wait('language popup', lambda: desktop.text('简体中文') and desktop.text('English'))
        keys([0xff50])
        for _ in range(1 if chinese else 2):
            keys([0xff54])
        keys([0xff0d])
        desktop.click('Apply language' if chinese else '应用语言')
        desktop.wait('persisted locale', lambda: json.loads((root / 'config/codex-thread-studio/settings.json').read_text()).get('language') == ('zh-CN' if chinese else 'en-US'))
        settings=desktop.wait('live Settings title', lambda: desktop.find(lambda n: n.get_role_name() == 'dialog' and n.get_name() == ('设置' if chinese else 'Settings')))
        close=desktop.wait('Settings Close',lambda:desktop.find(lambda n:n.get_name() in ['Close','关闭'] and 'button' in n.get_role_name() and desktop.within(n,settings) and desktop.showing(n) and desktop.sensitive(n)))
        desktop.activate(close)
        desktop.wait('closed settings', lambda: not desktop.find(lambda n: n.get_role_name() == 'dialog' and n.get_name() in ['Settings', '设置'] and desktop.showing(n)))

    def stored():
        with sqlite3.connect(f'file:{root / "config/codex-thread-studio/studio.sqlite3"}?mode=ro', uri=True) as database:
            return database.execute("SELECT title,note,tags_json,content,question,backend,thread_id,turn_id,item_id FROM favorites WHERE id='language-favorite'").fetchone()

    try:
        desktop.wait('conversation', lambda: desktop.text('Native target'))
        owned.focus_only_window()
        desktop.click('Main menu')
        desktop.click('Favorites…')
        desktop.wait('Favorites catalog', lambda: desktop.text('2 of 2 favorites'))
        row = desktop.wait('literal favorite row', lambda: desktop.find(lambda n: n.get_role_name() == 'list item' and n.get_name() == 'Save'))
        activate(row)
        desktop.wait('favorite editor', lambda: desktop.contents(editor('Favorite title')) == 'Save')
        original = stored()
        desktop.edit(editor('Search favorites'), 'Save')
        desktop.wait('filtered counter', lambda: desktop.text('1 of 2 favorites'))
        desktop.edit(editor('Favorite title'), 'Send')
        desktop.edit(editor('Favorite tags'), 'Save, Settings, Save')
        desktop.edit(editor('Favorite note'), NOTE)
        library = desktop.wait('Favorites dialog', lambda: desktop.find(lambda n: n.get_role_name() == 'dialog' and n.get_name() == 'Favorites'))
        close = desktop.wait('library Close', lambda: desktop.find(lambda n: n.get_name() == 'Close' and 'button' in n.get_role_name() and desktop.within(n, library)))
        desktop.activate(close)
        desktop.wait('dirty close prompt', lambda: desktop.text('Save favorite changes?'))
        desktop.click('Cancel')
        assert stored() == original, 'Cancelled close wrote unsaved metadata'
        desktop.click('Save')
        desktop.wait('explicit English save', lambda: stored()[:3] == ('Send', NOTE, '["Save","Settings"]'))
        first_saved = stored()
        assert first_saved[3:] == original[3:]
        desktop.activate(close)
        desktop.wait('closed English library', lambda: not desktop.find(lambda n:n.get_role_name()=='dialog' and n.get_name()=='Favorites' and desktop.showing(n)))
        language(True)
        desktop.click('主菜单')
        desktop.click('收藏…')
        row = desktop.wait('saved literal row', lambda: desktop.find(lambda n:n.get_role_name()=='list item' and n.get_name()=='Send'))
        activate(row)
        assert desktop.contents(editor('收藏标题')) == 'Send'
        assert desktop.contents(editor('收藏标签')) == 'Save, Settings'
        assert desktop.contents(editor('收藏笔记')) == NOTE
        desktop.edit(editor('搜索收藏'), 'Save')
        desktop.wait('translated filtered counter', lambda: desktop.text('显示 1 / 2 条收藏'))
        assert desktop.contents(editor('搜索收藏')) == 'Save'
        assert stored() == first_saved, 'Locale change translated favorite data'
        desktop.click('源码')
        desktop.wait('literal Source content', lambda: desktop.text('Settings Source Question 中文😀 {count}'))
        desktop.click('预览')
        desktop.edit(editor('收藏笔记'), NOTE+'UNSAVED\n')
        desktop.click('打开来源')
        desktop.wait('translated dirty source prompt', lambda: desktop.text('打开来源会话？'))
        desktop.click('取消')
        desktop.click('将全部收藏导出为 Markdown')
        desktop.wait('export cancellation', lambda: len(portal.calls) == 1 and desktop.find(lambda n: n.get_name() == '将全部收藏导出为 Markdown' and desktop.sensitive(n)))
        assert not exported.exists() and stored() == first_saved
        desktop.click('将全部收藏导出为 Markdown')
        desktop.wait('native export failure', lambda: desktop.text('导出失败') and desktop.text(str(invalid)))
        desktop.wait('literal failure destination', lambda: desktop.text('目标：' + str(invalid)))
        desktop.click('关闭')
        desktop.click('将全部收藏导出为 Markdown')
        desktop.wait('saved export', lambda: exported.exists() and NOTE in exported.read_text())
        assert 'UNSAVED' not in exported.read_text() and stored() == first_saved, 'Export included unsaved metadata'
        desktop.click('保存')
        desktop.wait('explicit Chinese save', lambda: stored()[:3] == ('Send', NOTE+'UNSAVED\n', '["Save","Settings"]'))
        saved = stored()
        assert saved[3:] == original[3:], 'Saving metadata changed source identity or response bytes'
        desktop.click('复制收藏')
        copied = desktop.wait('independent clipboard', owned.clipboard_text)
        assert NOTE in copied and CONTENT.strip() in copied
        desktop.click('移除收藏')
        desktop.wait('remove prompt', lambda: desktop.text('移除此收藏？'))
        desktop.click('取消')
        assert stored() == saved, 'Cancelled removal changed favorite'
        library = desktop.wait('Favorites dialog', lambda: desktop.find(lambda n: n.get_role_name() == 'dialog' and n.get_name() == '收藏'))
        desktop.activate(desktop.wait('library Close', lambda: desktop.find(lambda n: n.get_name() == 'Close' and 'button' in n.get_role_name() and desktop.within(n, library))))
        desktop.wait('closed Favorites', lambda: not desktop.find(lambda n: n.get_role_name() == 'dialog' and n.get_name() == '收藏' and desktop.showing(n)))
        language(False)
        keys([0xffe9, 0xffc1])
        child.wait(timeout=20)
        assert child.returncode == 0
    except BaseException:
        (root / 'accessibility.json').write_text(json.dumps(desktop.describe(), ensure_ascii=False))
        raise
    finally:
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=10)
        measured = memory.close()
        portal.close()
        owned.close()
        log.close()
    assert len(portal.calls) == 3 and not portal.plan and not portal.failure
    assert not runtime.starts(root), 'Favorites workflow started model inference'
    assert not any(token in (root / 'app.log').read_text() for token in ['CRITICAL', 'panicked', 'Gtk-WARNING'])
    for name in ['backend.pid', 'opencode.pid']:
        if (root / name).exists():
            assert not Path('/proc', (root / name).read_text().strip()).exists(), 'Owned provider survived'
    result = dict(ok=True, installedPair=True, applicationHooks=False, gatewayOverride=False, nativeFavoritesLanguage=True,
                  bilingualDirtyDialogs=True, literalMetadata=True, filteredCounter=True, previewSource=True,
                  sourceDiscardGuard=True, nativeExportCancelFailureSuccess=True, localizedExportFailure=True,
                  explicitSave=True, sourceIdentityPreserved=True, independentClipboard=True, cancelRemoval=True,
                  modelTurns=0, memory=measured, artifacts=str(root))
    (root / 'favorites-language.json').write_text(json.dumps(result, indent=2))
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
    base = Path(__file__).resolve().parents[1] / 'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='release-favorites-', dir=base))
    runtime.create_profile(root)
    runtime.fixture.BACKEND, runtime.fixture.OPENCODE_BACKEND = runtime.CODEX, runtime.OPENCODE
    api = runtime.fixture.Gateway(bundle / 'codex-thread-studio-gateway', root, 'codex')
    try:
        preferences = api.preferences()
        preferences['language'] = 'en-US'
        api.settings.write_text(json.dumps(preferences))
        for index, title in enumerate(['Save', 'Settings']):
            favorite = dict(id='language-favorite' if index == 0 else 'literal-settings', scope='message', backend='codex',
                            threadId='target', threadTitle='Settings', projectPath=str(root / 'project'), turnId='private-turn',
                            itemId='private-answer-' + str(index), title=title, question='Question Source 中文😀',
                            content=CONTENT if index == 0 else '# Settings\n\nOther literal response\n',
                            note='Original note Save' if index == 0 else 'Other literal note', tags=['Settings'], createdAt='2026-10-26T00:00:00Z')
            status, value = api.api('/studio/favorites', favorite, method='POST')
            assert status == 201, (status, value)
    finally:
        api.close()
    for name, content in [('fake-codex.py', runtime.CODEX), ('fake-opencode.py', runtime.OPENCODE)]:
        script = root / name
        script.write_text(content)
        script.chmod(0o700)
    (root / 'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex', selectedSession='target')))
    env = dict(os.environ, CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root / 'config/codex-thread-studio/backends.json'),
               CODEX_THREAD_STUDIO_CODEX_BIN=str(root / 'fake-codex.py'), CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root / 'fake-opencode.py'),
               CODEX_HOME=str(root / 'codex'), XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'),
               XDG_CACHE_HOME=str(root / 'cache'), READER_CACHE_DIR=str(root / 'reader-cache'), GDK_BACKEND='wayland',
               GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix', GTK_A11Y='atspi')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE') or key == 'CODEX_THREAD_STUDIO_GATEWAY_BIN':
            env.pop(key)
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', shutil.which('mutter') or 'mutter', '--headless', '--wayland', '--no-x11',
                        '--wayland-display', 'studio-test', '--virtual-monitor', '1600x1000', '--', sys.executable,
                        str(Path(__file__).resolve()), str(bundle), '--private-root', str(root)],
                       cwd=root, env=private, check=True, timeout=200)


if __name__ == '__main__':
    main()
