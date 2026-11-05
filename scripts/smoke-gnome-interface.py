#!/usr/bin/env python3
"""Native interface regression checks plus screenshots for human visual review.

Usage: python3 scripts/smoke-gnome-interface.py BUNDLE --output /tmp/studio-native-qa --phase after
Requires GNOME Shell, python3-gi/AT-SPI and the packaged native executable with
its sibling gateway. Uses the existing owned headless Wayland session, private
provider/profile helpers and real native input/accessibility; no application
hooks, host profiles, model prompts or external input data. All screenshots,
clipboard samples and reports contain only private generated fixture data.

Keeps eleven PNGs for normal/maximized conversation and document views, table
paging, keyboard selection/caret, and dark mode, with geometry/selection JSON.
The final interface.json reports checksPassed, not overall UI acceptance: PNGs
still require visual review. --output defaults to STUDIO_NATIVE_QA_DIR or the
system temporary directory; every run creates a fresh private subdirectory.
"""
import argparse
import importlib.util
import json
import os
import re
import resource
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / 'scripts/test-support'))
from private_wayland import isolated_environment, owned_gnome_shell
from native_accessibility import Client

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def phase_name(value):
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_-]{0,31}', value):
        raise argparse.ArgumentTypeError('phase must be 1–32 ASCII letters, digits, underscores or hyphens, starting with a letter or digit')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', metavar='BUNDLE', type=Path,
                        help='Package directory containing the native executable and sibling gateway')
    parser.add_argument('--output', type=Path,
                        default=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or tempfile.gettempdir()),
                        help='Artifact base directory (default: STUDIO_NATIVE_QA_DIR or the temporary directory)')
    parser.add_argument('--phase', type=phase_name, default='review',
                        help='Safe short screenshot suffix (default: review)')
    parser.add_argument('--inner', type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    bundle = args.bundle.expanduser().resolve(strict=True)
    for name in ['codex-thread-studio-gnome', 'codex-thread-studio-gateway']:
        executable = bundle / name
        if not executable.is_file() or not os.access(executable, os.X_OK):
            parser.error('Missing packaged executable: ' + str(executable))
    base = args.output.expanduser().resolve()
    if args.inner:
        root = args.inner.resolve(strict=True)
        if root.parent != base:
            parser.error('Private child root must be inside the explicit output base')
        return inspect(root, bundle, args.phase)
    base.mkdir(exist_ok=True, parents=True)
    root = Path(tempfile.mkdtemp(prefix='interface-' + args.phase + '-', dir=base))
    outputs = load('quality_outputs', REPO / 'scripts/smoke-gnome-outputs.py')
    outputs.runtime.create_profile(root)
    (root / 'home/Pictures').mkdir(parents=True)
    (root / 'config/user-dirs.dirs').write_text('XDG_PICTURES_DIR="$HOME/Pictures"\n')
    prefs = root / 'config/codex-thread-studio/settings.json'
    value = json.loads(prefs.read_text()); value['language'] = 'en-US'
    prefs.write_text(json.dumps(value))
    (root / 'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex', selectedSession='target')))
    (root / 'raw-items.json').write_text(json.dumps([
        dict(id='user', type='userMessage', content=[dict(type='text', text='Review the implementation and explain the next steps.')]),
        dict(id='assistant', type='agentMessage', text='## Implementation review\n\nThe backend reuses the existing session and document services.\n\n- Review the changes\n- Open the document beside this conversation\n- Send a follow-up message\n\n```rust\nfn main() { println!("Hello, Studio"); }\n```\n\nSee [the project notes](review.md).')
    ]))
    (root / 'project/review.md').write_text('# Project review\n\nThis document should be comfortable to read beside a conversation.\n\n## Tasks\n\n| Task | Status |\n|---|---|\n| Session navigation | Review |\n| Conversation layout | Review |\n| Document reading | Review |\n\n## Details\n\nReadable content, consistent controls and clear focus matter.\n')
    (root / 'project/paging.md').write_text('# Table paging\n\n'+('Context before the final table.\n\n'*18)+'| Task | Status |\n|---|---|\n'+''.join('| row-'+str(i)+' | Review |\n' for i in range(1,52)))
    for name, code in zip(('fake-codex.py', 'fake-opencode.py'), outputs.providers()):
        p = root / name; p.write_text(code); p.chmod(0o700)
    env = dict(os.environ, HOME=str(root / 'home'),
        CODEX_THREAD_STUDIO_CODEX_BIN=str(root / 'fake-codex.py'),
        CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root / 'fake-opencode.py'),
        CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root / 'config/codex-thread-studio/backends.json'),
        CODEX_HOME=str(root / 'codex'), XDG_CONFIG_HOME=str(root / 'config'),
        XDG_DATA_HOME=str(root / 'data'), XDG_CACHE_HOME=str(root / 'cache'),
        READER_CACHE_DIR=str(root / 'reader-cache'), GDK_BACKEND='wayland',
        GTK_A11Y='atspi', GTK_USE_PORTAL='1', GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE_') or key in ('CODEX_THREAD_STUDIO_GATEWAY_BIN', 'AT_SPI_BUS_ADDRESS', 'STUDIO_OWNED_MUTTER_INPUT'):
            env.pop(key)
    print('Review root:', root, flush=True)
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', sys.executable, str(Path(__file__).resolve()),
                        str(bundle), '--output', str(base), '--phase', args.phase, '--inner', str(root)],
                       env=private, cwd=root, check=True, timeout=150)

def inspect(root, bundle, phase):
    from gi.repository import GLib
    outputs = load('quality_outputs', REPO / 'scripts/smoke-gnome-outputs.py')
    with owned_gnome_shell(root, monitors=('1600x1000',)) as (shell, input):
        input.enable_clipboard_reader()
        def keys(codes):
            for key in codes: input.call('NotifyKeyboardKeysym', GLib.Variant('(ub)', (key, True)))
            time.sleep(.08)
            for key in reversed(codes): input.call('NotifyKeyboardKeysym', GLib.Variant('(ub)', (key, False)))
        with (root / 'app.log').open('w') as log:
            child = subprocess.Popen([str(bundle / 'codex-thread-studio-gnome')], env=dict(os.environ), cwd=root, stdout=log, stderr=log)
            try:
                client = Client(child)
                def focus(node):
                    input.focus_only_window()
                    for _ in range(80):
                        node.clear_cache()
                        if node.get_state_set().contains(client.api.StateType.FOCUSED):return
                        keys([0xff09])
                    raise RuntimeError('Control is not reachable through the native Tab chain: '+node.get_name())
                client.wait('application mapped', client.application)
                client.wait('conversation controls', lambda: client.find(lambda n: n.get_name()=='Send' and client.showing(n)), seconds=40)
                time.sleep(2)
                keys([0xffeb, 0xff52])
                time.sleep(.6)
                shots=[]; samples=[]
                def shot(name):
                    before=set((root/'home').rglob('*.png'))
                    keys([0xffe9, 0xff61])
                    deadline=time.monotonic()+15
                    while time.monotonic()<deadline:
                        found=next((p for p in (root/'home').rglob('*.png') if p not in before and p.stat().st_size>24),None)
                        if found:
                            dest=root/(name+'.png'); shutil.copy2(found,dest); shots.append(str(dest))
                            (root/'screenshots.json').write_text(json.dumps(shots,indent=2))
                            controls=[]
                            names={'Close','Minimize','Maximize','Main menu','Send','Stop','Queue','Continue','Open document','Close document','Toggle session sidebar'}
                            for node in client.tree():
                                if client.showing(node) and (node.get_name() in names or node.get_name().startswith('Write a message.')):
                                    component=node.get_component_iface()
                                    if component:
                                        b=component.get_extents(client.api.CoordType.WINDOW)
                                        controls.append(dict(name=node.get_name(),role=node.get_role_name(),x=b.x,y=b.y,width=b.width,height=b.height))
                            samples.append(dict(name=name,controls=controls))
                            (root/'control-geometry.json').write_text(json.dumps(samples,ensure_ascii=False,indent=2))
                            return
                        time.sleep(.1)
                    raise RuntimeError('GNOME screenshot timed out: '+name)
                shot('conversation-'+phase)
                (root/'conversation-accessibility.json').write_text(json.dumps(client.describe(),ensure_ascii=False,indent=2))
                composer=client.wait('composer',lambda:client.find(lambda n:n.get_name().startswith('Write a message.') and n.get_role_name()=='text' and client.showing(n)))
                # GTK/Wayland intentionally does not expose global coordinates;
                # reach the editor through the real native keyboard focus chain.
                focus(composer)
                for character in 'interface review': keys([ord(character)])
                client.wait('ordinary keyboard draft',lambda:client.contents(composer)=='interface review')
                subprocess.run([str(bundle / 'codex-thread-studio-gnome'), str(root/'project/review.md')],check=True,timeout=15,capture_output=True)
                client.wait('document opened',lambda:client.find(lambda n:n.get_name()=='Close document' and client.showing(n)),seconds=30)
                time.sleep(1)
                def small_table_visible():
                    metrics=[]
                    for name in ['Session navigation','Conversation layout','Document reading']:
                        node=client.find(lambda n:n.get_name()==name and n.get_role_name()=='label' and client.showing(n))
                        if not node:return False
                        parent=node
                        for _ in range(18):
                            if parent is None:return False
                            if parent.get_role_name()=='scroll pane':break
                            parent=parent.get_parent()
                        else:return False
                        cell=node.get_component_iface().get_extents(client.api.CoordType.WINDOW)
                        viewport=parent.get_component_iface().get_extents(client.api.CoordType.WINDOW)
                        if cell.height<=0 or cell.y<viewport.y-1 or cell.y+cell.height>viewport.y+viewport.height+1:return False
                        metrics.append(dict(text=name,y=cell.y,height=cell.height,viewportY=viewport.y,viewportHeight=viewport.height))
                    return metrics
                visible=client.wait('all three small-table rows actually visible',small_table_visible)
                (root/'small-table-visibility.json').write_text(json.dumps(visible,indent=2))
                shot('document-'+phase)
                (root/'document-accessibility.json').write_text(json.dumps(client.describe(),ensure_ascii=False,indent=2))
                keys([0xffeb,0xff54])
                time.sleep(.7)
                shot('document-normal-'+phase)
                client.click('Close document')
                client.wait('document actually closed',lambda:not client.find(lambda n:n.get_name()=='Close document' and client.showing(n)))
                time.sleep(.35)
                composer=client.wait('composer after document close',lambda:client.find(lambda n:n.get_name().startswith('Write a message.') and n.get_role_name()=='text' and client.showing(n)))
                client.wait('draft retained after document close',lambda:client.contents(composer)=='interface review')
                shot('conversation-normal-'+phase)
                # Real end-of-document paging, not simulated row-height math.
                keys([0xffeb,0xff52]);time.sleep(.4)
                subprocess.run([str(bundle/'codex-thread-studio-gnome'),str(root/'project/paging.md')],check=True,timeout=15,capture_output=True)
                preview=client.wait('paging Preview',lambda:client.find(lambda n:n.get_name()=='Preview' and n.get_role_name()=='text' and client.showing(n) and (client.contents(n) or '').startswith('Table paging')))
                focus(preview);keys([0xffe3,0xff57]);time.sleep(.8)
                next_button=client.wait('next table page',lambda:client.find(lambda n:n.get_name()=='Next table page' and n.get_role_name()=='button' and client.showing(n) and client.sensitive(n)))
                def table_position(node):
                    node.clear_cache()
                    b=node.get_component_iface().get_extents(client.api.CoordType.WINDOW)
                    return dict(x=b.x,y=b.y,width=b.width,height=b.height)
                first=table_position(next_button)
                assert first['y']>0 and first['height']>0,first
                shot('paging-first-'+phase)
                client.activate(next_button)
                client.wait('last actual row',lambda:client.find(lambda n:n.get_name()=='row-51' and n.get_role_name()=='label' and client.showing(n)))
                time.sleep(.4)
                last=table_position(next_button)
                assert abs(first['y']-last['y'])<=1,(first,last,'Table paging shifted outer content')
                shot('paging-last-'+phase)
                client.click('Previous table page')
                client.wait('first actual row restored',lambda:client.find(lambda n:n.get_name()=='row-1' and n.get_role_name()=='label' and client.showing(n)))
                time.sleep(.4)
                restored=table_position(next_button)
                assert abs(first['y']-restored['y'])<=1,(first,restored,'Previous page shifted outer content')
                (root/'table-paging-position.json').write_text(json.dumps(dict(first=first,last=last,restored=restored),indent=2))
                focus(preview)
                iface=preview.get_text_iface()
                count=client.api.Text.get_character_count(iface)
                keys([0xffe3,0xff57]);keys([0xffe3,0xffe1,0xff50])
                client.wait('native ShiftCtrlHome selection',lambda:client.api.Text.get_n_selections(iface)==1)
                selected=client.api.Text.get_selection(iface,0)
                assert (selected.start_offset,selected.end_offset)==(0,count),(selected.start_offset,selected.end_offset,count)
                keys([0xffe3,0xff50]);keys([0xffe3,0xffe1,0xff57])
                client.wait('native ShiftCtrlEnd selection',lambda:client.api.Text.get_n_selections(iface)==1)
                selected=client.api.Text.get_selection(iface,0)
                assert (selected.start_offset,selected.end_offset)==(0,count)
                keys([0xffe3,ord('c')])
                copied=input.clipboard_text()
                assert 'row-51' in copied and 'Table paging' in copied,copied[-250:]
                shot('selection-across-table-'+phase)
                keys([0xffe3,0xff57]);time.sleep(.2)
                assert client.api.Text.get_caret_offset(iface)==count
                assert client.api.Text.get_n_selections(iface)==0
                shot('object-caret-'+phase)
                (root/'native-keyboard-selection.json').write_text(json.dumps(dict(characters=count,selection=[selected.start_offset,selected.end_offset],copiedBytes=len(copied.encode()),tableLastRowCopied=True,caretRestored=True),indent=2))
                contents=client.contents(preview)
                target=contents.rfind('Context before the final table.')+len('Context before the final table.')
                assert target>=len('Context before the final table.')
                iface=preview.get_text_iface()
                before_offset=client.api.Text.get_caret_offset(iface)
                assert client.api.Text.set_caret_offset(iface,target)
                time.sleep(.3)
                after_offset=client.api.Text.get_caret_offset(iface)
                assert after_offset==target,(after_offset,target)
                after_caret=table_position(next_button)
                assert abs(after_caret['y']-restored['y'])<=1,(after_caret,restored)
                shot('caret-in-text-'+phase)
                (root/'caret-confirmation.json').write_text(json.dumps(dict(beforeOffset=before_offset,afterOffset=after_offset,tableBefore=restored,tableAfter=after_caret),indent=2))
                client.click('Main menu')
                for label in ['Git Review','Compare Markdown files']:
                    client.wait('disclosed workspace action '+label,lambda label=label:client.find(lambda n:n.get_name()==label and n.get_role_name()=='menu item' and client.showing(n)))
                dark=client.wait('dark appearance menu item',lambda:client.find(lambda n:n.get_name()=='Dark appearance' and n.get_role_name()=='menu item'))
                for _ in range(80):
                    dark.clear_cache()
                    if dark.get_state_set().contains(client.api.StateType.FOCUSED):break
                    keys([0xff54])
                else:raise RuntimeError('Dark appearance is not keyboard-reachable in Main menu')
                keys([0x20])
                client.wait('dark appearance persisted',lambda:json.loads((root/'config/codex-thread-studio/gnome.json').read_text()).get('theme')=='dark')
                time.sleep(.6)
                shot('document-dark-'+phase)
                client.click('Close document')
                client.wait('dark document actually closed',lambda:not client.find(lambda n:n.get_name()=='Close document' and client.showing(n)))
                time.sleep(.35)
                shot('conversation-dark-'+phase)
                (root/'screenshots.json').write_text(json.dumps(shots,indent=2))
                assert len(shots) == 11, ('Missing visual review screenshots', shots)
                assert not outputs.runtime.starts(root), 'Interface review unexpectedly sent a Codex prompt'
                requests = root/'opencode-requests.jsonl'
                if requests.exists():
                    assert not any(json.loads(line)['path'].endswith(('/prompt_async', '/command'))
                                   for line in requests.read_text().splitlines()), 'Interface review unexpectedly sent an OpenCode prompt'
                result = dict(checksPassed=True, visualReviewRequired=True,
                              bundle=str(bundle), phase=phase, artifacts=str(root), screenshots=shots,
                              scope='Actual native package; private generated profiles/data and mock providers only. Checks passed; PNG review is still required for UI acceptance.')
                (root/'interface.json').write_text(json.dumps(result,indent=2))
                print(json.dumps(result),flush=True)
            finally:
                if child.poll() is None:
                    child.terminate()
                    try:child.wait(timeout=12)
                    except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)

if __name__ == '__main__': main()
