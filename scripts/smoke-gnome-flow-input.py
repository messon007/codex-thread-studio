#!/usr/bin/env python3
"""Full reader projections with actual input on a fixture-owned Wayland desktop."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode=True
REPO=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(REPO/'scripts/test-support'))
from private_wayland import isolated_environment, OwnedMutterInput
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('flow_input_runtime',REPO/'scripts/smoke-gnome-router-runtime.py')
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def accessibility(root, app):
    """Inspect only the fixture child on the private AT-SPI bus."""
    path=root/'clipboard-request.json'
    if not path.exists():return False
    request=json.loads(path.read_text())
    if request.get('action')!='reader-accessibility':return False
    path.unlink()
    try:
        assert request['pid']==app.pid and app.poll() is None,'Accessibility target is not the owned child'
        import gi
        gi.require_version('Atspi','2.0')
        from gi.repository import Atspi, GLib
        Atspi.init();Atspi.set_timeout(1000,1000)
        deadline=time.monotonic()+4
        application=None
        while time.monotonic()<deadline and application is None:
            context=GLib.MainContext.default()
            for _ in range(256):
                if not context.pending():break
                context.iteration(False)
            desktop=Atspi.get_desktop(0)
            application=next((a for i in range(desktop.get_child_count()) if (a:=desktop.get_child_at_index(i)) and a.get_process_id()==app.pid),None)
            if application is None:time.sleep(.02)
        assert application is not None,'Owned accessibility application missing'
        pending=[(application,None)];nodes=[]
        while pending:
            assert len(nodes)<2000,'Private accessibility tree exceeds bound'
            node,parent=pending.pop()
            if node is None:continue
            try:
                node.clear_cache()
                item=dict(parent=parent,role=node.get_role_name(),name=node.get_name(),
                    description=node.get_description(),showing=node.get_state_set().contains(Atspi.StateType.SHOWING))
                text=node.get_text_iface()
                if text:
                    count=Atspi.Text.get_character_count(text)
                    item.update(characterCount=count,text=Atspi.Text.get_text(text,0,min(8192,count)),caret=Atspi.Text.get_caret_offset(text))
                    item['selection']=[dict(start=(r:=Atspi.Text.get_selection(text,i)).start_offset,end=r.end_offset) for i in range(min(4,Atspi.Text.get_n_selections(text)))]
                    if item['text'].startswith('Cross blocks'):
                        item['fullText']=Atspi.Text.get_text(text,0,-1)
                        item['probes']=[]
                        for offset in request.get('offsets',[]):
                            assert type(offset) is int and 0<=offset<count,'Bad private accessible text offset'
                            for label,granularity in [('character',Atspi.TextGranularity.CHAR),('line',Atspi.TextGranularity.LINE)]:
                                span=Atspi.Text.get_string_at_offset(text,offset,granularity)
                                item['probes'].append(dict(kind=label,offset=offset,start=span.start_offset,end=span.end_offset,text=span.content))
                identity=len(nodes);nodes.append(item)
                pending.extend((node.get_child_at_index(i),identity) for i in reversed(range(node.get_child_count())))
            except GLib.Error:continue
        response=dict(nodes=nodes)
        phase=request.get('phase','formula')
        assert phase in ['formula','selected','table'],'Unknown private accessibility phase'
        (root/f'flow-accessibility-{phase}.json').write_text(json.dumps(response,indent=2)+'\n')
    except Exception as error:response=dict(error=str(error))
    temporary=root/'clipboard-response.tmp';temporary.write_text(json.dumps(response));temporary.replace(root/'clipboard-response.json')
    return True

def run(native,gateway,root):
    digest=lambda:{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (root/'project').glob('*.md')}
    before=digest()
    owned=OwnedMutterInput();owned.enable_clipboard_reader()
    with (root/'app.log').open('w') as log:
        app=subprocess.Popen([native],cwd=root,env=dict(os.environ,G_DEBUG='fatal-criticals'),stdout=log,stderr=log)
        memory=ProcessMemory(app.pid,gateway,include_web_content=True)
        try:
            deadline=time.monotonic()+75
            while app.poll() is None:
                if not accessibility(root,app):owned.respond(root)
                if time.monotonic()>deadline:raise TimeoutError(('Native reader input timed out',root))
                time.sleep(.01)
        finally:
            if app.poll() is None:app.terminate();app.wait(timeout=10)
            measured=memory.close();owned.close()
    result=json.loads((root/'flow-input.json').read_text())
    assert app.returncode==0 and result['ok'],(root,result,(root/'app.log').read_text()[-2000:])
    assert all(result[k] for k in ['keyboardCrossBlockSelection','clickClearsSelection','doubleClickWord','tripleClickFormula','pointerCrossBlockSelection','foreignPrimaryRetained','changedSelectionRejected','replacedBufferRejected','retiredViewRejected','indexedTableOffscreenRows','accessibleFormula','accessibleOffsets','accessibleSelection','accessibleTable','selectedObjects','clearedObjects','retiredOutlinesCleared','replacedOutlinesCleared','inlineMathUnboxed'])
    assert len(result['selectionPixels'])==2 and all(p['unchangedSize'] for p in result['selectionPixels'])
    assert digest()==before,'Input acceptance changed the document'
    assert runtime.starts(root)==[],'Input acceptance sent a provider prompt'
    assert not any(s in (root/'app.log').read_text() for s in ['CRITICAL','panicked','Gtk-WARNING'])
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():runtime.until(lambda:not Path('/proc',(root/name).read_text().strip()).exists(),seconds=10)
    assert measured['webContentProcessesObserved']==0,'Native reader started web content'
    result.update(memory=measured,sourcesUnchanged=True,noProviderPrompts=True,ownedCleanup=True,artifacts=str(root))
    (root/'measurement.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root',type=Path)
    args=parser.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=[str(Path(p).resolve()) for p in [args.native,args.gateway]]
    if args.private_root:return run(native,gateway,args.private_root)
    base=REPO/'target/gnome-native-smoke';base.mkdir(exist_ok=True,parents=True)
    root=Path(tempfile.mkdtemp(prefix='flow-input-',dir=base));runtime.create_profile(root)
    code=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,error='Method not found')\n    elif method=='thread/read':")
    for name,body in [('fake-codex.py',code),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(body);path.chmod(0o700)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    (root/'project/input.md').write_text('# Cross blocks\n\nFirst **bold 中文😀** $x^2$ tail.\n\nSecond paragraph.\n\n```text\nplain code\n```\n')
    (root/'project/table.md').write_text('# Table drag\n\n| Name | Value |\n|---|---|\n'+''.join(f'| ROW-{i} | VALUE-{i} |\n' for i in range(70)))
    env={k:v for k,v in os.environ.items() if not k.startswith('STUDIO_GNOME_SMOKE_')}
    env.update(CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),
        CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),
        XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),
        READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',
        STUDIO_OWNED_MUTTER_INPUT='1',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_FLOW_INPUT='1',STUDIO_GNOME_CONTINUOUS_PREVIEW='1')
    env.pop('G_DEBUG',None)
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11',
            '--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),
            native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=110)

if __name__=='__main__':main()
