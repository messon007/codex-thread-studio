#!/usr/bin/env python3
"""Actual installed Comments controls in English/Chinese; private providers only."""
import argparse, hashlib, importlib.util, json, os, resource, shutil, sqlite3, subprocess, sys, tempfile, time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from native_accessibility import Client
from private_wayland import OwnedMutterInput, isolated_environment
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('comments_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
QUOTE='Saved comments <b>Source</b> 中文😀 {value}'
NOTE='Unsaved comments <b>Comments</b> 中文😀 {value}\n'
INSTRUCTIONS='Additional instructions / Send comments 中文😀'

def seed(bundle,root,locale):
    runtime.fixture.BACKEND,runtime.fixture.OPENCODE_BACKEND=runtime.CODEX,runtime.OPENCODE
    api=runtime.fixture.Gateway(bundle/'codex-thread-studio-gateway',root,'codex')
    try:
        preferences=api.preferences();preferences['language']=locale;api.settings.write_text(json.dumps(preferences))
        status,snapshot=api.api('/studio/session-state/annotations?sessionKey=codex:target');assert status==200
        path=root/'project/Source.md'
        source=dict(provider='document',version=1,anchor=dict(root=str(path.parent),filePath=str(path),startLine=3,endLine=3,displayKind='source',baseHash=hashlib.sha256(path.read_bytes()).hexdigest(),future=dict(literal='Saved comments')))
        record=dict(id='installed-comment',excerpt=QUOTE,note='Saved comments',source=source,createdAt='2026-10-26T00:00:00Z')
        status,result=api.api('/studio/session-state/annotations',dict(sessionKey='codex:target',drafts=[record],additional='Original instructions',expectedRevision=snapshot['revision']),method='PUT')
        assert status==200,(status,result)
        return record
    finally:api.close()

def stored(root):
    with sqlite3.connect(f'file:{root/"config/codex-thread-studio/studio.sqlite3"}?mode=ro',uri=True) as database:
        row=database.execute('SELECT drafts_json FROM session_annotation_drafts WHERE session_key=?',('codex:target',)).fetchone()
        additional=database.execute('SELECT content FROM session_annotation_additional WHERE session_key=?',('codex:target',)).fetchone()
        return json.loads(row[0]) if row else [],additional[0] if additional else ''

def phase(bundle,root,locale,owned):
    record=seed(bundle,root,locale)
    zh=locale=='zh-CN'
    def tr(en,cn):return cn if zh else en
    with (root/(locale+'.log')).open('w') as log:
        child=subprocess.Popen([str(bundle/'codex-thread-studio-gnome')],cwd=root,env=os.environ,stdout=log,stderr=log)
        memory=ProcessMemory(child.pid,bundle/'codex-thread-studio-gateway');desktop=Client(child)
        def keys(values):
            for value in values:owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(value,True)))
            time.sleep(.06)
            for value in reversed(values):owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(value,False)))
        def editor(name):return desktop.wait(name,lambda:desktop.find(lambda n:n.get_name()==name and desktop.showing(n) and n.get_editable_text_iface() is not None))
        def status(value):return desktop.find(lambda n:n.get_name()==value and n.get_role_name()=='label' and desktop.showing(n))
        def library():return desktop.find(lambda n:n.get_role_name()=='dialog' and n.get_name()==tr('Comments','批注') and desktop.showing(n))
        def close_library():
            dialog=desktop.wait('Comments library',library)
            button=desktop.wait('Comments Close',lambda:desktop.find(lambda n:n.get_name() in ['Close','关闭'] and 'button' in n.get_role_name() and desktop.within(n,dialog) and desktop.showing(n)))
            desktop.activate(button)
        try:
            desktop.wait('selected conversation',lambda:desktop.text('Native target'))
            owned.focus_only_window()
            desktop.click(tr('Main menu','主菜单'));desktop.click(tr('Comments…','批注…'))
            desktop.wait('saved comment',lambda:status(tr('Saved comments','已保存的批注')))
            assert desktop.contents(editor(tr('Comment text','批注内容')))=='Saved comments'
            assert desktop.text('1. '+QUOTE),'Comment row lost literal accessible name'
            before=stored(root)
            desktop.edit(editor(tr('Comment text','批注内容')),NOTE)
            desktop.edit(editor(tr('Additional instructions','补充说明')),INSTRUCTIONS)
            desktop.click(tr('Open source','打开来源'))
            desktop.wait('dirty source guard',lambda:status(tr('Save comments before opening their source','请先保存批注，再打开来源')))
            desktop.click(tr('Reload saved comments','重新加载已保存批注'))
            desktop.wait('reload confirmation',lambda:desktop.text(tr('Reload saved comments?','重新加载已保存的批注？')))
            desktop.click(tr('Cancel','取消'))
            assert desktop.contents(editor(tr('Comment text','批注内容')))==NOTE and stored(root)==before
            close_library()
            desktop.wait('dirty close confirmation',lambda:desktop.text(tr('Save comment changes?','保存批注更改？')))
            desktop.click(tr('Cancel','取消'))
            assert desktop.contents(editor(tr('Additional instructions','补充说明')))==INSTRUCTIONS and stored(root)==before
            desktop.click(tr('Save','保存'))
            desktop.wait('explicit saved note',lambda:stored(root)[0][0]['note']==NOTE)
            saved,additional=stored(root)
            assert len(saved)==1 and additional==INSTRUCTIONS and saved[0]['source']==record['source'] and saved[0]['excerpt']==QUOTE
            desktop.click(tr('Insert into message','插入消息'))
            desktop.wait('insert feedback',lambda:status(tr('Inserted into message. Comments are retained until you remove them.','已插入消息。批注会保留，直到你将其移除。')))
            composer=editor('Write a message. Ctrl+Enter sends; Enter starts a new line.')
            prompt=desktop.contents(composer)
            assert NOTE in prompt and QUOTE in prompt and INSTRUCTIONS in prompt and prompt.count('Please address every comment')==1
            assert stored(root)==(saved,additional),'Inserting comments consumed or rewrote records'
            desktop.click(tr('Open source','打开来源'))
            desktop.wait('native source opening',lambda:not library() and desktop.text('Source.md'))
            assert not runtime.starts(root),'Opening a comment source started inference'
            desktop.click(tr('Main menu','主菜单'));desktop.click(tr('Comments…','批注…'))
            desktop.wait('reopened comments',lambda:status(tr('Saved comments','已保存的批注')))
            desktop.click(tr('Remove comment','移除批注'));desktop.click(tr('Save','保存'))
            desktop.wait('explicit removal',lambda:stored(root)[0]==[])
            desktop.wait('empty instructions',lambda:desktop.text(tr('Select text in a document and choose Comment to add it here.','在文档中选择文字，然后选择“添加批注”以添加到此处。')))
            close_library();desktop.wait('closed Comments',lambda:not library())
            assert desktop.contents(composer)==prompt,'Comments navigation changed the composer draft'
            keys([0xffe9,0xffc1]);child.wait(timeout=20);assert child.returncode==0
        except BaseException:
            (root/(locale+'-accessibility.json')).write_text(json.dumps(desktop.describe(),ensure_ascii=False));raise
        finally:
            if child.poll() is None:child.terminate();child.wait(timeout=10)
            measured=memory.close()
    assert not any(token in (root/(locale+'.log')).read_text() for token in ['CRITICAL','panicked','Gtk-WARNING'])
    assert not runtime.starts(root)
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
    return dict(locale=locale,ok=True,dirtySourceGuard=True,cancelReload=True,cancelClose=True,explicitSave=True,literalAnchor=True,promptInsertion=True,retainedRecords=True,nativeSourceOpen=True,explicitRemoval=True,draftRetained=True,memory=measured)

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('bundle');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));bundle=Path(args.bundle).resolve()
    if args.private_root:
        root=Path(args.private_root);owned=OwnedMutterInput()
        try:results=[phase(bundle,root,locale,owned) for locale in ['en-US','zh-CN']]
        finally:owned.close()
        result=dict(ok=True,installedPair=True,applicationHooks=False,gatewayOverride=False,modelTurns=0,phases=results,artifacts=str(root))
        (root/'comments-language.json').write_text(json.dumps(result,indent=2));print(json.dumps(result));return
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='release-comments-',dir=base));runtime.create_profile(root)
    (root/'project/Source.md').write_text('# Native comments\n\n'+QUOTE+'\n')
    for name,value in [('fake-codex.py',runtime.CODEX),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(value);path.chmod(0o700)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    env=dict(os.environ,CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',GTK_A11Y='atspi')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE') or key=='CODEX_THREAD_STUDIO_GATEWAY_BIN':env.pop(key)
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),str(bundle),'--private-root',str(root)],cwd=root,env=private,check=True,timeout=240)

if __name__=='__main__':main()
