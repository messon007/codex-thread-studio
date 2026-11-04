#!/usr/bin/env python3
"""Inline-image API and actual native history/viewer controls, with private fake providers."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import struct
import subprocess
import sys
import tempfile
import time
from urllib.request import Request
from urllib.error import HTTPError

sys.dont_write_bytecode = True
def module(name, filename):
    spec=importlib.util.spec_from_file_location(name,Path(__file__).with_name(filename))
    value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value
images=module('image_fixture','smoke-gnome-images.py')
native_fixture=module('native_fixture','smoke-gnome-native.py')
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--api-only',action='store_true');parser.add_argument('--large-source',action='store_true',help='exercise a PNG transport fixture with a 10 MiB source');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=[str(Path(p).resolve()) for p in [args.native,args.gateway]]
    artifacts=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';artifacts.mkdir(exist_ok=True,parents=True)
    root=Path(tempfile.mkdtemp(prefix='history-images-',dir=artifacts));images.runtime.create_profile(root)
    png=images.png(3000,1000);(root/'project/inline.png').write_bytes(png);(root/'project/local.png').write_bytes(images.png(120,60))
    images.runtime.fixture.BACKEND=images.runtime.CODEX;images.runtime.fixture.OPENCODE_BACKEND=images.runtime.OPENCODE
    api=images.runtime.fixture.Gateway(Path(gateway),root,'codex')
    def upload(body,authorized=True,origin=None):
        headers={'Content-Type':'application/octet-stream'}
        if authorized:headers['Authorization']='Bearer '+api.token
        if origin:headers['Origin']=origin
        try:
            with api.client.open(Request(api.origin+'/studio/native/image/inline',data=body,headers=headers),timeout=15) as response:return response.status,response.read()
        except HTTPError as error:return error.code,error.read()
    try:
        assert upload(png,False)[0]==401
        assert upload(png,origin='https://example.invalid')[0]==403
        status,pixels=upload(png);assert status==200 and pixels[:8]==b'STIMG001',status
        assert struct.unpack('<4I',pixels[8:24])==(3000,1000,2048,683)
        assert upload(png)[1]==pixels
        for body in [b'not an image',b'<html><script>alert(1)</script></html>']:
            assert upload(body)[0]==400
        status,svg_pixels=upload(b'<svg xmlns="http://www.w3.org/2000/svg" width="5" height="5"><rect width="5" height="5" fill="#008866"/></svg>')
        assert status==200 and struct.unpack('<2I',svg_pixels[8:16])==(5,5)
        assert svg_pixels[24:28]==bytes([0,136,102,255])
        assert upload(b'')[0]==413
        assert upload(b'x'*(10*1024*1024+1))[0]==413
        assert upload(images.png(32769,1))[0]==400
        assert not images.runtime.starts(root)
    finally:api.close()
    if args.api_only:print(json.dumps({'ok':True,'inlineImageApi':True,'artifacts':str(root)}));return
    source=png if not args.large_source else png+b'\0'*(10*1024*1024-len(png))
    (root/'project/history-source.png').write_bytes(source)
    url="('data:image/png;base64,'+__import__('base64').b64encode(Path(__file__).with_name('project').joinpath('history-source.png').read_bytes()).decode())"
    # Modify fake providers, never real profiles, so normal native history paths carry images.
    injection='''
_history_image_url=%s
_original_thread=thread
def thread(identity):
    value=_original_thread(identity)
    if identity=='native-smoke-thread':
        value['turns'][0]['items'][0]['content'].extend([
            {'type':'image','url':_history_image_url},
            {'type':'localImage','path':str(Path(__file__).with_name('project')/'local.png')},
            {'type':'image','url':'https://example.invalid/not-fetched.png'}])
    return value
''' % url
    backend=native_fixture.FAKE_CODEX.replace('for raw in sys.stdin:',injection+'\nfor raw in sys.stdin:',1)
    opencode=Path(__file__).with_name('test-support').joinpath('gnome-opencode.py').read_text()
    oc_injection="\n_history_image_url="+url+"\nMESSAGES[0]['parts'].extend([{'id':'inline','type':'file','mime':'image/png','filename':'history.png','url':_history_image_url},{'id':'local','type':'file','mime':'image/png','filename':'local.png','url':(DIRECTORY/'project/local.png').as_uri()}])\n"
    opencode=opencode.replace('SUBSCRIBERS = []',oc_injection+'\nSUBSCRIBERS = []',1)
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',opencode)]:
        path=root/name;path.write_text(source);path.chmod(0o700)
    environment=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_HISTORY_IMAGES='1')
    environment.pop('DISPLAY',None);environment.pop('XAUTHORITY',None)
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],env=environment,cwd=root,stdout=log,stderr=log)
        observer=ProcessMemory(child.pid,gateway)
        try:
            try:child.wait(timeout=65)
            except subprocess.TimeoutExpired:child.terminate();child.wait(timeout=10);raise AssertionError(f'History images timed out: {root}')
        finally:memory=observer.close()
    assert child.returncode==0,(root,child.returncode)
    result=json.loads((root/'history-images.json').read_text());assert result['ok'],(root,result)
    assert result.get('visibleThumbnails') is True,'The debug binary must verify source-specific thumbnails inside the visible viewport'
    log=(root/'app.log').read_text()
    assert not any(marker in log for marker in ['Gtk-CRITICAL','GLib-GObject-CRITICAL','panicked at']),'Native image fixture logged a critical error'
    assert all(json.loads(line).get('method')!='turn/start' for line in (root/'requests.jsonl').read_text().splitlines())
    for name in ['backend.pid','opencode.pid']:
        if not (root/name).exists():continue
        identity=(root/name).read_text().strip()
        for _ in range(100):
            if not Path('/proc',identity).exists():break
            time.sleep(.05)
        assert not Path('/proc',identity).exists(),f'Owned backend {identity} survived fixture'
    print(json.dumps({'ok':True,'inlineImageApi':True,'inlineSourceBytes':(root/'project/history-source.png').stat().st_size,'observedMemory':memory,'nativeControls':result,'backendCleanup':True,'artifacts':str(root)}))

if __name__=='__main__':main()
