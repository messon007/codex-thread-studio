#!/usr/bin/env python3
"""Native chat comments with private fake providers, persistence and no model turns."""
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

sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory

def load(name,file):
    spec=importlib.util.spec_from_file_location(name,Path(__file__).with_name(file))
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module
runtime=load('chat_comment_runtime','smoke-gnome-router-runtime.py')
native_fixture=load('chat_comment_native','smoke-gnome-native.py')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root',type=Path)
    args=parser.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=(str(Path(p).resolve()) for p in [args.native,args.gateway])
    if args.private_root:
        root=args.private_root;results=[]
        for phase in ['capture','restore']:
            env=dict(os.environ,STUDIO_GNOME_SMOKE_CHAT_COMMENTS=phase)
            # Each launch starts at the intended owning conversation, while the
            # ordinary app still saves its selected sibling on first close.
            (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='native-smoke-thread')))
            with (root/(phase+'.log')).open('w') as log:
                child=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log)
                observer=ProcessMemory(child.pid,gateway,include_web_content=True)
                try:child.wait(timeout=130)
                except subprocess.TimeoutExpired:child.terminate();child.wait(timeout=10);raise
                finally:memory=observer.close()
            path=root/'chat-comments-result.json';result=json.loads(path.read_text()) if path.exists() else {}
            assert child.returncode==0 and result.get('ok'),(root,phase,child.returncode,result,(root/(phase+'.log')).read_text()[-2000:])
            with sqlite3.connect('file:'+str(root/'config/codex-thread-studio/studio.sqlite3')+'?mode=ro',uri=True) as db:
                records=json.loads(db.execute('SELECT drafts_json FROM session_annotation_drafts WHERE session_key=?',('codex:native-smoke-thread',)).fetchone()[0])
                sibling=db.execute('SELECT drafts_json FROM session_annotation_drafts WHERE session_key=?',('codex:native-cache-thread',)).fetchone()
            assert records==result['records'] and len(records)==5,(root,records,result)
            assert sibling is None,'Stale action created a sibling comment library'
            calls=[json.loads(line) for line in (root/'requests.jsonl').read_text().splitlines()]
            assert not [r for r in calls if r.get('method')=='turn/start'],'Comment capture/insert sent a model prompt'
            assert not any(word in (root/(phase+'.log')).read_text() for word in ['CRITICAL','panicked at','Gtk-WARNING'])
            assert memory['webContentProcessesObserved']==0,'Chat comments started a web-content engine'
            for name in ['backend.pid','opencode.pid']:
                path=root/name
                if path.exists():runtime.until(lambda:not Path('/proc',path.read_text().strip()).exists(),seconds=10)
            results.append(dict(phase=phase,result=result,memory=memory))
        # Test-only browser compatibility check; the native app has no Node dependency.
        repo=Path(__file__).resolve().parents[1]
        script='import assert from "node:assert/strict";import {readFileSync} from "node:fs";'
        script+='import {CommentSourceRegistry} from '+json.dumps((repo/'ui/comment-core.mjs').as_uri())+';'
        script+='import {createChatCommentProvider} from '+json.dumps((repo/'ui/comment-source-providers.mjs').as_uri())+';'
        script+='const registry=new CommentSourceRegistry().register(createChatCommentProvider());for(const record of JSON.parse(readFileSync(0,"utf8")))assert.deepEqual(registry.normalizeSource(record.source),record.source);'
        subprocess.run(['node','--input-type=module','--eval',script],input=json.dumps(records).encode(),check=True,timeout=15)
        print(json.dumps(dict(ok=True,phases=results,noModelPrompts=True,sharedSqlite=True,sourceOwnership=True,legacyNormalization=True,artifacts=str(root))))
        return
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='chat-comments-',dir=base));runtime.create_profile(root)
    body='# Chat comments\n\nLiteral Save **bold** 中文😀\n\n```rust\nfn literal() { println!("中文😀"); }\n```\n\n| Name | Value |\n|---|---|\n'+''.join(f'| Cell {i} 中文😀 | **Value {i}** |\n' for i in range(80))
    body+='\n```text\nlarge-chat-comment-boundary '+('x'*17000)+'\n```\n'
    (root/'chat-body.md').write_text(body)
    injection='''
_chat_original_thread=thread
def thread(identity='native-smoke-thread'):
    value=_chat_original_thread(identity)
    if identity in TITLES:value['turns'][0]['items'][1]['text']=Path(__file__).with_name('chat-body.md').read_text()
    return value
'''
    backend=native_fixture.FAKE_CODEX.replace('for raw in sys.stdin:',injection+'\nfor raw in sys.stdin:',1)
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root))
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=280)

if __name__=='__main__':main()
