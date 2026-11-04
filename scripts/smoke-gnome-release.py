#!/usr/bin/env python3
"""Installed native release UI acceptance without application test hooks.

Requires python3-gi, the Atspi 2.0 typelib, and Mutter. All input, accessibility
queries, providers, files and desktop activation stay in an owned private session.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import signal
import re
import math
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / 'scripts/test-support'))
from private_wayland import isolated_environment, OwnedMutterInput
from private_file_portal import FilePortal
from memory_sampler import ProcessMemory
from private_codex_exit import instrument as instrument_exit
from private_codex_lag import instrument as instrument_lag

spec = importlib.util.spec_from_file_location('release_outputs', REPO / 'scripts/smoke-gnome-outputs.py')
outputs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(outputs)

ORIGINAL = (
    '# Release native reader\n\n'
    r'Native **中文😀** before $\frac{a}{b}$ after $x^2$.' + '\n\n'
    '```math\nx^2 + y^2 = z^2\n```\n\n'
    '| Name | Value |\n|---|---|\n| Alpha | $x+y$ |\n'
)
PLAIN = r'Native 中文😀 before $\frac{a}{b}$ after $x^2$.'
SAVED = 'Saved through native release 中文😀.'
DRAFT = 'Unsent release draft 中文😀 '
STRUCTURED_SOURCE = '// Installed JSON5 中文😀\r\n{"message":"Installed value 中文😀","items":[1,2],}\r\n'
STRUCTURED_PREVIEW = '// Installed JSON5 中文😀\n{\n  "message": "Installed value 中文😀",\n  "items": [\n    1,\n    2\n  ],\n}\n'
ACTIVITY_ITEM = dict(id='release-tool', type='mcpToolCall', server='literal <server>', tool='lookup', status='failed', arguments={'query':'Installed metadata 中文😀'}, error={'message':'Installed tool error'}, result=None)
HTML_SOURCE = '<!doctype html>\r\n<h1>Installed HTML original</h1><p>Installed HTML paragraph 中文😀</p><script>globalThis.untrusted=1</script>\r\n'


def run(root, all_diagrams=False, language=False, backends=False, git_review=False, viewers=False, recovery=False, event_gap=False, remote_images=False):
    import gi
    gi.require_version('Atspi', '2.0')
    gi.require_version('GioUnix', '2.0')
    from gi.repository import Atspi, Gio, GioUnix, GLib
    Atspi.init()
    Atspi.set_timeout(1500, 1500)
    prefix = root / 'installed native 中文 % test'
    native = prefix / 'bin/codex-thread-studio-gnome'
    gateway = prefix / 'bin/codex-thread-studio-gateway'
    document = root / 'project/release.md'
    controlled = OwnedMutterInput()  # Verifies the compositor service's PID.
    controlled.enable_clipboard_reader()
    saved_image = root / 'saved release original 中文.png'
    saved_svg = root / 'saved release original 中文.svg'
    chosen_source = root / 'project/open file 中文.rs'
    chosen_bytes = chosen_source.read_bytes()
    plans = [dict(
        interface='FileChooser', method='SaveFile', title='Save original image',
        options=dict(multiple=False, directory=False, current_name='sample.png'),
        uris=[saved_image.as_uri()]), dict(
        interface='FileChooser', method='OpenFile', title='Open document',
        options=dict(multiple=False, directory=False),
        uris=[chosen_source.as_uri()])]
    if viewers:
        plans.insert(0,dict(interface='FileChooser',method='SaveFile',title='Save original image',
            options=dict(multiple=False,directory=False,current_name='svg-bitmaps.svg'),
            uris=[saved_svg.as_uri()]))
    if remote_images:
        plans.insert(int(viewers),dict(interface='FileChooser',method='SaveFile',title='Save original image',
            options=dict(multiple=False,directory=False,current_name='image.png'),
            uris=[(root/'saved release remote.png').as_uri()]))
    if language:
        language_plans=[]
        for chinese in [True,False]:
            language_plans.append(dict(interface='FileChooser',method='OpenFile',title='选择项目目录' if chinese else 'Choose project folder',options=dict(multiple=False,directory=True),status=1))
        for chinese in [True,False]:
            for method,title,options in [
                ('OpenFile','打开图片' if chinese else 'Open image',dict(multiple=False,directory=False,accept_label='打开' if chinese else 'Open')),
                ('SaveFile','保存原始图片' if chinese else 'Save original image',dict(multiple=False,directory=False,current_name='Attached image {width} 中文😀.png')),
            ]:
                language_plans.append(dict(interface='FileChooser',method=method,title=title,options=options,status=1))
        plans=language_plans+plans
    portal = FilePortal(root, plans)
    log = (root / 'app.log').open('w')
    child = subprocess.Popen([str(native), str(document)], cwd=root,
                             env=dict(os.environ), stdout=log, stderr=log)
    memory = ProcessMemory(child.pid, gateway, include_web_content=True)
    step = 'startup'

    def until(name, test, seconds=25):
        nonlocal step
        step = name
        start = time.monotonic()
        while time.monotonic() - start < seconds:
            portal.iterate()
            if portal.failure:
                raise RuntimeError((name, 'Private portal rejected request', portal.failure))
            if child.poll() is not None:
                raise RuntimeError((name, 'Application exited', child.returncode, root))
            result = test()
            if result:
                return result
            while GLib.MainContext.default().pending():
                GLib.MainContext.default().iteration(False)
            time.sleep(.03)
        raise TimeoutError((name, root))

    def application():
        desktop = Atspi.get_desktop(0)
        for i in range(desktop.get_child_count()):
            app = desktop.get_child_at_index(i)
            if app and app.get_process_id() == child.pid:
                return app

    def tree():
        pending, rows = [application()], []
        while pending:
            if len(rows) >= 2000:
                raise RuntimeError('Private accessibility tree exceeded its bound')
            item = pending.pop()
            if item is None:
                continue
            try:
                rows.append(item)
                pending.extend(item.get_child_at_index(i)
                               for i in reversed(range(item.get_child_count())))
            except GLib.Error:
                continue  # A disposed node is not eligible for an action.
        return rows

    def text(item):
        interface = item.get_text_iface()
        if interface:
            # Explicit interface avoids Accessible.get_text's GI name collision.
            return Atspi.Text.get_text(interface, 0, Atspi.Text.get_character_count(interface))

    def reader_preview(passage):
        # Whole-document Preview has one named native text surface; the legacy
        # diagnostic renderer exposes separate labels. Read actual text content.
        return find(lambda a: showing(a) and (a.get_name() == passage or
            (a.get_role_name() == 'text' and a.get_name() in ['Preview', '预览'] and
             ((passage == PLAIN and (text(a) or '').startswith('Release native reader\n\nNative 中文😀 before ')) or
              (passage != PLAIN and passage in (text(a) or ''))))))

    def find(test):
        for item in tree():
            try:
                item.clear_cache()
                if test(item):
                    return item
            except GLib.Error:
                # A native dialog can dispose a node after tree traversal but
                # before its properties are queried. Retry on the next poll.
                continue
        return None

    def showing(item):
        return item.get_state_set().contains(Atspi.StateType.SHOWING)

    def action(item, name):
        assert item.get_application().get_process_id() == child.pid
        interface = item.get_action_iface()
        for i in range(Atspi.Action.get_n_actions(interface)):
            # The installed Python typelib exposes get_action_name; its C API
            # replacement is not exported as Action.get_name in these bindings.
            if Atspi.Action.get_action_name(interface, i) == name:
                assert Atspi.Action.do_action(interface, i), (step, name)
                return
        raise RuntimeError((step, 'Native accessibility action missing', name))

    def click(name):
        item = until('enabled button: ' + name, lambda: find(
            lambda a: a.get_name() == name and 'button' in a.get_role_name()
            and a.get_state_set().contains(Atspi.StateType.SENSITIVE)
            and a.get_state_set().contains(Atspi.StateType.SHOWING)))
        action(item, 'click')

    def source(editable):
        return find(lambda a: a.get_role_name() == 'text'
                    and a.get_state_set().contains(Atspi.StateType.EDITABLE) == editable
                    and (text(a) or '').startswith('# Release native reader'))

    def keys(values):
        for key in values:
            controlled.call('NotifyKeyboardKeysym', GLib.Variant('(ub)', (key, True)))
        time.sleep(.06)
        for key in reversed(values):
            controlled.call('NotifyKeyboardKeysym', GLib.Variant('(ub)', (key, False)))

    def inspect_diagram(kind, steps=1, chinese=False):
        click(f'查看 {kind} 图表' if chinese else f'Inspect {kind} diagram')
        prefix='渲染图片 · ' if chinese else 'Rendered image · '
        suffix='像素' if chinese else 'pixels'
        until('native diagram inspector', lambda: find(
            lambda a: showing(a) and a.get_name().startswith(prefix)))
        caption = find(lambda a: showing(a) and a.get_name().startswith(prefix)).get_name()
        dimensions = re.fullmatch(re.escape(prefix)+r'(\d+) × (\d+) '+suffix, caption)
        assert dimensions, caption
        width, height = map(int, dimensions.groups())
        started = time.monotonic()
        for index in range(steps):
            click('放大' if chinese else 'Zoom in')
            next_percent = f'{min(4.0, 1.25 ** (index + 1)) * 100:.0f}%'
            until(f'diagram zoom: {kind} {next_percent}', lambda: find(
                lambda a: a.get_name() == next_percent and 'button' in a.get_role_name()))
        zoom = min(4.0, 1.25 ** steps)
        percent = f'{zoom * 100:.0f}%'
        until('diagram zoom', lambda: find(lambda a: a.get_name() == percent and 'button' in a.get_role_name()))
        scale = min(zoom, 8192 / width, 8192 / height, math.sqrt(8 * 1024 * 1024 / (width * height)))
        expected = [int(width * scale), int(height * scale)]
        until('vector zoom raster: ' + kind, lambda: find(lambda a: a.get_name() ==
              f'{prefix}{expected[0]} × {expected[1]} {suffix}'))
        elapsed = round((time.monotonic() - started) * 1000)
        click(percent)
        until('diagram actual size', lambda: find(lambda a: a.get_name() == '100%' and 'button' in a.get_role_name()))
        click('适应' if chinese else 'Fit')
        keys([0xff1b])
        until('closed diagram inspector', lambda: find(lambda a: showing(a) and a.get_name().startswith(prefix)) is None)
        return dict(zoomPercent=round(zoom * 100), pixels=expected, actionsToReadyMs=elapsed)

    def within(item, parent):
        for _ in range(30):
            if item == parent:
                return True
            item = item.get_parent()
            if item is None:
                return False
        return False

    def activate(item):
        interface = item.get_action_iface()
        names = [Atspi.Action.get_action_name(interface, i)
                 for i in range(Atspi.Action.get_n_actions(interface))] if interface else []
        for name in ['click', 'activate']:
            if name in names:
                action(item, name)
                return
        # Some native preference rows have no AT-SPI action. Use their ordinary
        # keyboard focus chain on the owned compositor instead.
        for _ in range(80):
            focused = find(lambda a: a.get_state_set().contains(Atspi.StateType.FOCUSED))
            if focused and (within(focused, item) or within(item, focused)):
                keys([0xff0d])
                return
            keys([0xff09])
            for node in tree():
                node.clear_cache()
        raise RuntimeError((step, 'Native control not keyboard reachable', item.get_name()))

    def activity_controls():
        click('Inspect activity snapshot')
        dialog=until('installed activity inspector',lambda:find(
            lambda a:showing(a) and a.get_name()=='Activity details' and a.get_role_name()=='dialog'))
        view=until('installed raw metadata',lambda:find(lambda a:showing(a) and within(a,dialog)
            and a.get_role_name()=='text' and '"release-tool"' in (text(a) or '')))
        raw=text(view)
        assert json.loads(raw)==ACTIVITY_ITEM, 'Installed inspector lost original tool metadata'
        assert not view.get_state_set().contains(Atspi.StateType.EDITABLE), 'Installed activity inspector is editable'
        def copied_activity():
            try:return controlled.clipboard_text()==raw
            except GLib.Error:return False  # Accessible action and Wayland offer are asynchronous.
        click('Copy displayed page')
        until('installed exact activity Copy',copied_activity)
        keys([0xff1b])
        until('closed installed activity inspector',lambda:not find(
            lambda a:showing(a) and a.get_name()=='Activity details' and a.get_role_name()=='dialog'))
        assert text(composer)==DRAFT and document.read_text()==ORIGINAL
        until('reader preserved after activity inspection',lambda:reader_preview(PLAIN))

    def backend_controls():
        path = root / 'config/codex-thread-studio/backends.json'
        assert not path.exists()
        keys([0xffe3, ord(',')])
        tab = until('Backends settings tab', lambda: find(
            lambda a: a.get_name() == 'Backends' and a.get_role_name() in
            ['page tab', 'toggle button', 'push button', 'button']))
        activate(tab)
        diagnostics = until('connection diagnostics settings row', lambda: find(
            lambda a: a.get_name().startswith('Connection diagnostics')
            and a.get_role_name() == 'list item'))
        activate(diagnostics)
        connections = until('installed connections dialog', lambda: find(
            lambda a: showing(a) and a.get_name() == 'Connections & accounts'
            and a.get_role_name() == 'dialog'))
        until('installed connection metadata loaded', lambda: find(
            lambda a: showing(a) and within(a, connections)
            and a.get_name() == 'Refresh connection information'
            and a.get_state_set().contains(Atspi.StateType.SENSITIVE)))
        assert find(lambda a: within(a, connections) and a.get_name() == 'Connected'), 'Connected event stream absent from installed diagnostics'
        keys([0xff1b])
        until('return from connection diagnostics', lambda: not find(
            lambda a: showing(a) and a.get_name() == 'Connections & accounts'
            and a.get_role_name() == 'dialog'))
        row = until('backend editor action row', lambda: find(
            lambda a: a.get_name().startswith('Edit backend configuration')
            and a.get_role_name() == 'list item'))
        activate(row)
        dialog = until('backend configuration dialog', lambda: find(
            lambda a: a.get_name() == 'Backend configuration' and a.get_role_name() == 'dialog'))
        editor = until('backend JSON text', lambda: find(
            lambda a: a.get_role_name() == 'text' and within(a, dialog)
            and a.get_state_set().contains(Atspi.StateType.EDITABLE)))
        source = json.dumps(dict(version=1, backends=[dict(
            id='release-extra', name='<Native> 中文', tag='NX', adapter='codex-app-server',
            command=str(root / 'never-start-release-extra'))]), ensure_ascii=False, indent=2).replace('\n', '\r\n') + '\r\n'
        assert Atspi.EditableText.set_text_contents(editor.get_editable_text_iface(), source)

        def panel_click(name):
            button = until('backend editor button: ' + name, lambda: find(
                lambda a: a.get_name() == name and 'button' in a.get_role_name()
                and within(a, dialog) and a.get_state_set().contains(Atspi.StateType.SENSITIVE)))
            action(button, 'click')

        panel_click('Validate')
        until('backend validation result', lambda: find(
            lambda a: within(a, dialog) and a.get_name() == 'Valid configuration · 1 enabled backends'))
        assert not path.exists(), 'Validate created backend configuration'
        panel_click('Save')
        until('explicit backend file save', lambda: path.exists() and path.read_bytes() == source.encode())
        until('saved backend editor', lambda: find(
            lambda a: within(a, dialog) and a.get_name() == 'Saved. Restart Studio to apply backend changes.'))
        assert text(composer) == DRAFT and document.read_text() == ORIGINAL
        panel_click('Close')
        until('closed backend editor', lambda: find(
            lambda a: a.get_role_name() == 'dialog' and a.get_name() == 'Backend configuration') is None)
        settings = until('backend parent settings', lambda: find(
            lambda a: a.get_role_name() == 'dialog' and a.get_name() == 'Settings'))
        close = until('backend parent close', lambda: find(
            lambda a: a.get_name() == 'Close' and 'button' in a.get_role_name() and within(a, settings)))
        action(close, 'click')
        until('closed backend settings', lambda: find(
            lambda a: a.get_role_name() == 'dialog' and a.get_name() == 'Settings') is None)

    def git_controls():
        click('Git Review')
        search = until('native Git filter', lambda: find(
            lambda a: a.get_role_name() in ['entry', 'text'] and a.get_name() == 'Filter files'
            and a.get_state_set().contains(Atspi.StateType.SHOWING)))

        def file_row():
            def parent(item):
                for _ in range(6):
                    item = item.get_parent()
                    if item is None:
                        return None
                    if item.get_role_name() == 'list item':
                        return item
            label = find(lambda a: a.get_role_name() == 'label' and a.get_name() == 'release.md'
                         and a.get_state_set().contains(Atspi.StateType.SHOWING) and parent(a))
            return parent(label) if label else None

        def diff():
            return find(lambda a: a.get_role_name() == 'text'
                        and a.get_state_set().contains(Atspi.StateType.SHOWING)
                        and (text(a) or '').startswith('diff --git'))

        def select_file(keyboard=False):
            row = until('native Git file row', file_row)
            parent = row.get_parent()
            selection = parent.get_selection_iface()
            if not keyboard and selection and Atspi.Selection.select_child(selection, row.get_index_in_parent()):
                return
            # GtkListView's selection bridge is not available in every GTK
            # version. Follow actual focus, ignoring focused ancestor frames.
            for _ in range(80):
                focused = []
                for node in tree():
                    node.clear_cache()
                    if node.get_state_set().contains(Atspi.StateType.FOCUSED):
                        focused.append(node)
                if any(within(node, row) for node in focused):
                    keys([0xff0d])
                    return
                if any(node.get_role_name() == 'list' and within(row, node) for node in focused):
                    keys([0xff50])
                    for _ in range(row.get_index_in_parent()):
                        keys([0xff54])
                    keys([0xff0d])
                    return
                keys([0xff09])
            raise RuntimeError('Git file row is not reachable through native keyboard focus')

        select_file(keyboard=True)
        until('native Git diff', lambda: diff() and '+' + SAVED in text(diff()))
        assert Atspi.EditableText.set_text_contents(search.get_editable_text_iface(), 'no release file matches')
        until('hidden Git selection', lambda: file_row() is None and diff() is None)
        stage_button = find(lambda a: a.get_name() == 'Stage file' and 'button' in a.get_role_name())
        assert stage_button
        stage_button.clear_cache()
        assert not stage_button.get_state_set().contains(Atspi.StateType.SENSITIVE)
        assert Atspi.EditableText.set_text_contents(search.get_editable_text_iface(), 'RELEASE.MD')
        until('case-insensitive Git file filter', file_row)
        select_file()
        until('filtered native Git diff', lambda: diff() and '+' + SAVED in text(diff()))
        click('Open file')
        until('Git-opened Markdown Preview', lambda: reader_preview(SAVED))
        click('Close document')
        until('return to native Git Review', lambda: diff() and '+' + SAVED in text(diff()))
        click('Stage file')

        def staged():
            result = subprocess.run(['git', '-C', str(root / 'project'), 'diff', '--cached', '--name-only', '--'],
                                    check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            return result.stdout == b'release.md\n'

        until('explicit native Git staging', staged)
        until('staged file left unstaged view', lambda: file_row() is None)
        assert document.read_text() == ORIGINAL + '\n' + SAVED + '\n'
        assert text(composer) == DRAFT

    def choose_language(title,index):
        combo=until('language combo: '+title,lambda:find(lambda a:'combo' in a.get_role_name() and a.get_name()==title and a.get_state_set().contains(Atspi.StateType.SHOWING)))
        interface=combo.get_action_iface()
        names=[Atspi.Action.get_action_name(interface,i) for i in range(Atspi.Action.get_n_actions(interface))]
        if 'activate' in names:action(combo,'activate')
        elif 'click' in names:action(combo,'click')
        else:
            # GTK 4.22 exposes neither an action nor usable global bounds
            # for AdwComboRow on Wayland. Follow ordinary Tab focus instead.
            def focused(item):
                item.clear_cache()
                if item.get_state_set().contains(Atspi.StateType.FOCUSED):return True
                return any(focused(item.get_child_at_index(i)) for i in range(item.get_child_count()))
            for _ in range(40):
                while GLib.MainContext.default().pending():GLib.MainContext.default().iteration(False)
                parent=combo.get_parent()
                if focused(combo) or (parent and parent.get_role_name()=='list item' and focused(parent)):break
                keys([0xff09])
                time.sleep(.04)
            else:
                focused_nodes=[(a.get_name(),a.get_role_name()) for a in tree() if a.get_state_set().contains(Atspi.StateType.FOCUSED)]
                raise RuntimeError(('Language combo not reachable through keyboard focus',focused_nodes))
            keys([ord(' ')])
        # The selected choice also exists in the collapsed row. Require a
        # different choice to become visible before sending popup keys.
        option='English' if title=='语言' else '简体中文'
        until('language popup',lambda:find(lambda a:a.get_name()==option and a.get_state_set().contains(Atspi.StateType.SHOWING)))
        keys([0xff50])
        for _ in range(index):keys([0xff54])
        keys([0xff0d])


    def language_controls():
        def command_language(chinese):
            def open_command(command):
                keys([0xffe3,0xffe1,ord('p')])
                title='命令' if chinese else 'Commands'
                palette=until('translated command palette',lambda:find(lambda a:showing(a) and a.get_role_name()=='dialog' and a.get_name()==title))
                entry=until('named command search',lambda:find(lambda a:showing(a) and within(a,palette) and a.get_role_name() in ['entry','text'] and a.get_name()==('搜索命令…' if chinese else 'Search commands…')))
                assert Atspi.EditableText.set_text_contents(entry.get_editable_text_iface(),'/'+command)
                # The previous list also contains /status. Wait for the
                # asynchronous filter to replace it before pressing Enter.
                until('filtered literal slash command',lambda:{a.get_name() for a in tree() if showing(a) and within(a,palette) and a.get_role_name()=='label' and a.get_name().startswith('/')}=={'/'+command})
                until('translated palette instruction',lambda:find(lambda a:showing(a) and within(a,palette) and a.get_name()==('按 Enter 执行 · 按 Escape 关闭' if chinese else 'Enter to run · Escape to close')))
                keys([0xff0d])
                until('command palette closed',lambda:not find(lambda a:showing(a) and a.get_role_name()=='dialog' and a.get_name()==title))
            def close_dialog(dialog):
                close=until('native dialog close',lambda:find(lambda a:showing(a) and within(a,dialog) and 'button' in a.get_role_name() and a.get_name() in ['Close','关闭']))
                action(close,'click')
                until('native dialog closed',lambda:not showing(dialog))
            open_command('status')
            title='会话状态' if chinese else 'Session status'
            status=until('translated session status',lambda:find(lambda a:showing(a) and a.get_role_name()=='dialog' and a.get_name()==title))
            backend='后端' if chinese else 'Backend'
            session='会话' if chinese else 'Session'
            project='项目' if chinese else 'Project'
            usage='令牌用量' if chinese else 'Token usage'
            body=until('translated fields and literal status values',lambda:find(lambda a:showing(a) and within(a,status) and f'{backend}: codex\n' in a.get_name() and f'{session}: target\n' in a.get_name() and f'{project}: {root / "project"}\n' in a.get_name() and usage in a.get_name()))
            assert ('消息权限: 继承会话' if chinese else 'Message permissions: Inherit session') in body.get_name()
            close_dialog(status)
            open_command('permissions')
            title='消息权限' if chinese else 'Message permissions'
            permissions=until('translated message permissions',lambda:find(lambda a:showing(a) and a.get_role_name()=='dialog' and a.get_name()==title))
            inherited='当前: 继承会话' if chinese else 'Current: Inherit session'
            until('unchanged inherited access',lambda:find(lambda a:showing(a) and within(a,permissions) and a.get_name()==inherited))
            until('translated selected permission',lambda:find(lambda a:showing(a) and within(a,permissions) and a.get_name()==('已选择' if chinese else 'Selected')))
            full='完全访问' if chinese else 'Full access'
            label=until('full access row',lambda:find(lambda a:showing(a) and within(a,permissions) and a.get_name()==full))
            row=label
            for _ in range(8):
                button=find(lambda a:showing(a) and within(a,row) and 'button' in a.get_role_name() and a.get_name()==('使用' if chinese else 'Use'))
                if button:break
                row=row.get_parent()
                assert row and row!=permissions,'Full access row has no Use action'
            assert button
            action(button,'click')
            heading='对后续消息使用完全访问权限？' if chinese else 'Use full access for future messages?'
            confirm=until('translated destructive confirmation',lambda:find(lambda a:showing(a) and a.get_role_name() in ['dialog','alert'] and heading in a.get_name()))
            until('translated destructive action',lambda:find(lambda a:showing(a) and within(a,confirm) and 'button' in a.get_role_name() and a.get_name()==('使用完全访问权限' if chinese else 'Use full access')))
            cancel=until('translated confirmation cancel',lambda:find(lambda a:showing(a) and within(a,confirm) and 'button' in a.get_role_name() and a.get_name()==('取消' if chinese else 'Cancel')))
            action(cancel,'click')
            until('confirmation cancelled without changing permissions',lambda:find(lambda a:showing(a) and within(a,permissions) and a.get_name()==inherited))
            close_dialog(permissions)
            assert text(composer)==DRAFT and document.read_text()==ORIGINAL
            inspect_diagram('math',chinese=chinese)

        image_name='Attached image {width} 中文😀.png'
        image_path=root/'project'/image_name
        image_bytes=(root/'project/sample.png').read_bytes()
        image_path.write_bytes(image_bytes)
        launcher=GioUnix.DesktopAppInfo.new_from_filename(str(
            prefix/'share/applications/io.github.messon007.codexthreadstudio.Gnome.desktop'))
        assert launcher
        def open_language_image(chinese=False):
            assert launcher.launch([Gio.File.new_for_path(str(image_path))],None)
            ready='3000 × 1000 像素 · 预览 2048 × 683' if chinese else '3000 × 1000 pixels · Preview 2048 × 683'
            until('image for live language change',lambda:find(lambda a:showing(a) and a.get_name()==ready))
            click('放大' if chinese else 'Zoom in')
            until('image zoom retained for language change',lambda:find(lambda a:showing(a) and a.get_name()=='125%' and 'button' in a.get_role_name()))
        def image_language(chinese):
            names=(['打开图片','重新加载图片','关闭图片','缩小','放大','复制图片预览','保存原始图片…','适应'] if chinese else
                   ['Open image','Reload image','Close image','Zoom out','Zoom in','Copy image preview','Save original image…','Fit'])
            for name in names:
                until('image language control '+name,lambda name=name:find(lambda a:showing(a) and a.get_name()==name and 'button' in a.get_role_name()))
            ready='3000 × 1000 像素 · 预览 2048 × 683' if chinese else '3000 × 1000 pixels · Preview 2048 × 683'
            until('live image dimension status',lambda:find(lambda a:showing(a) and a.get_name()==ready))
            assert find(lambda a:showing(a) and a.get_name()==image_name),'Image filename was translated or formatted'
            assert find(lambda a:showing(a) and a.get_name()==str(image_path)),'Image path was changed'
            assert find(lambda a:showing(a) and a.get_name()=='125%' and 'button' in a.get_role_name()),'Language change reset image zoom'
            for name in (['打开图片','保存原始图片…'] if chinese else ['Open image','Save original image…']):
                count=len(portal.calls);click(name)
                until('translated image portal '+name,lambda:len(portal.calls)==count+1 and not portal.objects)
                if name in ['打开图片','Open image']:
                    assert portal.calls[-1]['options']['filters'][0][0]==('图片' if chinese else 'Images')
                assert find(lambda a:showing(a) and a.get_name()==image_name),'Cancelled image chooser changed the source'
                until('image action restored after portal cancel',lambda:find(lambda a:showing(a) and a.get_name()==name and a.get_state_set().contains(Atspi.StateType.SENSITIVE)))
            click('缩小' if chinese else 'Zoom out')
            until('translated image zoom action',lambda:find(lambda a:showing(a) and a.get_name()=='100%' and 'button' in a.get_role_name()))
            click('关闭图片' if chinese else 'Close image')
            until('reader restored after image language check',lambda:reader_preview(PLAIN))
            assert image_path.read_bytes()==image_bytes and text(composer)==DRAFT
        def new_session_language(chinese):
            keys([0xffe3,ord('n')])
            heading='新建会话' if chinese else 'New session'
            dialog=until('translated New session',lambda:find(lambda a:showing(a) and a.get_role_name() in ['dialog','frame'] and a.get_name()==heading))
            body='选择此会话的项目目录。' if chinese else 'Choose the project directory for this session.'
            until('translated project instruction',lambda:find(lambda a:showing(a) and within(a,dialog) and a.get_name()==body))
            entry=until('named project directory',lambda:find(lambda a:showing(a) and within(a,dialog) and a.get_role_name() in ['entry','text'] and a.get_name()==('项目目录' if chinese else 'Project directory')))
            literal=str(root/'project/Choose project folder {index} 中文😀')
            assert Atspi.EditableText.set_text_contents(entry.get_editable_text_iface(),literal)
            for name in (['选择项目目录','创建','取消'] if chinese else ['Choose project folder','Create','Cancel']):
                until('New session control '+name,lambda name=name:find(lambda a:showing(a) and within(a,dialog) and a.get_name()==name and 'button' in a.get_role_name()))
            assert text(entry)==literal,'Project path was translated or formatted'
            count=len(portal.calls);click('选择项目目录' if chinese else 'Choose project folder')
            until('translated project-folder portal cancelled',lambda:len(portal.calls)==count+1 and not portal.objects)
            assert text(entry)==literal,'Cancelled folder chooser changed the project path'
            action(find(lambda a:showing(a) and within(a,dialog) and a.get_name()==('取消' if chinese else 'Cancel') and 'button' in a.get_role_name()),'click')
            until('New session cancelled',lambda:not find(lambda a:showing(a) and a.get_role_name() in ['dialog','frame'] and a.get_name()==heading))
            assert text(composer)==DRAFT and document.read_text()==ORIGINAL
        def named_menu(locale):
            title='主菜单' if locale=='zh' else 'Main menu'
            label='会话 Map' if locale=='zh' else 'Session Map'
            button=until('shell menu: '+title,lambda:find(lambda a:a.get_name()==title and a.get_role_name()=='toggle button' and a.get_state_set().contains(Atspi.StateType.SHOWING)))
            action(button,'click')
            until('translated accessible menu: '+label,lambda:find(lambda a:a.get_role_name()=='menu item' and a.get_name()==label and a.get_state_set().contains(Atspi.StateType.SHOWING)))
            menu=until('native menu container',lambda:find(lambda a:a.get_role_name()=='menu' and a.get_state_set().contains(Atspi.StateType.SHOWING)))
            rows=[a for a in tree() if a.get_role_name()=='menu item' and within(a,menu)]
            assert len(rows)>=20 and all(a.get_name() for a in rows),'Shell menu actions lost accessible labels'
            keys([0xff1b])
            until('closed translated shell menu',lambda:find(lambda a:a.get_role_name()=='menu item' and a.get_name()==label and a.get_state_set().contains(Atspi.StateType.SHOWING)) is None)
        path=root/'config/codex-thread-studio/settings.json'
        before=json.loads(path.read_text())
        # The independent recovery probe appends its authoritative history item
        # to the fake provider's SOURCES. Derive the count from that fixture
        # ledger, not the UI text we are checking.
        ledger=json.loads((root/'codex-turns.json').read_text()) if (root/'codex-turns.json').exists() else {}
        history_count=len(json.loads((root/'raw-items.json').read_text()))+sum(
            len(turn['items']) for turn in ledger.get('target',[]) if turn['id'].startswith('gap-fixture-turn-'))
        keys([0xffe3,0xffe1,ord('h')])
        until('installed history pager',lambda:find(lambda a:showing(a) and a.get_name()=='Live' and 'button' in a.get_role_name()))
        keys([0xffe3,ord(',')])
        until('native language controls',lambda:find(lambda a:a.get_name()=='Apply language' and 'button' in a.get_role_name()))
        # Let the adaptive dialog finish its opening animation before following
        # its ordinary keyboard focus chain.
        time.sleep(.4)

        choose_language('Language',1)
        click('Apply language')
        until('shared Chinese preference',lambda:json.loads(path.read_text()).get('language')=='zh-CN')
        until('live Chinese Send',lambda:find(lambda a:a.get_name()=='发送' and 'button' in a.get_role_name()))
        composer.clear_cache()
        assert composer.get_name()=='输入消息。Ctrl+Enter 发送；Enter 换行。', 'Composer accessible instruction stayed English'
        assert text(composer)==DRAFT and document.read_text()==ORIGINAL
        question_controls('zh', False)
        settings=until('Chinese settings dialog',lambda:find(
            lambda a:a.get_role_name()=='dialog' and a.get_name()=='设置'))
        close=until('Chinese settings close',lambda:find(
            lambda a:a.get_name() in ['Close','关闭'] and 'button' in a.get_role_name() and within(a,settings)))
        action(close,'click')
        until('closed Chinese settings',lambda:find(
            lambda a:a.get_role_name()=='dialog' and a.get_name()=='设置') is None)
        for label in ['更早','较新','实时会话','复制消息 Markdown','收藏显示的回复']:
            until('Chinese history/transcript action: '+label,lambda label=label:find(lambda a:showing(a) and a.get_name()==label and 'button' in a.get_role_name()))
        until('Chinese history summary',lambda:find(lambda a:showing(a) and a.get_name()==f'历史 · 第 1 页 · {history_count} 条消息'))
        new_session_language(True)
        command_language(True)
        named_menu('zh')
        comparison_language('zh')
        if git_review:
            click('Git 审查')
            until('Chinese Git accessible search name',lambda:find(
                lambda a:a.get_role_name() in ['entry','text'] and a.get_name()=='筛选文件'
                and a.get_state_set().contains(Atspi.StateType.SHOWING)))
            click('关闭 Git 审查')
            until('closed Chinese Git workspace',lambda:find(
                lambda a:a.get_name()=='筛选文件' and a.get_state_set().contains(Atspi.StateType.SHOWING)) is None)
            launcher=GioUnix.DesktopAppInfo.new_from_filename(str(
                prefix/'share/applications/io.github.messon007.codexthreadstudio.Gnome.desktop'))
            assert launcher and launcher.launch([Gio.File.new_for_path(str(document))],None)
            until('retained document after Chinese Git probe',lambda:reader_preview(PLAIN))
        keys([0xffe3,ord(',')])
        until('reopened Chinese language settings',lambda:find(
            lambda a:a.get_name()=='应用语言' and 'button' in a.get_role_name()))
        choose_language('语言',2)
        click('应用语言')
        until('shared English preference',lambda:json.loads(path.read_text()).get('language')=='en-US')
        until('live English Send',lambda:find(lambda a:a.get_name()=='Send' and 'button' in a.get_role_name()))
        composer.clear_cache()
        assert composer.get_name()=='Write a message. Ctrl+Enter sends; Enter starts a new line.'
        assert text(composer)==DRAFT and document.read_text()==ORIGINAL
        question_controls('en', True)
        after=json.loads(path.read_text())
        assert {k:v for k,v in before.items() if k!='language'}=={k:v for k,v in after.items() if k!='language'},'Language control replaced unrelated preferences'
        dialog=until('English settings dialog',lambda:find(lambda a:a.get_role_name()=='dialog' and a.get_name()=='Settings'))
        def in_dialog(item):
            for _ in range(30):
                if item==dialog:return True
                item=item.get_parent()
                if item is None:return False
            return False
        close=until('settings close button',lambda:find(lambda a:a.get_name()=='Close' and 'button' in a.get_role_name() and in_dialog(a)))
        action(close,'click')
        until('closed language settings',lambda:find(lambda a:a.get_name()=='Apply language' and 'button' in a.get_role_name()) is None)
        new_session_language(False)
        command_language(False)
        until('document visible after language controls',lambda:reader_preview(PLAIN))
        for label in ['Older','Newer','Live','Copy message Markdown','Save displayed response to favorites']:
            until('English history/transcript action: '+label,lambda label=label:find(lambda a:showing(a) and a.get_name()==label and 'button' in a.get_role_name()))
        until('English history summary',lambda:find(lambda a:showing(a) and a.get_name()==f'History · page 1 · {history_count} items'))
        click('Live')
        until('closed installed history pager',lambda:not find(lambda a:showing(a) and a.get_name()=='Live' and 'button' in a.get_role_name()))
        assert text(composer)==DRAFT and document.read_text()==ORIGINAL
        named_menu('en')
        comparison_language('en')
        # The image-only workspace intentionally removes hidden conversation
        # widgets from AT-SPI. Exercise its live bindings independently, keeping
        # the preceding modal input/history/composer coverage unchanged.
        for chinese in [True,False]:
            open_language_image(not chinese)
            keys([0xffe3,ord(',')])
            until('image language settings',lambda:find(lambda a:showing(a) and a.get_name()==('Apply language' if chinese else '应用语言') and 'button' in a.get_role_name()))
            choose_language('Language' if chinese else '语言',1 if chinese else 2)
            click('Apply language' if chinese else '应用语言')
            until('saved image locale',lambda:json.loads(path.read_text()).get('language')==('zh-CN' if chinese else 'en-US'))
            title='设置' if chinese else 'Settings'
            settings=until('image locale Settings dialog',lambda:find(lambda a:showing(a) and a.get_role_name()=='dialog' and a.get_name()==title))
            close=until('image locale Settings close',lambda:find(lambda a:showing(a) and within(a,settings) and 'button' in a.get_role_name() and a.get_name() in ['Close','关闭']))
            action(close,'click')
            until('image locale Settings closed',lambda:not find(lambda a:showing(a) and a.get_role_name()=='dialog' and a.get_name()==title))
            image_language(chinese)

    def comparison_language(locale):
        chinese = locale == 'zh'
        click('比较 Markdown 文件' if chinese else 'Compare Markdown files')
        for name in (['上一处更改', '下一处更改', '阅读器', '打开两个 Markdown 文件进行比较']
                     if chinese else ['Previous change', 'Next change', 'Reader', 'Open two Markdown files to compare']):
            until('comparison interface: '+name, lambda name=name: find(
                lambda a:a.get_name()==name and a.get_state_set().contains(Atspi.StateType.SHOWING)))
        until('retained comparison left content', lambda: reader_preview(PLAIN))
        # The indexed source includes the final empty line after a trailing LF.
        lines=ORIGINAL.count('\n')+1
        footer=(f'{lines} 行 · 0.0 MB · 第 1 行' if chinese
                else f'{lines} lines · 0.0 MB · Line 1')
        until('translated comparison document status',lambda:find(
            lambda a:a.get_name()==footer and a.get_state_set().contains(Atspi.StateType.SHOWING)))
        assert text(composer)==DRAFT and document.read_text()==ORIGINAL
        click('阅读器' if chinese else 'Reader')
        until('returned from translated comparison',lambda:find(
            lambda a:a.get_name()==('阅读器' if chinese else 'Reader')
            and 'button' in a.get_role_name() and a.get_state_set().contains(Atspi.StateType.SHOWING)) is None)
        until('retained reader after comparison',lambda:reader_preview(PLAIN))
        assert text(composer)==DRAFT and document.read_text()==ORIGINAL

    def pdf_language_controls():
        click('Next page')
        click('Text')
        def pdf_text(label):
            return find(lambda a: showing(a) and a.get_name()==label and a.get_role_name()=='text')
        until('PDF second page text',lambda:pdf_text('PDF text') and 'page 2' in (text(pdf_text('PDF text')) or ''))
        before=text(pdf_text('PDF text'))
        click('Search PDF')
        query=until('PDF search field',lambda:find(lambda a:showing(a) and a.get_name()=='Search PDF' and a.get_editable_text_iface()))
        assert Atspi.EditableText.set_text_contents(query.get_editable_text_iface(),'Needle')
        until('PDF search completed',lambda:find(lambda a:showing(a) and a.get_name()=='Search complete'))
        def copied_pdf():
            try:
                actual=controlled.clipboard_text()
                (root/'pdf-clipboard.json').write_text(json.dumps(dict(expected=before,actual=actual),ensure_ascii=False))
                return actual==before
            except GLib.Error:
                return False  # GTK accessibility dispatch and Wayland ownership are asynchronous.
        click('Copy selected or page text')
        until('PDF page copied through Wayland',copied_pdf)
        for locale in ['zh-CN','en-US']:
            chinese=locale=='zh-CN'
            current_chinese=not chinese
            keys([0xffe3,ord(',')])
            title='外观' if current_chinese else 'Appearance'
            tab=until('PDF language appearance page',lambda:find(lambda a:showing(a) and a.get_name()==title and a.get_role_name()=='page tab'))
            activate(tab)
            choose_language('语言' if current_chinese else 'Language',1 if chinese else 2)
            click('应用语言' if current_chinese else 'Apply language')
            until('saved PDF locale: '+locale,lambda:json.loads((root/'config/codex-thread-studio/settings.json').read_text()).get('language')==locale)
            label='设置' if chinese else 'Settings'
            dialog=until('PDF locale settings',lambda:find(lambda a:showing(a) and a.get_name()==label and a.get_role_name()=='dialog'))
            close=until('PDF locale settings close',lambda:find(lambda a:showing(a) and a.get_name() in ['Close','关闭'] and 'button' in a.get_role_name() and within(a,dialog)))
            action(close,'click')
            until('PDF settings closed',lambda:find(lambda a:showing(a) and a.get_name()==label and a.get_role_name()=='dialog') is None)
            content=until('retained PDF text',lambda:pdf_text('PDF 文本' if chinese else 'PDF text'))
            assert text(content)==before
            query=until('retained PDF query',lambda:find(lambda a:showing(a) and a.get_name()==('搜索 PDF' if chinese else 'Search PDF') and a.get_editable_text_iface()))
            assert text(query)=='Needle'
            number=until('retained PDF page number',lambda:find(lambda a:showing(a) and a.get_name()==('PDF 页码' if chinese else 'PDF page number')))
            assert text(number)=='2'
            until('localized PDF result label',lambda:find(lambda a:showing(a) and a.get_name()==('第 3 页' if chinese else 'Page 3')))
            until('localized PDF status',lambda:find(lambda a:showing(a) and a.get_name()==('搜索完成' if chinese else 'Search complete')))
            assert find(lambda a:showing(a) and a.get_name()=='viewer.pdf')
            assert text(composer)==DRAFT
            click('复制选中文本或页面文本' if chinese else 'Copy selected or page text')
            until('retained PDF page copied through Wayland',copied_pdf)

    def question_controls(locale, use_other):
        identity='release-choice-'+locale
        payload=dict(id=identity,method='item/tool/requestUserInput',params=dict(
            threadId='target',turnId='release-input',itemId=identity,
            questions=[dict(id='answer',header='Release input',question='Release custom response',isOther=True,
                options=[dict(label='Use preset',description='Provider words stay literal: Send Settings')])]))
        temporary=root/'input-events.tmp'
        temporary.write_text(json.dumps([payload]));temporary.replace(root/'input-events.json')
        heading='代理需要你的输入' if locale=='zh' else 'Agent needs your input'
        until('installed question dialog',lambda:find(lambda a:a.get_role_name()=='dialog' and a.get_name()==heading))
        field=until('installed custom response',lambda:find(lambda a:a.get_role_name()=='text' and a.get_name()=='Release custom response'))
        custom='  Retained custom 中文😀 {value}  '
        assert Atspi.EditableText.set_text_contents(field.get_editable_text_iface(),custom)
        def choice(name):
            return until('native choice: '+name,lambda:find(lambda a:a.get_name()==name and a.get_role_name() in ('radio button','check box')))
        normal=choice('Use preset')
        other=choice('其他回答' if locale=='zh' else 'Other response')
        def checked(item):
            item.clear_cache()
            return item.get_state_set().contains(Atspi.StateType.CHECKED)
        def toggle(item):
            interface=item.get_action_iface()
            names=[Atspi.Action.get_action_name(interface,i) for i in range(Atspi.Action.get_n_actions(interface))]
            for name in ['toggle','click','activate']:
                if name in names:
                    action(item,name)
                    return
            # GTK 4.22's radio nodes have no AT-SPI Action. Enter their ordinary
            # Tab stop and select through the native group keyboard bindings.
            def focused(control):
                control.clear_cache()
                return control.get_state_set().contains(Atspi.StateType.FOCUSED)
            for _ in range(30):
                if focused(normal) or focused(other):break
                keys([0xff09])
                while GLib.MainContext.default().pending():GLib.MainContext.default().iteration(False)
            else:raise RuntimeError('Radio group is not reachable through native keyboard focus')
            for _ in range(3):
                if checked(item):return
                keys([ord(' ') if focused(item) else 0xff52 if item==normal else 0xff54])
                time.sleep(.08)
                while GLib.MainContext.default().pending():GLib.MainContext.default().iteration(False)
            assert checked(item),'Native keyboard navigation did not select the requested radio'
        until('typing selects Other',lambda:checked(other) and not checked(normal))
        toggle(normal)
        until('visible preset selection',lambda:checked(normal) and not checked(other))
        assert text(field)==custom,'Selecting the preset discarded the custom draft'
        if use_other:
            toggle(other)
            until('restored Other selection',lambda:checked(other) and not checked(normal))
        click('提交' if locale=='zh' else 'Submit')
        expected=custom if use_other else 'Use preset'
        def answer():
            return [row for row in outputs.runtime.requests(root) if row.get('id')==identity and 'method' not in row]
        replies=until('installed native input reply',answer)
        assert len(replies)==1 and replies[0].get('result')==dict(answers=dict(answer=dict(answers=[expected]))),replies
        until('closed native question',lambda:find(lambda a:a.get_role_name()=='dialog' and a.get_name()==heading) is None)
        assert text(composer)==DRAFT and document.read_text()==ORIGINAL

    try:
        app = until('owned accessibility registration', application)
        assert app.get_toolkit_name().lower() == 'gtk'
        until('initial reader Preview', lambda: reader_preview(PLAIN))
        until('loaded native conversation', lambda: find(
            lambda a: a.get_name() == 'Native release conversation 中文😀.'))
        controlled.focus_only_window()
        composer = until('native composer', lambda: find(
            lambda a: a.get_role_name() == 'text' and a.get_name().startswith('Write a message.')))
        assert Atspi.EditableText.set_text_contents(composer.get_editable_text_iface(), DRAFT)

        if recovery:
            old_pid=int((root/'backend.pid').read_text())
            (root/'exit-codex-process').write_text('1')
            def replacement():
                try:
                    pid=int((root/'backend.pid').read_text())
                    return pid if pid>0 and pid!=old_pid else None
                except (FileNotFoundError,ValueError):
                    return None  # The fixture rewrites its PID during startup.
            until('replaced owned Codex process',replacement)
            until('recovered verified Send',lambda:find(lambda a:a.get_name()=='Send'
                  and 'button' in a.get_role_name()
                  and a.get_state_set().contains(Atspi.StateType.SENSITIVE)
                  and a.get_state_set().contains(Atspi.StateType.SHOWING)))
            until('recovered authoritative conversation',lambda:find(
                lambda a:a.get_name()=='Native release conversation 中文😀.'))
            assert text(composer)==DRAFT and document.read_text()==ORIGINAL
            assert not Path('/proc',str(old_pid)).exists(),'Crashed owned Codex process was not reaped'

        missed_events=None
        if event_gap:
            provider_pid=int((root/'backend.pid').read_text())
            def reads():
                try:return sum(r.get('method') in ('thread/read','thread/resume') and r.get('params',{}).get('threadId')=='target' for r in outputs.runtime.requests(root))
                except (OSError,ValueError):return -1
            before=reads();assert before>=1
            assert application().get_process_id()==child.pid
            paused=False
            try:
                os.kill(child.pid,signal.SIGSTOP);paused=True
                (root/'emit-event-gap').write_text('1')
                def published():
                    try:return json.loads((root/'gap-published.json').read_text()).get('sequence')==1
                    except (OSError,ValueError):return False
                # No AT-SPI calls while the owned app is paused. Its companion
                # gateway/provider keep running and overrun the bounded stream.
                until('bounded event burst published',published)
            finally:
                if paused:
                    try:os.kill(child.pid,signal.SIGCONT)
                    except ProcessLookupError:pass
            notice=until('native missed-events notice',lambda:find(lambda a:showing(a)
                and re.fullmatch(r'The UI missed (\d+) App Server events',a.get_name() or '')))
            missed_events=int(re.fullmatch(r'The UI missed (\d+) App Server events',notice.get_name()).group(1))
            assert missed_events>0
            until('authoritative history read after real lag',lambda:reads()>before)
            until('recovered unseen history item',lambda:find(lambda a:showing(a)
                and a.get_name()=='Recovered missed events 1 中文😀'))
            until('verified Send after real event gap',lambda:find(lambda a:a.get_name()=='Send'
                and 'button' in a.get_role_name() and showing(a)
                and a.get_state_set().contains(Atspi.StateType.SENSITIVE)))
            assert int((root/'backend.pid').read_text())==provider_pid,'Event gap restarted the provider'
            assert text(composer)==DRAFT and document.read_text()==ORIGINAL

        paragraph = reader_preview(PLAIN)
        continuous = paragraph.get_role_name() == 'text' and paragraph.get_name() == 'Preview'
        # GTK 4.22's AT-SPI Component.GrabFocus is explicitly unsupported.
        # Text selection and native widget actions do not require that API.
        quote = r'$\frac{a}{b}$'
        first = text(paragraph).index('\ufffc') if continuous else PLAIN.index(quote)
        selected_length = 1 if continuous else len(quote)
        interface = paragraph.get_text_iface()
        if Atspi.Text.get_n_selections(interface):
            assert Atspi.Text.set_selection(interface, 0, first, first + selected_length)
        else:
            assert Atspi.Text.add_selection(interface, first, first + selected_length)
        action(paragraph, 'clipboard.copy')

        def copied():
            try:
                return controlled.clipboard_text() == quote
            except GLib.Error:
                return False  # The accessible action is dispatched asynchronously.

        until('independent native selection Copy', copied, seconds=8)
        if continuous:
            assert Atspi.Text.set_selection(interface, 0, 0, Atspi.Text.get_character_count(interface))
            action(paragraph, 'clipboard.copy')
            expected_rendered = 'Release native reader\n\n' + PLAIN + '\n\nx^2 + y^2 = z^2\n\n\nName\tValue\nAlpha\t$x+y$'
            until('installed cross-block rendered Copy', lambda: controlled.clipboard_text() == expected_rendered, seconds=8)
        activity_controls()
        if language:
            language_controls()
        if backends:
            backend_controls()

        diagram_results = {'math': inspect_diagram('math')}

        click('Source')
        view = until('Source text', lambda: source(False))
        assert text(view) == ORIGINAL
        assert Atspi.Text.get_caret_offset(view.get_text_iface()) == 0, 'New Source page starts at EOF'
        click('Edit')
        editor = until('Edit text', lambda: source(True))
        assert text(editor) == ORIGINAL
        assert Atspi.Text.get_caret_offset(editor.get_text_iface()) == 0, 'New Edit page starts at EOF'
        added = '\n' + SAVED + '\n'
        updated = ORIGINAL + added
        assert Atspi.EditableText.insert_text(
            editor.get_editable_text_iface(), len(ORIGINAL), added, len(added.encode('utf-8')))
        until('changed Edit buffer', lambda: text(editor) == updated)
        action(editor, 'text.undo')
        until('native Undo', lambda: text(editor) == ORIGINAL)
        action(editor, 'text.redo')
        until('native Redo', lambda: text(editor) == updated)
        click('Preview')
        until('unchanged reading view', lambda: reader_preview(PLAIN))
        assert document.read_text() == ORIGINAL, 'View switch implicitly saved'
        click('Edit')
        editor = until('retained unsaved Edit buffer', lambda: source(True))
        assert text(editor) == updated
        if viewers:
            draft_entry = GioUnix.DesktopAppInfo.new_from_filename(str(
                prefix / 'share/applications/io.github.messon007.codexthreadstudio.Gnome.desktop'))
            assert draft_entry and draft_entry.launch([Gio.File.new_for_path(str(root / 'project/sample.png'))], None)
            until('image over unsaved editor', lambda: find(
                lambda a: a.get_name() == '3000 × 1000 pixels · Preview 2048 × 683'
                and a.get_state_set().contains(Atspi.StateType.SHOWING)))
            click('Close image')
            editor = until('unsaved editor after viewer return', lambda: source(True))
            assert text(editor) == updated and document.read_text() == ORIGINAL
            assert text(composer) == DRAFT
        click('Save current page (Ctrl+S)')
        until('explicit file save', lambda: document.read_text() == updated)
        click('Preview')
        until('saved Preview', lambda: reader_preview(SAVED))
        assert text(composer) == DRAFT
        if git_review:
            git_controls()

        entry = GioUnix.DesktopAppInfo.new_from_filename(str(
            prefix / 'share/applications/io.github.messon007.codexthreadstudio.Gnome.desktop'))
        assert entry and entry.launch([Gio.File.new_for_path(str(root / 'project/launcher.md'))], None)
        until('installed desktop entry file handoff', lambda: reader_preview('Installed launcher handoff 中文😀'))
        assert application().get_process_id() == child.pid
        assert text(composer) == DRAFT
        if viewers:
            # Observe lazy engine startup before the first intentional EPUB open.
            assert memory.values['webContentPeakRssKiB'] == 0, 'Unused browser/EPUB engine started'
            assert entry.launch([Gio.File.new_for_path(str(root/'project/table-images.md'))],None)
            until('installed table image document',lambda:reader_preview('Release table images'))
            click('Read full cell')
            until('installed full-cell reader',lambda:find(lambda a:showing(a) and a.get_name()=='Table cell' and a.get_role_name() in ('frame','dialog','window')))
            click('Open image viewer')
            until('installed cell-to-image handoff',lambda:find(lambda a:showing(a) and a.get_name()=='3000 × 1000 pixels · Preview 2048 × 683'))
            assert not find(lambda a:showing(a) and a.get_name()=='Table cell' and a.get_role_name() in ('frame','dialog','window'))
            click('Close image')
            until('installed table image return',lambda:reader_preview('Release table images'))
            assert text(composer)==DRAFT
            assert entry.launch([Gio.File.new_for_path(str(root/'project/launcher.md'))],None)
            until('installed launcher document restored',lambda:reader_preview('Installed launcher handoff 中文😀'))

            for filename, close_name, ready in [
                ('sample.png', 'Close image', lambda a: a.get_name() == '3000 × 1000 pixels · Preview 2048 × 683'),
                ('svg-bitmaps.svg', 'Close image', lambda a: a.get_name() == '400 × 240 pixels · Preview 2048 × 1229'),
                ('animation.gif', 'Close image', lambda a: a.get_name() == 'Pause animation'),
                ('animation.png', 'Close image', lambda a: a.get_name() == 'Replay animation'),
                ('animation.webp', 'Close image', lambda a: a.get_name() == 'Replay animation'),
                ('viewer.patch', 'Close source', lambda a: a.get_role_name() == 'text' and (text(a) or '').startswith('--- a/example')),
                ('viewer.csv', 'Close table', lambda a: a.get_name() == '1–3 of 3 rows'),
                ('viewer.pdf', 'Close PDF', lambda a: a.get_name() == 'Copy selected or page text' and a.get_state_set().contains(Atspi.StateType.SENSITIVE)),
                ('viewer.epub', 'Close EPUB', lambda a: a.get_name() == 'Next page or chapter' and a.get_state_set().contains(Atspi.StateType.SENSITIVE)),
            ]:
                assert entry.launch([Gio.File.new_for_path(str(root / 'project' / filename))], None)
                until('installed viewer: ' + filename, lambda: find(lambda a: showing(a) and ready(a)))
                if filename.startswith('animation.'):
                    # A finished finite animation has a stable Replay action;
                    # the longer fixture dwell leaves time for actual AT-SPI
                    # Play/Pause actions without an application test hook.
                    if filename != 'animation.gif':
                        click('Replay animation')
                    click('Pause animation')
                    until('installed animation paused: ' + filename, lambda: find(
                        lambda a: showing(a) and a.get_name() == 'Play animation'))
                    click('Play animation')
                    until('installed animation resumed: ' + filename, lambda: find(
                        lambda a: showing(a) and a.get_name() == 'Pause animation'))
                if filename == 'svg-bitmaps.svg':
                    click('Save original image…')
                    until('installed SVG bitmap original saved',lambda:saved_svg.is_file()
                        and saved_svg.read_bytes()==(root/'project/svg-bitmaps.svg').read_bytes())
                if filename == 'viewer.pdf' and language:
                    pdf_language_controls()
                if filename == 'viewer.patch':
                    click('Find in source')
                    search = until('source find entry', lambda: find(lambda a: showing(a)
                        and a.get_name() == 'Find in source' and a.get_editable_text_iface()))
                    keys([0xffe3, ord('f')])
                    assert Atspi.EditableText.set_text_contents(search.get_editable_text_iface(), 'after  ')
                    until('literal source search', lambda: find(lambda a: showing(a) and a.get_name() == '1 of 2 matches'))
                    keys([0xff0d])
                    until('source Enter next', lambda: find(lambda a: showing(a) and a.get_name() == '2 of 2 matches'))
                    keys([0xffe1, 0xff0d])
                    until('source Shift Enter previous', lambda: find(lambda a: showing(a) and a.get_name() == '1 of 2 matches'))
                    keys([0xffe3, ord('g')])
                    until('source Ctrl G next', lambda: find(lambda a: showing(a) and a.get_name() == '2 of 2 matches'))
                    keys([0xff1b])
                    until('source Escape closes find', lambda: not find(lambda a: showing(a) and a.get_name() == 'Close search'))
                    assert (root/'project/viewer.patch').read_text().endswith('+after  second\n')
                    patch = root/'project/viewer.patch'
                    original_patch = patch.read_text()
                    # Find leaves the selected occurrence's context open. Edit
                    # that current page, rather than assuming the first page.
                    saved_page = until('source page after find', lambda: find(lambda a: showing(a)
                        and a.get_role_name() == 'text' and not a.get_state_set().contains(Atspi.StateType.EDITABLE)
                        and (text(a) or '').startswith('+after  second')))
                    original_page = text(saved_page)
                    click('Edit')
                    code = until('native code editor', lambda: find(lambda a: showing(a)
                        and a.get_role_name() == 'text' and a.get_state_set().contains(Atspi.StateType.EDITABLE)
                        and text(a) == original_page))
                    updated_page = original_page.replace('after  second', 'edited 中文😀')
                    updated_patch = original_patch.replace('after  second', 'edited 中文😀')
                    assert Atspi.EditableText.set_text_contents(code.get_editable_text_iface(), updated_page)
                    click('Find in editor')
                    editor_find = until('editor find entry', lambda: find(lambda a: showing(a)
                        and a.get_name() == 'Find in editor' and a.get_editable_text_iface()))
                    assert Atspi.EditableText.set_text_contents(editor_find.get_editable_text_iface(), 'edited 中文😀')
                    until('unsaved editor match', lambda: find(lambda a: showing(a) and a.get_name() == '1 of 1 matches'))
                    assert patch.read_text() == original_patch, 'Editor Find saved code implicitly'
                    keys([0xffe3, ord('g')])
                    until('editor next match', lambda: find(lambda a: showing(a) and a.get_name() == '1 of 1 matches'))
                    keys([0xff1b])
                    until('editor Escape closes find', lambda: not find(lambda a: showing(a) and a.get_name() == 'Close search'))
                    click('Source code')
                    assert patch.read_text() == original_patch, 'Source switch saved code implicitly'
                    until('saved source before explicit save', lambda: find(lambda a: showing(a)
                        and a.get_role_name() == 'text' and text(a) == original_page))
                    click('Edit')
                    until('retained native code edits', lambda: find(lambda a: showing(a)
                        and a.get_state_set().contains(Atspi.StateType.EDITABLE) and text(a) == updated_page))
                    keys([0xffe3, ord('s')])
                    until('native code explicit save', lambda: patch.read_text() == updated_patch)
                click(close_name)
                until('viewer return: ' + filename, lambda: reader_preview('Installed launcher handoff 中文😀'))
                assert text(composer) == DRAFT
            until('intentional EPUB engine observed', lambda: memory.values['webContentPeakRssKiB'] > 0)
        if all_diagrams:
            for kind in ['mermaid', 'plantuml', 'd2', 'math']:
                assert entry.launch([Gio.File.new_for_path(str(root / f'project/diagram-{kind}.md'))], None)
                until('installed diagram file: ' + kind, lambda: reader_preview(f'Release diagram {kind}'))
                diagram_results[kind] = inspect_diagram(kind, steps=7)
                assert text(composer) == DRAFT
        if remote_images:
            remote_document=root/'project/remote-installed.md'
            before=remote_document.read_bytes()
            assert entry.launch([Gio.File.new_for_path(str(remote_document))],None)
            until('installed HTTPS reader',lambda:reader_preview('Remote installed image'))
            click('Open image viewer')
            until('installed HTTPS pixels',lambda:find(lambda a:a.get_name()=='512 × 512 pixels'))
            click('Save original image…')
            until('installed HTTPS original saved',lambda:(root/'saved release remote.png').is_file()
                and (root/'saved release remote.png').read_bytes()==(root/'remote-original.png').read_bytes())
            click('Close image')
            # AT-SPI actions are dispatched asynchronously. Observe the actual
            # reader return before sending another desktop activation.
            until('installed HTTPS viewer closed',lambda:reader_preview('Remote installed image') and not find(lambda a:showing(a)
                and a.get_name()=='Close image' and 'button' in a.get_role_name()))
            assert remote_document.read_bytes()==before
            assert text(composer)==DRAFT
        image = root / 'project/sample.png'
        assert entry.launch([Gio.File.new_for_path(str(image))], None)
        until('original image preview', lambda: find(
            lambda a: showing(a) and a.get_name() == '3000 × 1000 pixels · Preview 2048 × 683'))
        click('Save original image…')
        until('original image saved', lambda: saved_image.is_file()
              and saved_image.read_bytes() == image.read_bytes())
        assert len(portal.calls) == 1 + int(viewers) + int(remote_images) + 6*int(language) and len(portal.plan) == 1 and not portal.objects
        keys([0xffe3, ord('o')])  # Real Ctrl+O through the installed app action.
        until('Ctrl+O opens chosen source', lambda: find(lambda a: showing(a)
            and a.get_role_name() == 'text' and text(a) == chosen_bytes.decode()))
        assert chosen_source.read_bytes() == chosen_bytes, 'Opening code changed its bytes'
        reloaded_bytes=chosen_bytes+b'// Changed outside the installed app\n'
        chosen_source.write_bytes(reloaded_bytes)
        click('Reload saved document')
        until('installed ordinary-file Reload',lambda:find(lambda a:showing(a) and a.get_role_name()=='text' and text(a)==reloaded_bytes.decode()))
        assert chosen_source.read_bytes()==reloaded_bytes,'Reload wrote the source file'
        assert len(portal.calls) == 2 + int(viewers) + int(remote_images) + 6*int(language) and not portal.plan and not portal.objects
        assert portal.calls[-1]['options']['filters'][-1] == ('All files', [(0, '*')])
        click('Close source')
        if viewers:
            until('chosen Source close completed',lambda:not find(lambda a:showing(a) and a.get_name()=='Close source'))
            structured_file=root/'project/installed-structured.json5'
            assert entry.launch([Gio.File.new_for_path(str(structured_file))],None)
            until('installed formatted structured Preview',lambda:find(lambda a:showing(a) and a.get_role_name()=='text' and text(a)==STRUCTURED_PREVIEW))
            click('Source code')
            until('installed original structured Source',lambda:find(lambda a:showing(a) and a.get_role_name()=='text' and text(a)==STRUCTURED_SOURCE))
            click('Edit')
            edited=until('installed structured editor',lambda:find(lambda a:showing(a) and a.get_role_name()=='text'
                and a.get_state_set().contains(Atspi.StateType.EDITABLE) and text(a)==STRUCTURED_SOURCE.replace('\r\n','\n')))
            assert Atspi.EditableText.set_text_contents(edited.get_editable_text_iface(), '// edited installed 中文😀\n'+STRUCTURED_SOURCE.replace('\r\n','\n'))
            click('Preview')
            assert structured_file.read_bytes()==STRUCTURED_SOURCE.encode(),'View switch saved structured edits'
            until('installed dirty structured Preview retained snapshot',lambda:find(lambda a:showing(a) and a.get_role_name()=='text' and text(a)==STRUCTURED_PREVIEW))
            click('Save current page (Ctrl+S)')
            until('installed structured original save',lambda:structured_file.read_bytes()=='// edited installed 中文😀\r\n'.encode()+STRUCTURED_SOURCE.encode())
            until('installed structured Preview refreshed',lambda:find(lambda a:showing(a) and a.get_role_name()=='text' and (text(a) or '').startswith('// edited installed 中文😀\n// Installed JSON5')))
            click('Close source')
            until('structured Source close completed',lambda:not find(lambda a:showing(a) and a.get_name()=='Close source'))
            html_file=root/'project/installed-passive.html'
            assert entry.launch([Gio.File.new_for_path(str(html_file))],None)
            until('installed passive HTML Preview',lambda:find(lambda a:showing(a) and a.get_role_name()=='heading' and a.get_name()=='Installed HTML original'))
            click('Source code')
            until('installed original HTML Source',lambda:find(lambda a:showing(a) and a.get_role_name()=='text' and text(a)==HTML_SOURCE))
            click('Edit')
            html_editor=until('installed HTML editor',lambda:find(lambda a:showing(a) and a.get_role_name()=='text'
                and a.get_state_set().contains(Atspi.StateType.EDITABLE) and text(a)==HTML_SOURCE.replace('\r\n','\n')))
            assert Atspi.EditableText.set_text_contents(html_editor.get_editable_text_iface(), '<h1>Installed saved addition</h1>\n'+HTML_SOURCE.replace('\r\n','\n'))
            click('Preview')
            assert html_file.read_bytes()==HTML_SOURCE.encode(),'HTML view switch wrote edits'
            until('installed dirty HTML retained snapshot',lambda:find(lambda a:showing(a) and a.get_role_name()=='heading' and a.get_name()=='Installed HTML original'))
            click('Save current page (Ctrl+S)')
            until('installed HTML original save',lambda:html_file.read_bytes()=='<h1>Installed saved addition</h1>\r\n'.encode()+HTML_SOURCE.encode())
            until('installed HTML Preview refreshed',lambda:find(lambda a:showing(a) and a.get_role_name()=='heading' and a.get_name()=='Installed saved addition'))
            click('Close source')
        assert not list(root.glob('.tmp*')), 'Original image save retained temporary files'
        assert text(composer) == DRAFT
        keys([0xffe9, 0xffc1])  # Alt+F4: normal owned-window close, not a signal.
        child.wait(timeout=15)
        assert child.returncode == 0
        cleanup_started = time.monotonic()
        for name in ['backend.pid', 'opencode.pid']:
            if (root / name).exists():
                # The native process closes its parent pipe on exit. The sibling
                # gateway then reaps its providers asynchronously; observe that
                # bounded lifecycle rather than asserting an immediate PID race.
                pid = (root / name).read_text().strip()
                deadline = time.monotonic() + 10
                while Path('/proc', pid).exists() and time.monotonic() < deadline:
                    time.sleep(.05)
                assert not Path('/proc', pid).exists(), ('Owned provider survived cleanup', name, pid)
        provider_cleanup_seconds = time.monotonic() - cleanup_started
        assert not outputs.runtime.starts(root), 'Acceptance unexpectedly sent a model prompt'
        if recovery:
            started=[json.loads(line) for line in (root/'codex-process-starts.jsonl').read_text().splitlines()]
            exited=[json.loads(line) for line in (root/'codex-process-exits.jsonl').read_text().splitlines()]
            assert len(exited)==1 and exited[0]['code']==23 and len({e['pid'] for e in started})>=2,(started,exited)
            assert not any(Path('/proc',str(e['pid'])).exists() for e in started),'Owned replacement process survived close'
        runtime_log = (root / 'app.log').read_text()
        assert not any(token in runtime_log for token in ['CRITICAL', 'panicked', 'Theme parser error', 'Gtk-WARNING']), runtime_log[-2000:]
        measured = memory.close()
        assert measured['gatewayPeakRssKiB'] > 0, 'Sibling gateway was not observed'
        if not viewers:
            assert measured['webContentProcessesObserved'] == 0, 'Unused browser/EPUB engine started'
        else:
            deadline = time.monotonic() + 10
            while memory.web_content_alive() and time.monotonic() < deadline:
                time.sleep(.1)
            assert not memory.web_content_alive(), 'Owned EPUB content survived normal close'
        result = dict(ok=True, applicationHooks=False, installedPair=True,
                      providerCleanupSeconds=provider_cleanup_seconds,
                      nativeAccessibility=True, independentWaylandClipboard=True,
                      nativeContinuousPreview=continuous, nativeCrossBlockCopy=continuous,
                      continuousPreviewOverride=os.environ.get('STUDIO_GNOME_CONTINUOUS_PREVIEW'),
                      sourceEditStart=True, undoRedo=True, dirtyViewSwitch=True,
                      explicitSave=True, desktopEntryHandoff=True, draftRetained=True,
                      originalImageSave=True, nativeSavePortal=True, nativeOpenFileShortcut=True,
                      nativeDiagramInspector=True, diagramZoomFit=True, vectorZoomRaster=True,
                      liveLanguageControls=language,
                      nativeHistoryLanguage=language,
                      nativeTranscriptActionLanguage=language,
                      nativeComposerAccessibleLanguage=language,
                      nativeComparisonLanguage=language,
                      nativeImageLanguage=language, nativeNewSessionLanguage=language,
                      nativeCommandStatusPermissionLanguage=language,
                      nativeDiagramInspectorLanguage=language,
                      nativeDocumentStatusLanguage=language,
                      nativeQuestionSelection=language,
                      nativeBackendFileSave=backends,
                      nativeConnectionDiagnostics=backends,
                      nativeActivityInspection=True,
                      nativeGitReview=git_review,
                      nativeViewerReturns=viewers,
                      nativeStructuredPreview=viewers,
                      nativeStructuredOriginalSave=viewers,
                      nativeStaticHtmlPreview=viewers,
                      nativeStaticHtmlOriginalSave=viewers,
                      nativePdfLiveLanguage=viewers and language,
                      nativeTableImages=viewers,
                      nativeCellViewerHandoff=viewers,
                      nativeSvgBitmapSave=viewers,
                      nativeAnimationControls=viewers,
                      nativeRemoteImages=remote_images,
                      nativeSourceSearch=viewers,
                      nativeSourceEditing=viewers,
                      nativeSourceReload=True,
                      nativeEditorFind=viewers,
                      liveGitAccessibleLanguage=git_review and language,
                      nativeProcessRecovery=recovery,
                      nativeEventGapRecovery=event_gap,
                      missedEvents=missed_events,
                      diagrams=diagram_results,
                      backendCleanup=True, lazyWebContent=True, memory=measured,
                      artifacts=str(root))
        (root / 'release.json').write_text(json.dumps(result, indent=2))
        print(json.dumps(result))
    except BaseException:
        (root / 'failed-step.txt').write_text(step)
        try:
            (root / 'failed-buttons.json').write_text(json.dumps([
                dict(name=a.get_name(),role=a.get_role_name(), showing=a.get_state_set().contains(Atspi.StateType.SHOWING))
                for a in tree()], ensure_ascii=False))
        except Exception:
            pass  # Preserve the original failure if the app has already exited.
        raise
    finally:
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=10)
        memory.close()
        portal.close()
        controlled.close()
        log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', help='Directory containing the packaged native executable and gateway')
    parser.add_argument('--private-root', type=Path)
    parser.add_argument('--all-diagrams', action='store_true', help='exercise all four installed diagram engines at 4x zoom; requires packaged D2 and PlantUML plus Java/bubblewrap')
    parser.add_argument('--language', action='store_true', help='switch languages and verify native questions; with --viewers also retain PDF page/query/text/clipboard across locale changes')
    parser.add_argument('--backends', action='store_true', help='validate and explicitly save backend JSON through installed native controls')
    parser.add_argument('--git', action='store_true', help='filter native Git changes, open a file, return to Review and explicitly stage a private fixture file')
    parser.add_argument('--viewers', action='store_true', help='open/close image, source, table, PDF and EPUB through installed desktop activation and verify parent return')
    parser.add_argument('--recovery', action='store_true', help='exit the owned fake Codex process and verify automatic recovery and draft retention without application hooks')
    parser.add_argument('--event-gap', action='store_true', help='briefly pause the owned native process, overrun its gateway stream, and verify actual missed-event recovery')
    parser.add_argument('--remote-fixture', type=Path, help='explicit pinned public artwork fixture for installed HTTPS image/viewer/original Save checks')
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    if args.private_root:
        return run(args.private_root, args.all_diagrams, args.language, args.backends, args.git, args.viewers, args.recovery, args.event_gap, bool(args.remote_fixture))
    bundle = Path(args.bundle).resolve()
    for name in ['codex-thread-studio-gnome', 'codex-thread-studio-gateway']:
        if not (bundle / name).is_file():
            raise RuntimeError('Build/package both native executables first: ' + name)
    if args.all_diagrams:
        for name in ['d2', 'plantuml.jar']:
            if not (bundle / name).is_file():
                raise RuntimeError('Missing packaged diagram runtime: ' + name)
    for notice in ['epub-cfi/LICENSE', 'cssparser/LICENSE', 'html5ever/LICENSE',
                   'pele/LICENSE', 'ratex/LICENSE', 'ratex/OFL.txt', 'ratex/FONT_NOTICE.txt',
                   'i18n/LICENSE', 'i18n/UPSTREAM.md']:
        if not (bundle / 'licenses' / notice).is_file():
            raise RuntimeError('Missing packaged notice: ' + notice)
    base = Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or REPO / 'target/gnome-native-smoke')
    base.mkdir(exist_ok=True, parents=True)
    root = Path(tempfile.mkdtemp(prefix='release-ui-', dir=base))
    outputs.runtime.create_profile(root)
    if args.remote_fixture:
        import hashlib
        import shutil
        fixture=args.remote_fixture.resolve(strict=True)
        provenance=json.loads((fixture/'ORIGIN.json').read_text())
        png=provenance['files']['expected.png']
        assert hashlib.sha256((fixture/'expected.png').read_bytes()).hexdigest()==png['sha256']
        assert png['url'].startswith('https://raw.githubusercontent.com/rust-lang/rust-artwork/'+provenance['commit']+'/')
        shutil.copy2(fixture/'expected.png',root/'remote-original.png')
        shutil.copy2(fixture/'ARTWORK-LICENSE.md',root/'ARTWORK-LICENSE.md')
        shutil.copy2(fixture/'ORIGIN.json',root/'REMOTE-ORIGIN.json')
        (root/'project/remote-installed.md').write_text('# Remote installed image\n\n![Installed HTTPS PNG]('+png['url']+')\n')
    preferences=root/'config/codex-thread-studio/settings.json'
    raw=json.loads(preferences.read_text());raw['language']='en-US';preferences.write_text(json.dumps(raw))
    (root / 'project/open file 中文.rs').write_text('fn main() { println!("Open file 中文😀"); }\n')
    (root / 'project/release.md').write_text(ORIGINAL)
    (root / 'project/launcher.md').write_text('# Installed launcher handoff 中文😀\n')
    for kind, source in dict(mermaid='flowchart LR\nA[Native chat] --> B[Reader]',
                             plantuml='@startuml\nAlice -> Bob: Native chat\n@enduml',
                             d2='chat -> reader: Native chat', math=r'x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}').items():
        (root / f'project/diagram-{kind}.md').write_text(f'# Release diagram {kind}\n\n```{kind}\n{source}\n```\n')
    image_spec = importlib.util.spec_from_file_location('release_images', REPO / 'scripts/smoke-gnome-images.py')
    images = importlib.util.module_from_spec(image_spec)
    image_spec.loader.exec_module(images)
    (root / 'project/sample.png').write_bytes(images.png(3000, 1000))
    if args.viewers:
        (root/'project/installed-structured.json5').write_bytes(STRUCTURED_SOURCE.encode())
        (root/'project/installed-passive.html').write_bytes(HTML_SOURCE.encode())
        fixtures=REPO/'src-tauri/tests/fixtures/native-images'
        for extension in ['gif','png','webp']:
            filename='two-frame-infinite.gif' if extension=='gif' else f'release-two-frame.{extension}'
            (root/f'project/animation.{extension}').write_bytes((fixtures/filename).read_bytes())
        (root/'project/svg-bitmaps.svg').write_text('<svg xmlns="http://www.w3.org/2000/svg" width="400" height="240"><rect width="400" height="240" fill="#008866"/><image x="300" y="150" width="80" height="40" href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGO4o8H1H4QBEjMEGyP2UiEAAAAASUVORK5CYII="/></svg>')
        (root/'project/table-images.md').write_text('# Release table images\n\n| Label | Image |\n|---|---|\n| Picture | ![Release table image](sample.png) |\n')
        (root / 'project/viewer.csv').write_text('name,value\nUnicode,你好\nSecond,2\n')
        (root / 'project/viewer.patch').write_text('--- a/example\n+++ b/example\n@@ -1 +1,2 @@\n-before\n+after  first\n+after  second\n')
        for name, filename in [('pdf', 'smoke-gnome-pdf.py'), ('epub', 'smoke-gnome-epub.py')]:
            spec = importlib.util.spec_from_file_location('release_viewer_' + name, REPO / 'scripts' / filename)
            helper = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(helper)
            if name == 'pdf':
                helper.document(root / 'project/viewer.pdf')
            else:
                helper.book(root / 'project/viewer.epub', 'https://example.invalid')
    if args.git:
        git_environment = dict(os.environ, GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_SYSTEM='/dev/null')
        for arguments in [['init', '--initial-branch=main', '--template='],
                          ['config', 'user.name', 'Native installed acceptance'],
                          ['config', 'user.email', 'native-test@example.invalid'],
                          ['config', 'core.hooksPath', '/dev/null'],
                          ['config', 'commit.gpgsign', 'false'],
                          ['add', '--', '.'], ['commit', '-m', 'Native release baseline']]:
            subprocess.run(['git', '-C', str(root / 'project'), *arguments], env=git_environment,
                           check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    (root / 'raw-items.json').write_text(json.dumps([
        ACTIVITY_ITEM, dict(id='release-assistant', type='agentMessage', text='Native release conversation 中文😀.')]))
    (root / 'config/codex-thread-studio/gnome.json').write_text(json.dumps(
        dict(selectedBackend='codex', selectedSession='target')))
    for name, code in zip(['fake-codex.py', 'fake-opencode.py'], outputs.providers()):
        if args.event_gap and name=='fake-codex.py':
            code=instrument_lag(code,events=16384)
        if args.recovery and name=='fake-codex.py':
            code=instrument_exit(code)
        if args.language and name=='fake-codex.py':
            input_spec=importlib.util.spec_from_file_location('release_inputs',REPO/'scripts/smoke-gnome-input.py')
            inputs=importlib.util.module_from_spec(input_spec);input_spec.loader.exec_module(inputs)
            assert code.count('for raw in sys.stdin:')==1
            code=code.replace('for raw in sys.stdin:',inputs.PUBLISH+'\nfor raw in sys.stdin:',1)
        path = root / name
        path.write_text(code)
        path.chmod(0o700)
    prefix = root / 'installed native 中文 % test'
    subprocess.run(['bash', str(REPO / 'scripts/install-gnome.sh'), str(bundle), str(prefix)], check=True)
    env = dict(os.environ,
               CODEX_THREAD_STUDIO_CODEX_BIN=str(root / 'fake-codex.py'),
               CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root / 'fake-opencode.py'),
               CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root / 'config/codex-thread-studio/backends.json'),
               CODEX_HOME=str(root / 'codex'), XDG_CONFIG_HOME=str(root / 'config'),
               XDG_DATA_HOME=str(root / 'data'), XDG_CACHE_HOME=str(root / 'cache'),
               XDG_DATA_DIRS=str(prefix / 'share') + ':/usr/local/share:/usr/share',
               READER_CACHE_DIR=str(root / 'reader-cache'), GDK_BACKEND='wayland', GTK_A11Y='atspi',
               GTK_USE_PORTAL='1', GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix')
    if args.git:
        env.update(GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_SYSTEM='/dev/null')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE_') or key in ['STUDIO_OWNED_MUTTER_INPUT', 'CODEX_THREAD_STUDIO_GATEWAY_BIN']:
            env.pop(key)
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', 'mutter', '--headless', '--wayland', '--no-x11',
                        '--wayland-display', 'studio-test', '--virtual-monitor', '1600x1000', '--',
                        sys.executable, str(Path(__file__).resolve()), str(bundle), '--private-root', str(root),
                        *(['--all-diagrams'] if args.all_diagrams else []),*(['--language'] if args.language else []),
                        *(['--backends'] if args.backends else []), *(['--git'] if args.git else []),
                        *(['--viewers'] if args.viewers else []), *(['--recovery'] if args.recovery else []),
                        *(['--event-gap'] if args.event_gap else []),
                        *(['--remote-fixture',str(args.remote_fixture.resolve())] if args.remote_fixture else [])],
                       cwd=root, env=private, check=True, timeout=200)


if __name__ == '__main__':
    main()
