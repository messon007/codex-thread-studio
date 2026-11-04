#!/usr/bin/env python3
"""Installed GTK table/book/search/PTY language controls on private Wayland."""
import argparse, importlib.util, json, os, resource, shutil, subprocess, sys, tempfile, time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import OwnedMutterInput, isolated_environment
from native_accessibility import Client
from private_file_portal import FilePortal

def module(name,file):
    spec=importlib.util.spec_from_file_location(name,Path(__file__).with_name(file));value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value
archive=module('control_archive','smoke-gnome-router-archive.py');runtime=archive.runtime
QUERY='Needle {count} 中文😀 '
SQL='SELECT "Query" AS "Search", "Chapter {count}" AS "Wrap" FROM data WHERE "Query" = \'Needle \' ORDER BY "Chapter {count}" DESC'

def codex():
    return archive.codex().replace("    elif method=='thread/list':","    elif method=='thread/searchOccurrences':answer(identity,error='Method not found: thread/searchOccurrences')\n    elif method=='thread/list':",1)

def run(native,root):
    capture=os.environ.get('STUDIO_CONTROL_CAPTURE')=='1';epub_only=os.environ.get('STUDIO_CONTROL_EPUB_ONLY')=='1'
    assert capture or not any(k.startswith('STUDIO_GNOME_SMOKE') for k in os.environ)
    import gi
    gi.require_version('GioUnix','2.0')
    from gi.repository import Gio,GioUnix
    portal=FilePortal(root,[]);owned=OwnedMutterInput();log=(root/'app.log').open('w')
    child=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log);client=Client(child,iterate=portal.iterate)
    draft='Retain control-language draft 中文😀 '
    before={p.name:p.read_bytes() for p in [root/'codex-turns.json',root/'project/Search book {count}.csv',root/'project/Source {index}.epub']}
    def keys(values):
        for k in values:owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,True)))
        time.sleep(.04)
        for k in reversed(values):owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,False)))
        time.sleep(.04)
    def choose(combo,index):
        for _ in range(80):
            focus=client.find(lambda n:n.get_state_set().contains(client.api.StateType.FOCUSED))
            parent=combo.get_parent()
            if focus and (client.within(focus,combo) or parent and parent.get_role_name()=='list item' and client.within(focus,parent)):break
            keys([0xff09])
        else:raise RuntimeError(('Combo not keyboard reachable',combo.get_name()))
        keys([0x20]);keys([0xff50])
        for _ in range(index):keys([0xff54])
        keys([0xff0d])
    def locale(chinese):
        keys([0xffe3,ord(',')])
        combo=client.wait('language picker',lambda:client.find(lambda n:n.get_role_name()=='combo box' and n.get_name() in ['Language','语言'] and client.showing(n)))
        choose(combo,1 if chinese else 2);client.click('Apply language' if chinese else '应用语言')
        client.wait('saved locale',lambda:json.loads((root/'config/codex-thread-studio/settings.json').read_text()).get('language')==('zh-CN' if chinese else 'en-US'))
        title='设置' if chinese else 'Settings'
        dialog=client.wait('Settings',lambda:client.find(lambda n:n.get_role_name()=='dialog' and n.get_name()==title and client.showing(n)))
        client.activate(client.wait('Settings close',lambda:client.find(lambda n:n.get_role_name()=='button' and n.get_name() in ['Close','关闭'] and client.within(n,dialog) and client.showing(n))))
        client.wait('Settings closed',lambda:not client.find(lambda n:n.get_role_name()=='dialog' and n.get_name()==title and client.showing(n)))
    def present(name):return client.wait(name,lambda:client.text(name))
    def controls(names):
        for name in names:client.wait('control '+name,lambda name=name:client.find(lambda n:client.showing(n) and n.get_name()==name and 'button' in n.get_role_name()))
    def entry(name):return client.wait('entry '+name,lambda:client.find(lambda n:client.showing(n) and n.get_name()==name and n.get_editable_text_iface()))
    def combo(name):return client.wait('combo '+name,lambda:client.find(lambda n:client.showing(n) and n.get_role_name()=='combo box' and n.get_name()==name))
    def close_dialog(name):
        dialog=client.wait(name,lambda:client.find(lambda n:client.showing(n) and n.get_role_name()=='dialog' and n.get_name()==name))
        client.activate(client.wait('dialog close',lambda:client.find(lambda n:client.showing(n) and client.within(n,dialog) and n.get_role_name()=='button' and n.get_name() in ['Close','关闭'])))
        client.wait('closed '+name,lambda:not client.find(lambda n:client.showing(n) and n.get_role_name()=='dialog' and n.get_name()==name))
    try:
        present('Native target');present('Visible source answer')
        owned.focus_only_window()
        composer=client.wait('composer',lambda:client.find(lambda n:n.get_role_name()=='text' and n.get_name().startswith('Write a message.')));client.edit(composer,draft)
        prefix=root/'installed native 中文 % control'
        launcher=GioUnix.DesktopAppInfo.new_from_filename(str(prefix/'share/applications/io.github.messon007.codexthreadstudio.Gnome.desktop'));assert launcher
        def open_file(name):
            if capture:subprocess.run([native,str(root/'project'/name)],cwd=root,env=os.environ,check=True,timeout=15)
            else:assert launcher.launch([Gio.File.new_for_path(str(root/'project'/name))],None)
        if not epub_only:
            open_file('Search book {count}.csv');present('1–3 of 3 rows');client.click('SQL')
            editor=client.wait('SQL editor',lambda:client.find(lambda n:client.showing(n) and n.get_role_name()=='text' and n.get_editable_text_iface() and (client.contents(n) or '').startswith('SELECT')))
            client.edit(editor,SQL);client.click('Query');present('1–1 of 1 row')
            for chinese in [True,False]:
                locale(chinese)
                controls(['数据','SQL','图表','自动换行','紧凑','查询','关闭表格'] if chinese else ['Data','SQL','Chart','Wrap','Compact','Query','Close table'])
                present('1–1 / 共 1 行' if chinese else '1–1 of 1 row');present('Search book {count}.csv');present('章节 {index} 中文😀')
                assert client.contents(editor)==SQL,'Language changed the SQL buffer'
                assert client.find(lambda n:client.showing(n) and n.get_name()=='Search'),'Source column name was translated'
                selected=combo('自动分隔符' if chinese else 'Auto delimiter')
                # Open without choosing a different delimiter or reopening the table.
                for _ in range(80):
                    focus=client.find(lambda n:n.get_state_set().contains(client.api.StateType.FOCUSED))
                    if focus and client.within(focus,selected):break
                    keys([0xff09])
                else:raise RuntimeError('Delimiter is not keyboard reachable')
                keys([0x20]);present('制表符' if chinese else 'Tab');keys([0xff1b])
            client.click('Close table')
            keys([0xffe3,0xffe1,ord('f')]);search=entry('Find in conversation');client.edit(search,QUERY)
            present('2 matching loaded messages');choose(combo('All'),1);present('1 matching loaded messages')
            for chinese in [True,False]:
                locale(chinese);search=entry('在会话中查找' if chinese else 'Find in conversation');assert client.contents(search)==QUERY
                combo('用户' if chinese else 'User')
                present('1 条匹配的已加载消息' if chinese else '1 matching loaded messages')
                present(('用户\n' if chinese else 'User\n')+QUERY+'literal source request')
            client.click('Next match');present('1 of 1 loaded matches');client.click('Close conversation search')
            client.click('Terminal');pidfile=root/'terminal.pid';client.wait('private PTY',pidfile.exists);pid=pidfile.read_text()
            present('private-shell · '+str(root/'project'))
            command="printf 'Needle {count} terminal\\n' > terminal-literal.txt"
            for ch in command:
                owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(ord(ch),True)));owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(ord(ch),False)))
            keys([0xff0d]);canary=root/'project/terminal-literal.txt';client.wait('normal PTY input',canary.exists);assert canary.read_text()=='Needle {count} terminal\n'
            for chinese in [True,False]:
                locale(chinese)
                controls(['复制所选内容 (Ctrl+Shift+C)','粘贴 (Ctrl+Shift+V)','重新启动终端','关闭终端'] if chinese else ['Copy selection (Ctrl+Shift+C)','Paste (Ctrl+Shift+V)','Restart terminal','Close terminal'])
                assert pidfile.read_text()==pid and Path('/proc',pid.strip()).exists(),'Language restarted the terminal'
                assert canary.read_text()=='Needle {count} terminal\n';present(str(root/'project'))
            client.click('Close terminal');client.wait('PTY reaped',lambda:not Path('/proc',pid.strip()).exists())
        open_file('Source {index}.epub');present('Chapter 1');section_number=entry('Reading section number')
        # GTK 4.22 does not expose Component.grab_focus for this entry. Use
        # a real pointer click only after proving the owned window fills the
        # private monitor, making WINDOW coordinates unambiguous on Wayland.
        keys([0xffe9,0xffc7])  # Alt+F10: maximize the sole owned window.
        def maximized():
            frame=client.find(lambda n:n.get_role_name()=='frame' and client.showing(n))
            if frame is None:return False
            box=client.api.Component.get_extents(frame.get_component_iface(),client.api.CoordType.WINDOW)
            return box.width==1600 and box.height==1000
        client.wait('owned maximized allocation',maximized)
        time.sleep(.4)  # Let the compositor finish its maximize transition.
        if capture:
            (root/'native-capture-request').write_text('epub-section')
            client.wait('section capture',lambda:(root/'epub-section.png').exists())
        box=client.api.Component.get_extents(section_number.get_component_iface(),client.api.CoordType.WINDOW)
        (root/'section-input.json').write_text(json.dumps(dict(x=box.x,y=box.y,width=box.width,height=box.height)))
        assert box.width>0 and box.height>0
        owned.pointer(box.x+box.width/2,box.y+box.height/2)
        client.wait('actual section input focus',lambda:client.find(lambda n:n.get_state_set().contains(client.api.StateType.FOCUSED) and client.within(n,section_number)))
        keys([0xffe3,ord('a')]);keys([ord('2')])
        (root/'section-focus.json').write_text(json.dumps([dict(name=n.get_name(),role=n.get_role_name()) for n in client.tree() if n.get_state_set().contains(client.api.StateType.FOCUSED)]))
        if capture:
            (root/'native-capture-request').write_text('epub-section-after')
            client.wait('typed section capture',lambda:(root/'epub-section-after.png').exists())
        client.wait('typed section number',lambda:client.contents(section_number)=='2')
        # A real viewport resize causes the book's asynchronous layout/control
        # updates. An unsubmitted section number must survive those updates.
        keys([0xffe9,0xffc7])
        client.wait('restored owned allocation',lambda:not maximized())
        time.sleep(.6)
        assert client.contents(section_number)=='2','EPUB relayout overwrote the section number being edited'
        keys([0xff0d]);present('Chapter 2')
        position=entry('Reading section number');assert client.contents(position)=='2'
        client.click('Reading settings');choose(combo('Scroll'),1);client.click('Reading settings')
        client.wait('reading options closed',lambda:not client.find(lambda n:client.showing(n) and n.get_role_name()=='combo box' and n.get_name()=='Pages'))
        locale(True);controls(['打开 EPUB','重新加载 EPUB','关闭 EPUB','上一页或上一章','下一页或下一章']);present('Chapter 2')
        position=entry('阅读章节编号');assert client.contents(position)=='2'
        client.click('阅读设置');combo('分页');client.click('阅读设置')
        client.wait('Chinese reading options closed',lambda:not client.find(lambda n:client.showing(n) and n.get_role_name()=='combo box' and n.get_name()=='分页'))
        client.click('搜索当前章节 (Ctrl+F)');section=entry('搜索当前章节');client.edit(section,'Needle ');client.click('全书…')
        present('搜索全书');bookquery=entry('在所有章节中查找原样文本');assert client.contents(bookquery)=='Needle '
        client.click('搜索');present('搜索完成 · 匹配 3 个章节 · 已搜索 3 / 3 个章节')
        row=present('Chapter 1 · 1 处匹配');present('Needle and native text selection.')
        # Book search is modal: normal window settings shortcuts are blocked.
        # Close it, change the reader language, then reopen and explicitly run
        # the retained query in the second locale.
        close_dialog('搜索全书');locale(False);client.click('Book…');present('Search book')
        assert client.contents(entry('Literal text across all sections'))=='Needle '
        client.click('Search');present('Chapter 1 · 1 match')
        present('Search complete · 3 matching sections · searched 3 / 3 sections');close_dialog('Search book')
        assert client.contents(entry('Reading section number'))=='2';client.click('Reading settings');combo('Pages');client.click('Reading settings');client.click('Close EPUB')
        assert client.contents(composer)==draft
        for name,raw in before.items():assert (root/('codex-turns.json' if name=='codex-turns.json' else 'project/'+name)).read_bytes()==raw,name
        assert not runtime.starts(root),'Control reading sent inference'
        assert not portal.calls and not portal.failure
        client.wait('returned conversation input',lambda:client.showing(composer))
        client.click('Close');child.wait(timeout=25);assert child.returncode==0
        for name in ['backend.pid','opencode.pid']:
            p=root/name
            if p.exists():runtime.until(lambda:not Path('/proc',p.read_text().strip()).exists(),seconds=10)
        assert not any(t in (root/'app.log').read_text() for t in ['CRITICAL','Gtk-WARNING','panicked'])
        result=dict(ok=True,installedPair=True,noApplicationHooks=not capture,passiveCapture=capture,complete=not epub_only,gatewayOverride=False,tableSqlAndDataLiteral=not epub_only,delimiterContext=not epub_only,conversationSearch=not epub_only,terminalInputAndPid=not epub_only,epubPositionAndFlow=True,epubInputSurvivesRelayout=True,bookSearchLanguages=True,draftAndSourcePreserved=True,noInference=True,artifacts=str(root))
        (root/'control-language-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
    except BaseException:
        (root/'failed-tree.json').write_text(json.dumps(client.describe(),ensure_ascii=False,indent=2));raise
    finally:
        if child.poll() is None:child.terminate();child.wait(timeout=15)
        portal.close();owned.close();log.close()

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('bundle',type=Path);p.add_argument('--private-root',type=Path);p.add_argument('--epub-only',action='store_true',help='diagnostic EPUB scope; not complete control acceptance');p.add_argument('--capture',action='store_true',help='passive screenshots; requires the debug build');a=p.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    if a.private_root:return run(str(a.bundle),a.private_root)
    base=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke');base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='control-language-',dir=base));archive.create(root)
    prefix=root/'installed native 中文 % control';subprocess.run(['bash',str(a.bundle/'install.sh'),str(a.bundle),str(prefix)],check=True,stdout=subprocess.DEVNULL)
    def turn(i,role,text):return dict(id='t'+str(i),status='completed',items=[dict(id='i'+str(i),type='userMessage',content=[dict(type='text',text=text)])] if role=='user' else [dict(id='i'+str(i),type='agentMessage',text=text)])
    (root/'codex-turns.json').write_text(json.dumps(dict(target=[turn(1,'user',QUERY+'literal source request'),turn(2,'assistant',QUERY+'literal answer'),turn(3,'assistant','Visible source answer')])) )
    (root/'project/Search book {count}.csv').write_text('Query,Chapter {count},Wrap\nNeedle ,章节 {index} 中文😀,Search book\nOther,Read only,Data\n')
    module('control_epub','smoke-gnome-epub.py').book(root/'project/Source {index}.epub','https://example.invalid')
    for name,value in [('fake-codex.py',codex()),('fake-opencode.py',archive.opencode())]:path=root/name;path.write_text(value);path.chmod(0o700)
    shell=root/'private-shell';shell.write_text('#!/bin/sh\nprintf "%s" "$$" > "'+str(root/'terminal.pid')+'"\nexec /bin/sh\n');shell.chmod(0o700)
    settings=root/'config/codex-thread-studio/settings.json';raw=json.loads(settings.read_text());raw['language']='en-US';settings.write_text(json.dumps(raw));(settings.parent/'gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target',windowWidth=1280,windowHeight=900)))
    env=dict(os.environ,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(settings.parent/'backends.json'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_DIRS=str(prefix/'share')+':/usr/local/share:/usr/share',READER_CACHE_DIR=str(root/'reader-cache'),SHELL=str(shell),ENV='',BASH_ENV='',GDK_BACKEND='wayland',GTK_A11Y='atspi',GTK_USE_PORTAL='1',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE') or key=='CODEX_THREAD_STUDIO_GATEWAY_BIN':env.pop(key)
    env['STUDIO_CONTROL_EPUB_ONLY']='1' if a.epub_only else '0'
    env['STUDIO_CONTROL_CAPTURE']='1' if a.capture else '0'
    if a.capture:env.update(STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_CAPTURE_ONLY='1',STUDIO_GNOME_SMOKE_APPLICATION_ID='io.github.messon007.codexthreadstudio.Gnome.SmokeControlLanguage')
    with isolated_environment(env,headless=True) as private:subprocess.run(['dbus-run-session','--','mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),str(prefix/'bin/codex-thread-studio-gnome'),'--private-root',str(root)],cwd=root,env=private,check=True,timeout=300)
if __name__=='__main__':main()
