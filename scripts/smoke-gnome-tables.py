#!/usr/bin/env python3
"""Native table API and actual Wayland controls, with private files/fake providers."""
import argparse
import csv
import importlib.util
import io
import json
import os
import resource
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import zipfile

sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('table_fixture',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)


def workbook(path):
    parts={
        '[Content_Types].xml':'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>',
        '_rels/.rels':'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>',
        'xl/workbook.xml':'<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="First 中文" sheetId="1" r:id="rId1"/><sheet name="Second" sheetId="2" r:id="rId2"/></sheets></workbook>',
        'xl/_rels/workbook.xml.rels':'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>',
        'xl/worksheets/sheet1.xml':'<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:XFD1048576"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>name</t></is></c><c r="B1" t="inlineStr"><is><t>value</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>中文 &amp; 😀</t></is></c><c r="B2"><f>1+2</f><v>3</v></c></row><row r="1048576"><c r="XFD1048576"><v>1</v></c></row></sheetData></worksheet>',
        'xl/worksheets/sheet2.xml':'<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Second worksheet</t></is></c></row></sheetData></worksheet>',
    }
    with zipfile.ZipFile(path,'w',compression=zipfile.ZIP_DEFLATED) as archive:
        for name,text in parts.items():archive.writestr(name,text)


def api_checks(executable,root):
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    gateway=runtime.fixture.Gateway(Path(executable),root,'codex')
    def request(operation,**fields):return gateway.api('/studio/native/tables',{'operation':operation,**fields})
    try:
        assert gateway.api('/studio/native/tables',{'operation':'open','root':str(root/'project'),'path':'data.csv'},authorized=False)[0]==401
        status,opened=request('open',root=str(root/'project'),path='data.csv');assert status==200,opened
        identity=opened['id'];assert opened['delimiter']==';' and opened['page']['total']==131
        assert opened['page']['rows'][1][2]['clipped']
        assert request('cell',id=identity,sheet=0,row=1,column=2)[1]['text'].endswith('完整结束')
        status,comment=request('comment',id=identity,sheet=0,row=1,column=2)
        assert gateway.api('/studio/native/tables',{'operation':'comment','id':identity,'sheet':0,'row':1,'column':2},authorized=False)[0]==401
        assert status==200 and len(comment['text'])>2048 and comment['text'].endswith('完整结束'),comment
        assert comment['anchor']['documentHash']==opened['hash'] and comment['anchor']['range']=='C2' and comment['anchor']['sql'] is None and comment['anchor']['delimiter']==';',comment
        assert request('page',id=identity,sheet=0,offset=50)[1]['offset']==50
        status,result=request('query',id=identity,sheet=0,sql='SELECT name, CAST(score AS INTEGER) AS score FROM data WHERE CAST(score AS INTEGER)>127 ORDER BY score DESC')
        assert status==200 and result['rows'][0][0]['text']=='row-129',result
        query=result['queryId']
        comment=request('comment',id=identity,sheet=0,row=0,column=0,query=query)[1]
        assert comment['text']=='row-129' and comment['anchor']['range']=='A1' and comment['anchor']['header']=='name' and comment['anchor']['sql'].endswith('ORDER BY score DESC'),comment
        for sql in ['DELETE FROM data','PRAGMA table_info(data)',"ATTACH DATABASE '/tmp/unwanted.db' AS other",'SELECT * FROM data; SELECT * FROM data','SELECT char(0) FROM data']:
            assert request('query',id=identity,sheet=0,sql=sql)[0]==400,sql
        assert request('page',id=identity,sheet=0,offset=0,query=query)[0]==200
        assert request('page',id=identity,sheet=1,offset=0,query=query)[0]==400
        changed=request('query',id=identity,sheet=0,sql='SELECT COUNT(*) AS count FROM data')[1]
        assert changed['rows'][0][0]['text']=='130'
        assert request('page',id=identity,sheet=0,offset=0,query=query)[0]==409
        assert request('comment',id=identity,sheet=0,row=0,column=0,query=query)[0]==409
        assert request('format',sql='SELECT\n "name",\n "score"\nFROM data ORDER BY score',compact=True)[1]['sql']=='SELECT "name", "score"\nFROM data ORDER BY score'
        assert request('open',root=str(root/'project'),path='escape.csv')[0]==403
        large=root/'project/too-large.csv'
        with large.open('wb') as output:output.truncate(20*1024*1024+1)
        assert request('open',root=str(root/'project'),path=large.name)[0]==413
        request('close',id=identity)
        assert request('comment',id=identity,sheet=0,row=0,column=0)[0]==410
        assert request('page',id=identity,sheet=0,offset=0)[0]==410
        status,opened=request('open',root=str(root/'project'),path='book.xlsx')
        assert status==200 and len(opened['sheets'])==2 and opened['page']['rows'][1][1]['text']=='3',opened
        assert opened['sheets'][0]['truncated']
        cell=request('comment',id=opened['id'],sheet=0,row=1,column=0)[1]
        assert cell['text']=='中文 & 😀' and cell['anchor']['sheet']=='First 中文' and cell['anchor']['range']=='A2',cell
        assert request('page',id=opened['id'],sheet=1,offset=0)[1]['rows'][0][0]['text']=='Second worksheet'
        request('close',id=opened['id'])
        assert request('open',root=str(root/'project'),path='data.csv',delimiter='|')[0]==400
        (root/'project/large-comment.csv').write_text('value\n'+'x'*16385+'\n')
        status,large=request('open',root=str(root/'project'),path='large-comment.csv');assert status==200,large
        assert request('comment',id=large['id'],sheet=0,row=1,column=0)[0]==400
        query=request('query',id=large['id'],sheet=0,sql='SELECT value FROM data /*'+'x'*16384+'*/')[1]
        assert request('comment',id=large['id'],sheet=0,row=0,column=0,query=query['queryId'])[0]==400
        request('close',id=large['id'])
        status,tab=request('open',root=str(root/'project'),path='other.tsv');assert status==200,tab
        comment=request('comment',id=tab['id'],sheet=0,row=1,column=1)[1]
        assert comment['text']=='9' and comment['anchor']['delimiter']=='\t' and comment['anchor']['range']=='B2',comment
        request('close',id=tab['id'])
        assert not runtime.starts(root),'Table viewing triggered a model call'
    finally:gateway.close()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native',help='native GNOME executable')
    parser.add_argument('gateway',help='gateway-only executable')
    parser.add_argument('--api-only',action='store_true',help='check authenticated table operations without a compositor')
    args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=map(lambda value:str(Path(value).resolve()),[args.native,args.gateway])
    artifacts=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';artifacts.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='tables-ui-',dir=artifacts));runtime.create_profile(root)
    project=root/'project'
    text=io.StringIO(newline='');writer=csv.writer(text,delimiter=';');writer.writerow(['name','score','note'])
    for row in range(130):writer.writerow([f'row-{row}',str(row),('long value 中文 '+('notes, with; quotes " and Unicode 😀\n'*140)+'完整结束') if row==0 else 'long note context '+('word '*40)])
    (project/'data.csv').write_text('\ufeff'+text.getvalue());(project/'other.tsv').write_text('name\tvalue\nother\t9\n');(project/'escape.csv').symlink_to('/etc/passwd');workbook(project/'book.xlsx')
    with (project/'large.csv').open('w',newline='') as output:
        writer=csv.writer(output);writer.writerow([f'c{column}' for column in range(40)])
        for row in range(12000):writer.writerow([str(row*40+column) for column in range(40)])
    with (project/'wide.csv').open('w',newline='') as output:
        writer=csv.writer(output);writer.writerow([f'c{column}' for column in range(200)])
        for _ in range(9999):writer.writerow(['1']*200)
    api_checks(gateway,root)
    if args.api_only:
        print(json.dumps({'ok':True,'artifacts':str(root),'tableApi':True,'nativeControls':'not requested'}))
        return
    backend=runtime.CODEX.replace("assert params['excludeTurns'] is True","pass").replace("    elif method=='turn/start':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='turn/start':")
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(source);path.chmod(0o700)
    environment=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_TABLES='1')
    environment.pop('DISPLAY',None);environment.pop('XAUTHORITY',None)
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],env=environment,cwd=root,stdout=log,stderr=log)
        observer=ProcessMemory(child.pid,gateway)
        try:
            try:child.wait(timeout=65)
            except subprocess.TimeoutExpired:child.terminate();child.wait(timeout=10);raise AssertionError(f'Native table fixture timed out: {root}')
        finally:memory=observer.close()
    assert child.returncode==0,(root,child.returncode)
    result=json.loads((root/'tables-ui.json').read_text());assert result['ok'],(root,result)
    assert not runtime.starts(root),'Native table UI triggered a model call'
    for name in ['backend.pid','opencode.pid']:
        if not (root/name).exists():continue
        identity=(root/name).read_text().strip()
        for _ in range(100):
            if not Path('/proc',identity).exists():break
            time.sleep(.05)
        assert not Path('/proc',identity).exists(),f'Owned backend {identity} survived table fixture'
    print(json.dumps({'ok':True,'artifacts':str(root),'tableApi':True,'nativeControls':result,'backendCleanup':True,'largeSourceBytes':(project/'large.csv').stat().st_size,'observedMemory':memory}))


if __name__=='__main__':main()
