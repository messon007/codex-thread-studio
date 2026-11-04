#!/usr/bin/env python3
"""Installed Favorites capture: full immutable source and historical-page scope."""
import argparse, ast, importlib.util, json, os, resource, shutil, sqlite3, subprocess, sys, tempfile, time
from pathlib import Path

sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import OwnedMutterInput, isolated_environment
from native_accessibility import Client
spec=importlib.util.spec_from_file_location('favorite_history',Path(__file__).with_name('smoke-gnome-turn-navigation-scope.py'))
history=importlib.util.module_from_spec(spec);spec.loader.exec_module(history)

def response(index):
    if index==13:return 'Literal data must remain a complete message.\n… Output truncated in the native view.'
    if index not in (4,5,14):return 'Answer '+str(index)
    size=7100 if index!=5 else 10500
    return '# Complete favorite '+str(index)+'\r\n\r\n'+'Source 中文😀 context\r\n'*size+'\0Exact final bytes '+str(index)

def question(index):
    return history.PROMPT(index)+(' Question context 中文😀 '*12000 if index==16 else '')

def run(native,root,backend,language):
    assert not any(k.startswith('STUDIO_GNOME_SMOKE') for k in os.environ)
    catalog=json.loads((Path(__file__).resolve().parents[1]/'native/gnome/vendor/i18n/zh-CN.json').read_text()) if language=='zh-CN' else {}
    tr=lambda value:catalog.get(value,value)
    owned=OwnedMutterInput();owned.enable_clipboard_reader()
    def keys(values):
        for key in values:owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(key,True)))
        time.sleep(.04)
        for key in reversed(values):owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(key,False)))
        time.sleep(.04)
    log=(root/'app.log').open('w');child=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log);client=Client(child)
    originals={name:(root/name).read_bytes() for name in ['codex-turns.json','oc-turns.json']}
    draft='Favorite source capture keeps draft 中文😀 '
    def rows():
        with sqlite3.connect('file:'+str(root/'config/codex-thread-studio/studio.sqlite3')+'?mode=ro',uri=True) as db:
            db.row_factory=sqlite3.Row
            return [dict(row) for row in db.execute('SELECT scope,backend,thread_id,turn_id,item_id,title,content,question,note FROM favorites ORDER BY title')]
    def library():return client.find(lambda n:n.get_role_name()=='dialog' and n.get_name()==tr('Favorites') and client.showing(n))
    def choose(ordinal,index):
        client.click(tr('Turn navigation'));client.wait('turn chooser',lambda:client.text(tr('Turns in this page')))
        label=tr('User input {index}').replace('{index}',str(ordinal))+': '+history.PROMPT(index)
        client.click(label);client.wait('turn chooser closed',lambda:not client.text(tr('Turns in this page')))
    def favorite_button(index):
        prompt=client.find(lambda n:n.get_name()==(response(index) if index==16 else history.PROMPT(index)) and client.showing(n))
        if not prompt:return None
        viewport=prompt.get_parent()
        while viewport and viewport.get_role_name()!='scroll pane':viewport=viewport.get_parent()
        if not viewport:return None
        bounds=client.api.Component.get_extents(viewport.get_component_iface(),client.api.CoordType.WINDOW)
        origin=client.api.Component.get_extents(prompt.get_component_iface(),client.api.CoordType.WINDOW)
        candidates=[]
        for node in client.tree():
            if node.get_name()!=tr('Save displayed response to favorites') or not client.showing(node) or not client.sensitive(node):continue
            box=client.api.Component.get_extents(node.get_component_iface(),client.api.CoordType.WINDOW)
            if box.width>0 and bounds.y<=box.y<bounds.y+bounds.height and (box.y<=origin.y if index==16 else box.y>=origin.y):candidates.append((box.y,node))
        return (max if index==16 else min)(candidates,key=lambda pair:pair[0])[1] if candidates else None
    def close_library():
        lib=client.wait('favorite library',library)
        close=client.wait('dialog close control',lambda:client.find(lambda n:n.get_name() in ('Close',tr('Close')) and n.get_role_name() in ('button','push button') and client.within(n,lib) and client.showing(n) and client.sensitive(n)))
        client.activate(close)
        client.wait('favorite library closed',lambda:library() is None)
    def history_page(page):
        return client.text(tr('History · page {page} · {count} items').replace('{page}',str(page)).split('{count}',1)[0])
    def settled(lib):
        client.wait('favorite write acknowledged',lambda:client.find(lambda n:n.get_name()==tr('Copy favorite') and client.within(n,lib) and client.showing(n) and client.sensitive(n)))
    def save(index,title,thread=None):
        client.activate(client.wait('response favorite control',lambda:favorite_button(index)))
        lib=client.wait('favorite draft',library)
        field=client.wait('favorite title',lambda:client.find(lambda n:n.get_role_name()=='text' and n.get_name()==tr('Favorite title') and client.within(n,lib) and client.showing(n)))
        client.wait('favorite source prepared',lambda:client.sensitive(field))
        if index==16:
            client.wait('optional question explanation',lambda:client.text(tr('The question could not be included. You can still save the response.')))
            check=client.wait('disabled optional question',lambda:client.find(lambda n:n.get_name()==tr('Also save this question') and n.get_role_name()=='check box' and client.within(n,lib)))
            assert not client.sensitive(check) and not check.get_state_set().contains(client.api.StateType.CHECKED)
        client.edit(field,title);client.click(tr('Save'),parent=lib)
        client.wait('favorite explicitly saved',lambda:any(row['title']==title for row in rows()))
        settled(lib)
        saved=next(row for row in rows() if row['title']==title)
        assert saved['content']==response(index),('Favorite lost original response bytes',index,len(saved['content'].encode()),len(response(index).encode()),root)
        assert saved['scope']=='message' and saved['note']=='',saved['scope']
        assert saved['backend']==backend and saved['thread_id']==(thread or ('target' if backend=='codex' else 'oc-target'))
        assert saved['turn_id']==('turn-' if backend=='codex' else 'user-')+str(index)
        assert saved['item_id']=='answer-'+str(index)+('' if backend=='codex' else '-part')
        assert saved['question']==('' if index==16 else question(index))
        return lib
    try:
        client.wait('original conversation ready',lambda:client.text('Answer 15'))
        owned.focus_only_window()
        composer=client.wait('composer',lambda:client.find(lambda n:n.get_role_name()=='text' and n.get_name()==tr('Write a message. Ctrl+Enter sends; Enter starts a new line.')))
        client.edit(composer,draft)
        choose(5,14);lib=save(14,'Live complete source')
        client.click(tr('Copy favorite'),parent=lib)
        client.wait('complete favorite clipboard',lambda:owned.clipboard_text()=='Question:\n'+history.PROMPT(14)+'\n\n'+response(14).replace('\0','␀'))
        before=rows();client.toggle(tr('Also save this question'),keys,parent=lib)
        assert rows()==before,'Changing inclusion saved before explicit Save'
        client.click(tr('Copy favorite'),parent=lib)
        client.wait('excluded question clipboard',lambda:owned.clipboard_text()==response(14).replace('\0','␀'))
        client.click(tr('Save'),parent=lib)
        client.wait('explicit question exclusion',lambda:next(row for row in rows() if row['title']=='Live complete source')['question']=='')
        settled(lib)
        assert rows()[0]['content']==response(14)
        close_library()
        choose(4,13);save(13,'Marker is literal data');close_library()
        keys([0xffe3,0xffe1,ord('h')]);client.wait('history page one',lambda:history_page(1))
        client.click(tr('Older'));client.wait('history page two',lambda:history_page(2))
        choose(1,4);save(4,'Historical complete source');close_library()
        choose(2,5);before=rows()
        client.activate(client.wait('oversized favorite control',lambda:favorite_button(5)))
        client.wait('oversized source visibly rejected',lambda:client.text('Complete source exceeds the shared Favorites limit.'))
        assert rows()==before,'Oversized response silently saved an excerpt'
        close_library();client.click(tr('Live'))
        client.wait('original draft preserved',lambda:client.contents(composer)==draft)
        def select(title):
            label=client.wait('session '+title,lambda:client.find(lambda n:n.get_role_name()=='label' and n.get_name().endswith(title) and client.showing(n)))
            row=label
            while row.get_role_name()!='list item':row=row.get_parent()
            parent=row.get_parent();selection=parent.get_selection_iface()
            assert selection and client.api.Selection.select_child(selection,row.get_index_in_parent())
        select('Native other' if backend=='codex' else 'Native oc-other')
        client.wait('optional-question source',lambda:client.text(response(16)))
        lib=save(16,'Response without oversized question','other' if backend=='codex' else 'oc-other')
        # The capture warning is visible before Save. The saved item truthfully
        # has no question rather than persisting the question's display excerpt.
        close_library();select('Native target' if backend=='codex' else 'Native oc-target')
        client.wait('draft retained after optional-question source',lambda:client.contents(composer)==draft)
        assert all((root/name).read_bytes()==value for name,value in originals.items())
        assert not history.runtime.starts(root),'Favorite capture started inference'
        owned.focus_only_window();keys([0xffe9,0xffc1]);child.wait(timeout=25);assert child.returncode==0
        assert not any(word in (root/'app.log').read_text() for word in ['CRITICAL','Gtk-WARNING','panicked'])
        result=dict(ok=True,backend=backend,language=language,installedPair=True,applicationHooks=False,fullSnapshot=True,exactUnicodeCrLfNul=True,historyPageFavorite=True,optionalQuestion=True,oversizedOptionalQuestion=True,independentClipboard=True,oversizedSourceRejected=True,literalMarkerPreserved=True,noImplicitSave=True,sourceAndDraftPreserved=True,noInference=True,artifacts=str(root))
        (root/'favorite-source-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
    except BaseException:
        (root/'failed-tree.json').write_text(json.dumps(client.describe(),ensure_ascii=False,indent=2));raise
    finally:
        if child.poll() is None:child.terminate();child.wait(timeout=15)
        owned.close();log.close()

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('bundle',type=Path);parser.add_argument('--backend',choices=['codex','opencode'],default='codex');parser.add_argument('--language',choices=['en-US','zh-CN'],default='en-US');parser.add_argument('--private-root',type=Path);args=parser.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    if args.private_root:return run(str(args.bundle),args.private_root,args.backend,args.language)
    base=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke');base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='favorite-source-'+args.backend+'-',dir=base));history.runtime.create_profile(root)
    prefix=root/'installed native 中文 % favorite'
    subprocess.run(['bash',str(args.bundle.resolve()/'install.sh'),str(args.bundle.resolve()),str(prefix)],check=True,stdout=subprocess.DEVNULL)
    settings=root/'config/codex-thread-studio/settings.json';raw=json.loads(settings.read_text());raw['language']=args.language;settings.write_text(json.dumps(raw))
    (settings.parent/'gnome.json').write_text(json.dumps(dict(selectedBackend=args.backend,selectedSession='target' if args.backend=='codex' else 'oc-target',windowWidth=1400,windowHeight=950)))
    def turn(i):return dict(id='turn-'+str(i),status='completed',items=[dict(id='user-'+str(i),type='userMessage',content=[dict(type='text',text=question(i))]),dict(id='answer-'+str(i),type='agentMessage',text=response(i))])
    turns=[turn(i) for i in range(1,16)];other=[turn(16)]
    (root/'codex-turns.json').write_text(json.dumps(dict(target=turns,other=other)))
    messages={}
    for target,source in [('oc-target',turns),('oc-other',other)]:
        rows=[]
        for row in source:
            user,answer=row['items'];uid=user['id'];aid=answer['id']
            rows.extend([dict(info=dict(id=uid,role='user',sessionID=target),parts=[dict(id=uid+'-part',messageID=uid,sessionID=target,type='text',text=user['content'][0]['text'])]),dict(info=dict(id=aid,role='assistant',sessionID=target,parentID=uid,time=dict(completed=123)),parts=[dict(id=aid+'-part',messageID=aid,sessionID=target,type='text',text=answer['text'])])])
        messages[target]=rows
    (root/'oc-turns.json').write_text(json.dumps(messages))
    for name,code in zip(['fake-codex.py','fake-opencode.py'],history.providers()):ast.parse(code);path=root/name;path.write_text(code);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(settings.parent/'backends.json'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',GTK_A11Y='atspi')
    env.pop('CODEX_THREAD_STUDIO_GATEWAY_BIN',None)
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE'):env.pop(key)
    with isolated_environment(env,headless=True) as private:subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),str(prefix/'bin/codex-thread-studio-gnome'),'--private-root',str(root),'--backend',args.backend,'--language',args.language],cwd=root,env=private,check=True,timeout=240)

if __name__=='__main__':main()
