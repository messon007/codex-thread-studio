#!/usr/bin/env python3
"""Exercise read-only startup discovery failures on a private Wayland desktop."""
import argparse,hashlib,importlib.util,json,os,resource,shutil,sqlite3,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('discovery_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def child(native,gateway,root):
    before=(root/'project/discovery.md').read_bytes()
    viewer_before={name:(root/'project'/name).read_bytes() for name in ['recovery.csv','recovery.xlsx','recovery.pdf','recovery.epub'] if (root/'project'/name).is_file()}
    with (root/'app.log').open('w') as log:
        app=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log)
        memory=ProcessMemory(app.pid,gateway)
        try:code=app.wait(timeout=115)
        except BaseException:
            app.terminate()
            try:app.wait(timeout=10)
            except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
            raise
        finally:measured=memory.close()
    result=json.loads((root/'discovery.json').read_text())
    assert code==0 and result['ok'],(root,result)
    assert not any(s in (root/'app.log').read_text() for s in ['CRITICAL','panicked','Gtk-WARNING']),root
    assert runtime.starts(root)==[], 'Discovery sent a provider prompt'
    trace=root/'discovery-requests.jsonl'
    traces=[json.loads(s) for s in trace.read_text().splitlines()] if trace.exists() else []
    assert all(r['method']=='GET' for r in traces),traces
    mode=os.environ['STUDIO_GNOME_SMOKE_DISCOVERY']
    if mode=='automatic':assert len([r for r in traces if r['status']==503])>=2 and traces[-1]['status']==200,traces
    if mode=='permanent':assert [r['status'] for r in traces]==[401,200] and result['explicitReconnect'] and result['documentLaneAlive'] and result['staleGenerationRejected'],traces
    launch_trace=root/'launch-attempts.jsonl'
    launches=[json.loads(s) for s in launch_trace.read_text().splitlines()] if launch_trace.exists() else []
    if mode.startswith('launch-'):
        assert len(launches)==(1 if mode=='launch-close' else 2),launches
        assert launches[0]['kind']==('exit' if mode=='launch-retry' else 'hold'),launches
        if mode=='launch-close':assert result['requestQueueFull'],result
        if mode!='launch-close':
            assert launches[1]['kind']=='ready' and result['draftRetained'] and result['staleGenerationRejected'],(launches,result)
            assert [r['status'] for r in traces]==[200],traces
        if mode!='launch-retry':
            eof=int((root/'launch-eof').read_text())
            if mode in ['launch-replace','launch-timeout']:assert eof<=launches[1]['at'],launches
            if mode=='launch-timeout':assert result['launchTimeoutRecovered'] and launches[1]['at']-launches[0]['at']>=14_000_000_000,(result,launches)
    if mode in ['close','launch-close']:
        closing=(root/'discovery-closing').stat().st_mtime_ns
        assert not any(r['at']>closing for r in traces),traces
        assert result.get('closeDuringBackoff') or result.get('closeDuringLaunch')
        assert not any(r['at']>closing for r in launches),launches
    if mode.startswith('runtime-'):
        assert result['ownedGatewayExit'] and result['admittedExportCompleted'] and result['admittedExportCloseGuard'] and result['editor']['staleSaveRejected'] and result['lateTerminalRejected'] and result['sessionSwitchGuarded'],result
        actual=(root/'project/discovery.md').read_bytes()
        if mode=='runtime-stop':assert actual==before
        elif mode=='runtime-conflict':
            assert result['replacementRecovered'] and result['recoveredEditor']['conflictProtected'],result
            assert actual=='# Changed by another application\n\nExternal change 中文😀\n'.encode()
        else:
            assert result['replacementRecovered'] and result['recoveredEditor']['explicitSave'],result
            editor=result['recoveredEditor']
            assert actual==before[:editor['start']]+editor['expectedText'].encode()+before[editor['end']:]
            assert editor['undoRedoRoundTrip'],editor
            editor['sourcePreservedOutsideWindow']=True
            del editor['expectedText']
            if mode=='runtime-paged':assert editor['start']>0 and editor['end']<len(before),editor
        if mode.startswith('runtime-viewers'):
            assert result['viewers']['backgroundFocusRetained'],result
            for kind in ['table','pdf','epub']:
                assert result['viewers'][kind]['conflictProtected']==(mode=='runtime-viewers-conflict'),result
                assert result['viewers'][kind]['staleReplyRejected'],result
                assert (root/(kind+'-recovered.png')).stat().st_size>1000
            epub=result['viewers']['epub']
            supplement=os.environ.get('STUDIO_GNOME_SMOKE_BOOK_LOCATION')=='supplement'
            assert epub['supplement']==supplement,epub
            if supplement:
                surface=epub['supplementSurface']
                if mode=='runtime-viewers-conflict':assert surface is None,epub
                else:
                    assert surface['visible'] and 'Supplement paragraph 70' in surface['text'],surface
                    assert surface['href'].endswith('/OPS/Text/Supplement%20notes.xhtml#s70'),surface
            search=result['viewers']['epub']['bookSearch']
            assert search['draftRetained'] and search['resultsRetained'] and search['offlineActionsBlocked'] and search['noAutomaticSearch'],search
            searches=[json.loads(line) for line in (root/'epub-search-requests.jsonl').read_text().splitlines()]
            variant=os.environ.get('STUDIO_GNOME_SMOKE_BOOK_SEARCH','completed')
            first=list(range(0,112,16)) if variant!='completed' else [0]
            tail=[112,128] if variant!='completed' else []
            initial=([0,16,32,48,64] if variant=='inflight' else first+tail)
            resumed=([64,80,96]+tail+first+tail if variant=='inflight' else first+tail if variant=='paged' else [])
            expected=[('Needle',False,start,False) for start in initial]
            if mode!='runtime-viewers-conflict':
                expected += [('Needle',False,start,True) for start in resumed]
                expected += [('Unsent literal search 中文😀 ',True,start,True) for start in first+tail]
                expected += [('Needle',False,start,True) for start in first+first]
                assert search['recoveredDialogNavigation'] and search['retainedDialogQuery'] and search['newDocumentQuery'] and search['closedCallbacksRetired'] and search['immediateReopenIsolated'],search
                if variant!='completed':
                    paging=search['pagedRecovery']
                    assert paging['firstPageRows']==112 and paging['lastPageRows']==28 and paging['noGapsOrDuplicates'] and paging['draftRetained'] and paging['partialPageCompleted']==(variant=='inflight'),paging
            assert len(searches)==len(expected),(searches,expected)
            old_id=searches[0]['id'];new_id=searches[-1]['id']
            if mode!='runtime-viewers-conflict':assert old_id!=new_id
            for request,(query,case,start,new) in zip(searches,expected):
                assert request['query']==query and request['caseSensitive']==case and request['start']==start and request['id']==(new_id if new else old_id),(request,query,case,start,new)
            if variant=='inflight':
                held=json.loads((root/'epub-search-held.json').read_text())
                assert held['response']['start']==64 and held['response']['total']==140 and len(held['response']['sections'])==16,held
                assert search['retiredInFlightPageRejected'],search
            assert (root/'epub-search-recovered.png').stat().st_size>1000
            if os.environ.get('STUDIO_GNOME_SMOKE_TABLE_RECOVERY')=='xlsx':
                table=result['viewers']['table']
                assert table['workbook'] and table['worksheetRetained'] and table['worksheetCallbacksRetired'],table
                queries=[json.loads(line) for line in (root/'table-query-requests.jsonl').read_text().splitlines()]
                expected_sheets=[0,1] if mode=='runtime-viewers-conflict' else [0,1,1,1]
                assert [q['sheet'] for q in queries]==expected_sheets,queries
                assert queries[0]['sql']=='SELECT n, label, note FROM data ORDER BY CAST(n AS INTEGER) ASC',queries
                assert all(q['sql']=='SELECT n, label, note FROM data ORDER BY CAST(n AS INTEGER) DESC' for q in queries[1:]),queries
                assert queries[0]['id']==queries[1]['id'] and not any('unsent' in q['sql'] for q in queries),queries
                if mode!='runtime-viewers-conflict':
                    assert table['perSheetDraftsRetained'] and queries[1]['id']!=queries[2]['id']==queries[3]['id'],(table,queries)
            if os.environ.get('STUDIO_GNOME_SMOKE_PDF_RECOVERY')=='encrypted':
                unlock=result['viewers']['pdf']['encryptedUnlock']
                assert all(unlock[k] for k in ['reentryRequired','cancelRetainsSnapshot','wrongPasswordRetried','retiredPromptsRejected','duplicateRecoveryGuarded','entriesCleared']),unlock
                opens=[json.loads(line) for line in (root/'pdf-open-requests.jsonl').read_text().splitlines()]
                assert [o['passwordProvided'] for o in opens]==[False,True,True,False,False,True,True],opens
                assert opens[0]['id']==opens[1]['id']==opens[2]['id'] and opens[2]['id']!=opens[3]['id'],opens
                assert (root/'pdf-recovery-unlock.png').stat().st_size>1000
                for name in ['settings.json','gnome.json','studio.sqlite3']:
                    path=root/'config/codex-thread-studio'/name
                    if path.exists():assert not any(secret.encode() in path.read_bytes() for secret in ['open-sesame','private-wrong-password','private-cancelled-password']),name
            suffixes={'recovery.xlsx':b'\nExternal workbook edit\n','recovery.csv':b'150,Changed,External edit\n','recovery.pdf':b'\n% External edit\n','recovery.epub':b'\nExternal edit\n'}
            for name,original in viewer_before.items():
                assert (root/'project'/name).read_bytes()==original+(suffixes[name] if mode=='runtime-viewers-conflict' else b''),name
        if mode!='runtime-stop':
            assert (root/'gateway-recovered.png').stat().st_size>1000
            assert [r['status'] for r in traces]==[200,200],traces
        assert (root/'gateway-stopped.png').stat().st_size>1000
        with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3') as db:
            queue=json.loads(db.execute("SELECT queue_json FROM session_message_queues WHERE session_key='codex:target'").fetchone()[0])
        assert len(queue)==1 and queue[0]['text']=='Never automatically send this queued message',queue
    for name in ([] if mode.startswith('runtime-') else ['discovery-waiting.png']+([] if mode in ['close','launch-close'] else ['discovery-recovered.png'])+(['discovery-document.png'] if mode=='permanent' else [])):assert (root/name).stat().st_size>1000,name
    pids=[int(p.read_text().strip()) for p in root.glob('*.pid')]
    history=root/'recovery-launch-pids.jsonl'
    if history.exists():
        for line in history.read_text().splitlines():pids.extend(json.loads(line).values())
    for pid in set(pids):
        for _ in range(100):
            if not Path('/proc',str(pid)).exists():break
            time.sleep(.05)
        assert not Path('/proc',str(pid)).exists(),pid
    viewer_sources={name:dict(before=hashlib.sha256(original).hexdigest(),after=hashlib.sha256((root/'project'/name).read_bytes()).hexdigest()) for name,original in viewer_before.items()}
    value=dict(ok=True,viewerSourceHashes=viewer_sources,sourceBefore=dict(bytes=len(before),sha256=hashlib.sha256(before).hexdigest()),sourceAfter=dict(bytes=(root/'project/discovery.md').stat().st_size,sha256=hashlib.sha256((root/'project/discovery.md').read_bytes()).hexdigest()),checks=result,requests=traces,launches=launches,noProviderPrompts=True,ownedCleanup=True,memory=measured,artifacts=str(root))
    (root/'measurement.json').write_text(json.dumps(value,indent=2)+'\n')
    print(json.dumps(value))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway');p.add_argument('--private-root',type=Path);p.add_argument('--mode',choices=['automatic','permanent','close','launch-retry','launch-close','launch-replace','launch-timeout','runtime-stop','runtime-recover','runtime-conflict','runtime-paged','runtime-viewers','runtime-viewers-conflict'],default='automatic');p.add_argument('--document',type=Path);p.add_argument('--book-location',choices=['chapter','supplement'],default='chapter');p.add_argument('--table-recovery',choices=['csv','xlsx'],default='csv');p.add_argument('--pdf-recovery',choices=['plain','encrypted'],default='plain');p.add_argument('--book-search',choices=['completed','paged','inflight'],default='completed',help='Result-page fixture for runtime-viewers modes (debug native only)');args=p.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return child(native,gateway,args.private_root)
    if (args.book_location!='chapter' or args.book_search!='completed' or args.pdf_recovery!='plain' or args.table_recovery!='csv') and not args.mode.startswith('runtime-viewers'):p.error('Viewer recovery options require runtime-viewers modes')
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(exist_ok=True,parents=True)
    root=Path(tempfile.mkdtemp(prefix='discovery-'+args.mode+('' if args.book_search=='completed' else '-'+args.book_search)+'-',dir=base));runtime.create_profile(root)
    settings=root/'config/codex-thread-studio/settings.json';raw=json.loads(settings.read_text());raw['language']='en-US';settings.write_text(json.dumps(raw))
    (settings.parent/'gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    (root/'codex-turns.json').write_text(json.dumps({'target':[{'id':'existing-turn','status':'completed','items':[{'id':'existing-item','type':'agentMessage','text':'Existing conversation history 中文😀'}]}]}))
    (root/'project/discovery.md').write_text('# Independent document lane\n\nReadable before backend discovery. 中文😀\n')
    if args.mode=='runtime-paged':
        if not args.document:p.error('--document is required for runtime-paged')
        shutil.copyfile(args.document,root/'project/discovery.md')
    if args.mode.startswith('runtime-viewers'):
        for name in ['pdf','epub']:
            spec=importlib.util.spec_from_file_location('viewer_'+name,Path(__file__).with_name('smoke-gnome-'+name+'.py'))
            module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
            if name=='pdf':module.document(root/'project/recovery.pdf',password='open-sesame' if args.pdf_recovery=='encrypted' else None)
            else:module.book(root/'project/recovery.epub','http://127.0.0.1:9')
        if args.book_search != 'completed':
            from recovery_epub import extend_search_book
            extend_search_book(root/'project/recovery.epub')
        if args.book_location == 'supplement':
            from recovery_epub import add_supplement
            add_supplement(root/'project/recovery.epub')
        if args.book_search == 'inflight':
            (root/'hold-epub-search').write_text('64')
        if args.table_recovery=='xlsx':
            from recovery_workbook import workbook
            workbook(root/'project/recovery.xlsx')
        else:
            (root/'project/recovery.csv').write_text('n,label,note\n'+''.join(f'{n},Row {n},Long note for native wrap and column widths {n} 中文😀\n' for n in range(150)))
    if args.mode.startswith('launch-'):
        (root/'launch-fault').write_text('exit' if args.mode=='launch-retry' else 'hold')
    elif not args.mode.startswith('runtime-'):(root/'discovery-fault').write_text('401' if args.mode=='permanent' else '503')
    code=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,error='Method not found')\n    elif method=='thread/read':")
    for name,body in [('fake-codex.py',code),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(body);path.chmod(0o700)
    proxy=root/'gateway-proxy.py';proxy.write_text('#!/usr/bin/python3\nimport sys\nsys.dont_write_bytecode=True\nsys.path.insert(0,'+repr(str(Path(__file__).with_name('test-support').resolve()))+')\n'+('from private_gateway_launch import fault\nfault()\n' if args.mode.startswith('launch-') else '')+'from private_ollama_proxy import main\nraise SystemExit(main('+repr(gateway)+',sys.argv[1:],faults=True))\n');proxy.chmod(0o700)
    env={k:v for k,v in os.environ.items() if not k.startswith('STUDIO_GNOME_SMOKE_')}
    env.update(CODEX_THREAD_STUDIO_GATEWAY_BIN=str(proxy),CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',G_DEBUG='fatal-criticals',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_DISCOVERY=args.mode,STUDIO_GNOME_SMOKE_BOOK_SEARCH=args.book_search,STUDIO_GNOME_SMOKE_BOOK_LOCATION=args.book_location,STUDIO_GNOME_SMOKE_PDF_RECOVERY=args.pdf_recovery,STUDIO_GNOME_SMOKE_TABLE_RECOVERY=args.table_recovery)
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--','/usr/bin/mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=140)
if __name__=='__main__':main()
