#!/usr/bin/env python3
"""Packaged large Markdown through installed GNOME controls, never app hooks.

Only copied/generated documents and fake providers are used. Input and clipboard
are restricted to the owned compositor; the ordinary desktop entry opens files.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import resource
import shutil
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / 'scripts/test-support'))
from native_accessibility import Client
from private_wayland import OwnedMutterInput, isolated_environment
from memory_sampler import ProcessMemory

spec = importlib.util.spec_from_file_location('large_release_runtime', REPO / 'scripts/smoke-gnome-router-runtime.py')
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)
spec = importlib.util.spec_from_file_location('large_release_outputs', REPO / 'scripts/smoke-gnome-outputs.py')
outputs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(outputs)
DRAFT = 'Unsent installed large-reader draft 中文😀 '
FINAL = 'Unique final context 中文😀.'
SAVED = FINAL + ' Explicitly saved.'


def digest(path, replacement=None):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for line in stream:
            value.update(line.replace(*replacement) if replacement else line)
    return value.hexdigest()


def headings(path):
    with path.open(encoding='utf-8') as stream:
        return [(line, text.rstrip('\r\n').removeprefix('## '))
                for line, text in enumerate(stream, 1) if text.startswith('## ')]


def generate(path):
    with path.open('wb') as stream:
        stream.write(b'# Installed large Markdown\n\nBounded native reading.\n\n## Wide table\n\n')
        stream.write(('| ' + ' | '.join(f'Field {n:02}' for n in range(12)) + ' |\n').encode())
        stream.write(('|' + '---|' * 12 + '\n').encode())
        rows = 0
        while stream.tell() <= 32 * 1024 * 1024:
            cells = [f'row-{rows:06}-col-{col:02} ' + 'context 中文 ' * 8 for col in range(12)]
            stream.write(('| ' + ' | '.join(cells) + ' |\n').encode())
            rows += 1
        stream.write(('\n## Final section\n\n' + FINAL + '\n').encode())
    return rows


def run(bundle, root):
    import gi
    gi.require_version('GioUnix', '2.0')
    from gi.repository import Gio, GioUnix
    prefix = root / 'installed large 中文 % test'
    native = prefix / 'bin/codex-thread-studio-gnome'
    gateway = prefix / 'bin/codex-thread-studio-gateway'
    metadata = json.loads((root / 'fixtures.json').read_text())
    controlled = OwnedMutterInput()
    controlled.enable_clipboard_reader()
    log = (root / 'app.log').open('w')
    child = subprocess.Popen([str(native), str(root / 'project/reference.md')], cwd=root,
                             env=os.environ, stdout=log, stderr=log)
    memory = ProcessMemory(child.pid, gateway, include_web_content=True)
    desktop = Client(child)
    step = 'startup'
    measurements = []

    def wait(name, test, seconds=40):
        nonlocal step
        step = name
        (root / 'step.txt').write_text(name)
        return desktop.wait(name, test, seconds)

    def button(name):
        return desktop.find(lambda node: node.get_name() == name and desktop.showing(node)
                            and desktop.sensitive(node) and 'button' in node.get_role_name())

    def ready():
        return button('Preview') and button('Source') and button('Edit')

    def status():
        return desktop.find(lambda node: node.get_role_name() == 'label' and desktop.showing(node)
                            and re.fullmatch(r'\d+ lines · [\d.]+ MB · Line \d+(?: · Unsaved)?', node.get_name()))

    def editor(editable, marker):
        return desktop.find(lambda node: node.get_role_name() == 'text' and desktop.showing(node)
                            and node.get_state_set().contains(desktop.api.StateType.EDITABLE) == editable
                            and marker in (desktop.contents(node) or ''))

    def outline(title, line):
        # Each call opens and closes the pane. Virtualized outline rows outside
        # the viewport are deliberately absent from the accessibility tree.
        desktop.click('Contents and search')
        def item():
            node = desktop.find(lambda node: node.get_name() == title and node.get_role_name() == 'label' and desktop.showing(node)
                                and node.get_parent() and node.get_parent().get_role_name() == 'list item')
            return node.get_parent() if node else None
        label = wait('realized outline row', lambda: desktop.find(lambda node:
            desktop.showing(node) and node.get_role_name() == 'label'
            and node.get_name() in metadata[filename]['headingLabels']
            and node.get_parent() and node.get_parent().get_role_name() == 'list item'))
        outline_list = label.get_parent().get_parent()
        for _ in range(80):
            if desktop.find(lambda node: node.get_state_set().contains(desktop.api.StateType.FOCUSED)
                            and desktop.within(node, outline_list)):
                break
            keys([0xff09])
        else:
            raise RuntimeError('Owned outline list was not keyboard reachable')
        keys([0xff50])  # Home, then native Down realizes and selects model rows.
        for _ in range(512):
            row = item()
            if row and row.get_state_set().contains(desktop.api.StateType.SELECTED):break
            keys([0xff54])
        else:
            raise RuntimeError(('Native outline heading was not selected', title))
        with (root/'outline-actions.jsonl').open('a') as log:
            log.write(json.dumps(dict(title=title,line=line,nativeKeyboard=True),ensure_ascii=False)+'\n')
        wait('outline destination source line', lambda: ready() and status()
             and status().get_name().endswith(f'Line {line}'))
        desktop.click('Contents and search')

    def keys(values):
        for value in values:
            controlled.call('NotifyKeyboardKeysym', controlled.GLib.Variant('(ub)', (value, True)))
        time.sleep(.06)
        for value in reversed(values):
            controlled.call('NotifyKeyboardKeysym', controlled.GLib.Variant('(ub)', (value, False)))

    def text_action(node, name):
        interface = node.get_action_iface()
        names = [desktop.api.Action.get_action_name(interface, n)
                 for n in range(desktop.api.Action.get_n_actions(interface))]
        assert desktop.api.Action.do_action(interface, names.index(name))

    def table_range():
        label = desktop.find(lambda node: node.get_role_name() == 'label' and desktop.showing(node)
                             and re.fullmatch(rf"\d+–\d+ of {metadata['rows']} rows", node.get_name()))
        return list(map(int, re.match(r'(\d+)–(\d+)', label.get_name()).groups())) if label else None

    def preview_text(value):
        return desktop.find(lambda node: desktop.showing(node) and node.get_role_name() == 'text'
                            and node.get_name() == 'Preview' and value in (desktop.contents(node) or '')) or desktop.text(value)

    def uncheck_column(name):
        check = wait('column checkbox: ' + name, lambda: desktop.find(lambda node:
            node.get_name() == name and node.get_role_name() == 'check box' and desktop.showing(node)))
        # GtkCheckButton does not expose a named click action in this GTK build.
        # Use ordinary Tab/Space on the verified owned compositor instead.
        for _ in range(24):
            focused = desktop.find(lambda node: node.get_state_set().contains(desktop.api.StateType.FOCUSED))
            parent = check.get_parent()
            if focused and (desktop.within(focused, check)
                            or (parent and parent.get_role_name() == 'list item' and desktop.within(focused, parent))):
                keys([0x20])
                return
            keys([0xff09])
        raise RuntimeError(('Column checkbox is not keyboard reachable', name))

    try:
        started = time.monotonic()
        wait('selected fake conversation', lambda: desktop.text('Native target'))
        controlled.focus_only_window()
        composer = wait('composer', lambda: desktop.find(lambda node:
            node.get_name() == 'Write a message. Ctrl+Enter sends; Enter starts a new line.'
            and desktop.showing(node) and node.get_editable_text_iface()))
        desktop.edit(composer, DRAFT)
        entry = GioUnix.DesktopAppInfo.new_from_filename(str(
            prefix / 'share/applications/io.github.messon007.codexthreadstudio.Gnome.desktop'))
        assert entry
        for filename, title, writable in [('reference.md', metadata['referenceTitle'], False),
                                          ('large.md', 'Installed large Markdown', True),
                                          ('large-crlf.md', 'Installed large Markdown', True)]:
            path = root / 'project' / filename
            if writable:
                started = time.monotonic()
                assert entry.launch([Gio.File.new_for_path(str(path))], None)
            wait('initial bounded Preview: ' + filename, lambda: ready() and status()
                 and preview_text(title) and desktop.find(lambda node: desktop.showing(node)
                 and node.get_role_name() == 'label' and node.get_name() == filename), seconds=60)
            continuous = preview_text(title).get_role_name() == 'text'
            opened = round((time.monotonic() - started) * 1000)
            desktop.click('Source')
            source = wait('bounded initial Source', lambda: ready() and editor(False, '# ' + title))
            text = desktop.contents(source)
            if filename == 'large-crlf.md':
                assert '\r\n' in text, 'Source normalized the saved CRLF bytes'
            assert 0 < len(text.encode()) < 1024 * 1024, 'Source loaded the whole large document'
            assert path.stat().st_size > 4 * 1024 * 1024 and digest(path) == metadata[filename]['sha256']
            desktop.click('Preview')
            wait('Preview restored', lambda: ready() and status())
            heading, line = metadata[filename]['lastHeading']
            jump_start = time.monotonic()
            outline(heading, line)
            jump_ms = round((time.monotonic() - jump_start) * 1000)
            desktop.click('Source')
            tail = wait('distant Source section', lambda: ready() and editor(False, '## ' + heading))
            tail_text = desktop.contents(tail)
            if filename == 'large-crlf.md':
                assert '\r\n' in tail_text, 'Distant Source normalized CRLF'
            assert len(tail_text.encode()) < 1024 * 1024
            if writable:
                assert FINAL in tail_text
                interface = tail.get_text_iface()
                begin = tail_text.index(FINAL)
                if desktop.api.Text.get_n_selections(interface):
                    assert desktop.api.Text.set_selection(interface, 0, begin, begin + len(FINAL))
                else:
                    assert desktop.api.Text.add_selection(interface, begin, begin + len(FINAL))
                text_action(tail, 'clipboard.copy')
                wait('independent Source clipboard', lambda: controlled.clipboard_text() == FINAL)
                expected = digest(path, (FINAL.encode(), SAVED.encode()))
                desktop.click('Edit')
                edit = wait('bounded distant Edit', lambda: ready() and editor(True, FINAL))
                edit_text = desktop.contents(edit)
                assert len(edit_text.encode()) < 1024 * 1024
                added = SAVED.removeprefix(FINAL)
                assert desktop.api.EditableText.insert_text(edit.get_editable_text_iface(),
                    edit_text.index(FINAL) + len(FINAL), added, len(added.encode()))
                updated = edit_text.replace(FINAL, SAVED)
                wait('normal native insertion', lambda: desktop.contents(edit) == updated)
                assert desktop.api.Text.set_caret_offset(edit.get_text_iface(), len(updated))
                assert desktop.api.Text.get_caret_offset(edit.get_text_iface()) == len(updated)
                desktop.click('Source')
                original_source = wait('dirty Source switch', lambda: ready() and editor(False, FINAL))
                assert SAVED not in desktop.contents(original_source), 'Source displayed unsaved edits'
                assert digest(path) == metadata[filename]['sha256'], 'View switch implicitly saved'
                desktop.click('Edit')
                wait('retained page edits', lambda: ready() and editor(True, SAVED))
                desktop.click('Save current page (Ctrl+S)')
                wait('explicit paged save', lambda: ready() and digest(path) == expected)
                desktop.click('Source')
                saved_source = wait('saved EOF Source context', lambda: ready() and editor(False, SAVED))
                if filename == 'large-crlf.md':
                    assert '\r\n' in desktop.contents(saved_source), 'Edit snapshot normalized raw Source'
                desktop.click('Edit')
                retained = wait('saved EOF Edit context', lambda: ready() and editor(True, SAVED))
                text_action(retained, 'text.undo')
                wait('EOF undo after save', lambda: desktop.contents(retained) == edit_text)
                assert digest(path) == expected, 'Undo wrote the saved document'
                text_action(retained, 'text.redo')
                wait('EOF redo after save', lambda: desktop.contents(retained) == updated)
                desktop.click('Preview')
                wait('saved reading view', lambda: ready() and preview_text(SAVED))
                outline('Wide table', metadata[filename]['tableLine'])
                first_rows = wait('bounded first table page', table_range)
                assert first_rows[0] == 1 and first_rows[1] <= 50 and first_rows[1] * 12 <= 512
                desktop.click('Next table page')
                next_rows = wait('next table page', lambda: table_range()
                                 if table_range() and table_range()[0] == first_rows[1] + 1 else None)
                desktop.click('Columns 12/12')
                uncheck_column('Field 01')
                keys([0xff1b])
                wait('selected columns applied', lambda: desktop.text('Columns 11/12')
                     and button('Next table page'))
                assert table_range()[0] == next_rows[0], 'Column filtering reset the table position'
            else:
                assert digest(path) == metadata[filename]['sha256']
            assert desktop.contents(composer) == DRAFT
            measurements.append(dict(file=filename, nativeContinuousPreview=continuous, bytes=path.stat().st_size,
                                     openObservedMs=opened, outlineObservedMs=jump_ms,
                                     sourceWindowBytes=len(text.encode()), distantSourceBytes=len(tail_text.encode()),
                                     exactDestinationLine=line, explicitPagedEdit=writable))
        controlled.focus_only_window()
        keys([0xffe9, 0xffc1])
        child.wait(timeout=20)
        assert child.returncode == 0
    except BaseException:
        (root / 'failed-step.txt').write_text(step)
        (root / 'accessibility.json').write_text(json.dumps(desktop.describe(), ensure_ascii=False))
        pages = []
        for node in desktop.tree():
            if node.get_role_name() == 'text':
                pages.append(dict(name=node.get_name(), showing=desktop.showing(node),
                                  editable=node.get_state_set().contains(desktop.api.StateType.EDITABLE),
                                  text=(desktop.contents(node) or '')[:1024 * 1024]))
        (root / 'failed-text-pages.json').write_text(json.dumps(pages, ensure_ascii=False))
        raise
    finally:
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=10)
        measured = memory.close()
        controlled.close()
        log.close()
    assert not runtime.starts(root), 'Reading started a model turn'
    assert measured['webContentProcessesObserved'] == 0, 'Native Markdown started a web content engine'
    assert not any(token in (root / 'app.log').read_text() for token in ['CRITICAL', 'panicked', 'Gtk-WARNING'])
    for name in ['backend.pid', 'opencode.pid']:
        if (root / name).exists():
            assert not Path('/proc', (root / name).read_text().strip()).exists(), 'Owned provider survived close'
    result = dict(ok=True, installedPair=True, applicationHooks=False, gatewayOverride=False,
                  desktopEntryHandoff=True, boundedSourceEdit=True, exactOutlineLines=True,
                  independentClipboard=True, explicitSaveOnly=True, tablePaging=True,
                  columnFilteringKeepsPage=True, draftRetained=True, modelTurns=0,
                  memory=measured, measurements=measurements, artifacts=str(root))
    (root / 'large-reader.json').write_text(json.dumps(result, indent=2))
    print(json.dumps(result))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--reference', type=Path)
    parser.add_argument('--private-root', type=Path)
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    bundle = args.bundle.resolve()
    if args.private_root:
        return run(bundle, args.private_root)
    if not args.reference:
        parser.error('--reference is required for an actual large Markdown report')
    reference = args.reference.resolve(strict=True)
    assert reference.is_file() and 4 * 1024 * 1024 < reference.stat().st_size < 128 * 1024 * 1024
    base = REPO / 'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='release-large-', dir=base))
    runtime.create_profile(root)
    source_hash = digest(reference)
    shutil.copyfile(reference, root / 'project/reference.md')
    rows = generate(root / 'project/large.md')
    with (root / 'project/large.md').open('rb') as source, (root / 'project/large-crlf.md').open('wb') as target:
        for line in source:
            target.write(line.replace(b'\n', b'\r\n'))
    metadata = dict(rows=rows)
    for filename in ['reference.md', 'large.md', 'large-crlf.md']:
        path = root / 'project' / filename
        hs = headings(path)
        metadata[filename] = dict(sha256=digest(path), lastHeading=[hs[-1][1], hs[-1][0]], headingLabels=[title for _,title in hs])
    metadata['large.md']['tableLine'] = 5
    metadata['large-crlf.md']['tableLine'] = 5
    with reference.open(encoding='utf-8') as stream:
        metadata['referenceTitle'] = stream.readline().rstrip('\r\n').removeprefix('# ')
    (root / 'fixtures.json').write_text(json.dumps(metadata, indent=2))
    settings = root / 'config/codex-thread-studio/settings.json'
    preferences = json.loads(settings.read_text()); preferences['language'] = 'en-US'
    settings.write_text(json.dumps(preferences))
    (root / 'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex', selectedSession='target')))
    (root / 'raw-items.json').write_text(json.dumps([
        dict(id='large-reader-history', type='agentMessage', text='Private reader history.')]))
    for name, code in zip(['fake-codex.py', 'fake-opencode.py'], outputs.providers()):
        path = root / name; path.write_text(code); path.chmod(0o700)
    prefix = root / 'installed large 中文 % test'
    subprocess.run(['bash', str(REPO / 'scripts/install-gnome.sh'), str(bundle), str(prefix)], check=True)
    env = dict(os.environ, CODEX_THREAD_STUDIO_CODEX_BIN=str(root / 'fake-codex.py'),
               CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root / 'fake-opencode.py'),
               CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root / 'config/codex-thread-studio/backends.json'),
               CODEX_HOME=str(root / 'codex'), XDG_CONFIG_HOME=str(root / 'config'),
               XDG_DATA_HOME=str(root / 'data'), XDG_CACHE_HOME=str(root / 'cache'),
               XDG_DATA_DIRS=str(prefix / 'share') + ':/usr/local/share:/usr/share',
               READER_CACHE_DIR=str(root / 'reader-cache'), GDK_BACKEND='wayland', GTK_A11Y='atspi',
               G_DEBUG='fatal-criticals', GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE') or key in ['CODEX_THREAD_STUDIO_GATEWAY_BIN', 'STUDIO_GNOME_TEXT_FLOW_EXPERIMENT']:
            env.pop(key)
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', shutil.which('mutter') or 'mutter', '--headless',
                        '--wayland', '--no-x11', '--wayland-display', 'studio-test', '--virtual-monitor',
                        '1600x1000', '--', sys.executable, str(Path(__file__).resolve()), str(bundle),
                        '--private-root', str(root)], cwd=root, env=private, check=True, timeout=240)
    assert digest(reference) == source_hash, 'The original user report changed'


if __name__ == '__main__':
    main()
