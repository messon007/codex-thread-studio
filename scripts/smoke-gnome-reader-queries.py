#!/usr/bin/env python3
"""Native reader query API acceptance using private source copies and profiles."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import time
from urllib.request import Request
from urllib.error import HTTPError

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('query_runtime', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def call(gateway, path, body=None, document=None, expected=200, method=None, authorized=True, origin=None):
    headers = {'Content-Type':'application/json', 'Origin': origin or gateway.origin}
    if authorized:
        headers['Authorization'] = 'Bearer '+gateway.token
    if document:
        headers['x-reader-document'] = document
    request = Request(gateway.origin+path, data=json.dumps(body).encode() if body is not None else None, headers=headers, method=method)
    try:
        response = gateway.client.open(request, timeout=30)
    except HTTPError as error:
        response = error
    with response:
        data = response.read(256*1024+1)
        assert len(data) <= 256*1024
        assert response.status == expected, (path, response.status, expected, data[:2000])
        return json.loads(data) if data else None


def opened(gateway, source, readonly=False):
    result = call(gateway, '/studio/native/markdown/open-readonly' if readonly else '/studio/native/markdown/open', {'root':str(source.parent), 'path':str(source)})
    reader = result['readerId']
    until = time.monotonic()+60
    while time.monotonic() < until:
        result = call(gateway, '/studio/native/markdown/'+reader+'/document')
        if result['state'] == 'ready':
            return reader, result['documentId'], result['index']['revision']
        assert result['state'] != 'error', result
        time.sleep(.05)
    raise RuntimeError('Private query document indexing timed out')


def api(gateway_binary, root):
    runtime.create_profile(root)
    project = root/'project'
    rows = [dict(index=i, name='Name %04d 中文😀 '%i+'context '*14, active=i%2==0, value=i) for i in range(1200)]
    source = project/'query source 中文.ndjson'
    source.write_text(''.join(json.dumps(row,ensure_ascii=False)+'\n' for row in rows))
    array = project/'array.json'
    array.write_text(json.dumps(rows[:3],ensure_ascii=False))
    originals = {path:path.read_bytes() for path in (source,array)}
    runtime.fixture.BACKEND = runtime.CODEX
    runtime.fixture.OPENCODE_BACKEND = runtime.OPENCODE
    gateway = runtime.fixture.Gateway(Path(gateway_binary).resolve(),root)
    readers = []
    try:
        reader, document, revision = opened(gateway,source,readonly=True)
        readers.append(reader)
        path = '/studio/native/markdown/'+reader+'/jq-query'
        payload = dict(revision=revision,filter='select(.active) | .name')
        call(gateway,path,payload,document,expected=401,authorized=False)
        call(gateway,path,payload,document,expected=403,origin='https://unowned.invalid')
        call(gateway,path,document=document,expected=405)
        call(gateway,path,payload,document,expected=405,method='PUT')
        call(gateway,path,{**payload,'revision':'old'},document,expected=409)
        first = call(gateway,path,payload,document)
        assert first['kind']=='jqQuery' and first['revision']==revision and first['page']==0
        assert first['pages']>1 and first['totalValues']==600
        text = first['text']
        for number in range(1,first['pages']):
            page = call(gateway,path,dict(revision=revision,id=first['id'],page=number),document)
            assert page['id']==first['id'] and page['page']==number and len(page['text'].encode())<=48*1024
            text += page['text']
        assert [json.loads(line) for line in text.splitlines() if line.strip()] == [row['name'] for row in rows if row['active']]
        assert call(gateway,path,dict(revision=revision,id=first['id'],close=True),document)['closed']
        call(gateway,path,dict(revision=revision,id=first['id'],page=0),document,expected=400)
        summed = call(gateway,path,dict(revision=revision,filter='map(.value) | add',slurp=True),document)
        assert json.loads(summed['text']) == sum(row['value'] for row in rows)
        call(gateway,path,dict(revision=revision,filter='env'),document,expected=400)
        call(gateway,path,dict(revision=revision,filter='.'*16385),document,expected=400)
        with ThreadPoolExecutor(max_workers=1) as pool:
            slow = pool.submit(call,gateway,path,dict(revision=revision,filter='def loop: loop; loop'),document,400)
            # A separate source read must finish while the bounded evaluator is
            # still active; an infinite filter must not monopolize the service.
            time.sleep(.15)
            started = time.monotonic()
            call(gateway,'/studio/native/markdown/'+reader+'/source?line=1',document=document)
            responsiveness = time.monotonic()-started
            assert responsiveness<3, responsiveness
            assert not slow.done(), 'Infinite-filter fixture did not remain active during the source read'
            slow.result(timeout=12)
        second, second_document, second_revision = opened(gateway,array)
        readers.append(second)
        answer = call(gateway,'/studio/native/markdown/'+second+'/jq-query',dict(revision=second_revision,filter='map(.value) | add'),second_document)
        assert json.loads(answer['text'])==3
        call(gateway,path,payload,second_document,expected=409)
        assert all(path.read_bytes()==data for path,data in originals.items())
        assert not runtime.starts(root)
        result = dict(ok=True,readOnlyDocument=True,jqFilter=True,slurp=True,pagedUnicodeResults=True,queryClose=True,sourceRevisionChecks=True,authorization=True,originChecks=True,methodAllowlist=True,environmentDisabled=True,infiniteFilterBounded=True,sourceResponsiveSeconds=responsiveness,originalSourcesUnchanged=True,noInference=True,artifacts=str(root))
        (root/'query-api-result.json').write_text(json.dumps(result,indent=2)+'\n')
        print(json.dumps(result))
    finally:
        for reader in readers:
            call(gateway,'/studio/markdown/close',dict(id=reader))
        gateway.close()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('gateway')
    args=parser.parse_args()
    artifacts=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke')
    artifacts.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='reader-query-api-',dir=artifacts))
    api(args.gateway,root)


if __name__=='__main__':
    main()
