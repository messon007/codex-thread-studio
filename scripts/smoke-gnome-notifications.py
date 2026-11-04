#!/usr/bin/env python3
"""Native notification preference, private GIO transport and D-Bus reactivation.

Never sends test notifications to the user's desktop notification service.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('notice_fixture',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
from private_wayland import isolated_environment

def api_checks(executable,root):
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    service=runtime.fixture.Gateway(Path(executable),root,'codex');endpoint='/studio/native/notifications'
    def patch(rev,enabled,**extra):return service.api(endpoint,dict(expectedRevision=rev,enabled=enabled,**extra),method='PATCH')
    try:
        assert service.api(endpoint,authorized=False)[0]==401
        assert service.api(endpoint,{},method='PATCH',authorized=False)[0]==401
        for method in ['GET','PATCH']:
            before=service.settings.read_bytes()
            try:
                service.client.open(Request(service.origin+endpoint,data=b'{}' if method=='PATCH' else None,headers={'Authorization':'Bearer '+service.token,'Origin':'https://foreign.example','Content-Type':'application/json'},method=method))
                raise AssertionError('Foreign origin accessed notification settings')
            except HTTPError as error:assert error.code==403
            assert service.settings.read_bytes()==before
        code,base=service.api(endpoint);assert code==200 and base['enabled'] is False,base
        raw=service.preferences();raw['futureNotification']={'preserve':True};service.settings.write_text(json.dumps(raw))
        code,on=patch(base['revision'],True);assert code==200 and on['enabled'] is True,on
        assert service.api('/studio/preferences')[1]['desktopNotifications'] is True
        before=service.settings.read_bytes()
        assert patch(base['revision'],False)[0]==409 and service.settings.read_bytes()==before
        for enabled in [None,1,'true',{},[]]:assert patch(on['revision'],enabled)[0]==400
        assert patch(on['revision'],False,unknown=True)[0]==400 and service.settings.read_bytes()==before
        raw=service.preferences();raw['desktopNotifications']={'broken':True};service.settings.write_text(json.dumps(raw));before=service.settings.read_bytes()
        code,bad=service.api(endpoint);assert code==200 and bad['enabled'] is None and bad['validationError'],bad
        assert service.settings.read_bytes()==before
        assert patch(bad['revision'],False)[0]==200 and service.preferences()['futureNotification']['preserve']
        # IDs remain private shell state, rather than shared content preferences.
        prefs='/studio/native/preferences'
        assert service.api(prefs,{'notificationIds':['invalid']},method='PATCH')[0]==400
        ids=['studio-'+str(i).encode().hex() for i in range(32)]
        assert service.api(prefs,{'notificationIds':ids},method='PATCH')[0]==200
        assert service.api(prefs,{'notificationIds':ids+['studio-ff']},method='PATCH')[0]==400
        assert service.api(prefs,{'notificationIds':[]},method='PATCH')[0]==200
        assert not runtime.starts(root),'Notification settings triggered a model turn'
    finally:service.close()

XML='''<node><interface name="org.gtk.Notifications">
<method name="AddNotification"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/></method>
<method name="RemoveNotification"><arg type="s" direction="in"/><arg type="s" direction="in"/></method>
</interface></node>'''

def private_bus(native,gateway,root):
    from gi.repository import Gio,GLib
    connection=Gio.bus_get_sync(Gio.BusType.SESSION,None)
    connection.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','RequestName',GLib.Variant('(su)',('org.gtk.Notifications',0)),None,Gio.DBusCallFlags.NONE,5000,None)
    records=[];active={};errors=[];phase='emit';activated=False;second_started=False;service_pid=None;observer=None;memory=[]
    loop=GLib.MainLoop();appid=os.environ['STUDIO_GNOME_SMOKE_APPLICATION_ID'];object_path='/'+appid.replace('.','/')
    def simple(value):
        if isinstance(value,GLib.Variant):return simple(value.unpack())
        if isinstance(value,dict):return {k:simple(v) for k,v in value.items()}
        if isinstance(value,(tuple,list)):return [simple(v) for v in value]
        return value
    def called(_c,_sender,_path,_interface,method,parameters,invocation):
        values=parameters.unpack();app,identity=values[:2]
        if app!=appid:errors.append('Unexpected application used private notifier')
        if method=='AddNotification':
            props=simple(values[2]);records.append({'method':'add','id':identity,'properties':props});active[identity]=props
        else:records.append({'method':'remove','id':identity});active.pop(identity,None)
        (root/'notification-transport.json').write_text(json.dumps(records,indent=2))
        invocation.return_value(GLib.Variant('()',()))
    registration=connection.register_object('/org/gtk/Notifications',Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0],called,None,None)
    env=dict(os.environ,STUDIO_GNOME_SMOKE_NOTIFICATIONS='emit')
    log=(root/'app-emit.log').open('w');child=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log)
    observer=ProcessMemory(child.pid,gateway);deadline=time.monotonic()+100
    def owner():return connection.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','NameHasOwner',GLib.Variant('(s)',(appid,)),None,Gio.DBusCallFlags.NONE,5000,None).unpack()[0]
    def activate(identity):
        connection.call(appid,object_path,'org.freedesktop.Application','ActivateAction',GLib.Variant('(sava{sv})',('show-notification',[GLib.Variant('s',identity)],{})),None,Gio.DBusCallFlags.NONE,10000,None,
                        lambda c,r: finish_call(c,r))
    def finish_call(c,r):
        try:c.call_finish(r)
        except Exception as error:errors.append(str(error))
    def poll():
        nonlocal activated,phase,second_started,service_pid,observer
        try:
            if errors:raise AssertionError(errors)
            if time.monotonic()>deadline:raise AssertionError('Private notification test deadline exceeded')
            if phase=='emit':
                approval=next((v for v in reversed(records) if v.get('properties',{}).get('title')=='Agent needs your input' and v['id']=='studio-'+b'codex:target'.hex()),None)
                if not activated and approval and (root/'activate-notification.flag').exists():activate(approval['id']);activated=True
                if child.poll() is not None:
                    memory.append(observer.close());observer=None;log.close()
                    assert child.returncode==0,(root,child.returncode,(root/'app-emit.log').read_text()[-4000:])
                    assert (root/'notifications-emit.json').exists(),(root,'Native process exited without acceptance result',(root/'app-emit.log').read_text()[-4000:])
                    result=json.loads((root/'notifications-emit.json').read_text());assert result['ok'],result
                    assert all(result['router'].values()),result
                    additions=[v for v in records if v['method']=='add']
                    assert [v['properties']['title'] for v in additions]==['Work completed','Work completed','Work completed','Work failed','Agent needs your input','Agent needs your input','Work completed'],additions
                    assert all('Private' not in v['properties'].get('body','') for v in additions)
                    assert all(v['properties'].get('default-action')=='app.show-notification' for v in additions),additions
                    ids=json.loads((root/'config/codex-thread-studio/gnome.json').read_text())['notificationIds']
                    assert len(ids)==32 and additions[-1]['id'] in ids and additions[-1]['id'] in active,(ids,active)
                    # Let the D-Bus daemon start a new native process from the service file.
                    phase='restore';activate(additions[-1]['id']);second_started=True
            else:
                if owner() and service_pid is None:
                    service_pid=connection.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','GetConnectionUnixProcessID',GLib.Variant('(s)',(appid,)),None,Gio.DBusCallFlags.NONE,5000,None).unpack()[0]
                    assert service_pid!=child.pid
                    observer=ProcessMemory(service_pid,gateway)
                if (root/'notifications-restore.json').exists() and not owner():
                    result=json.loads((root/'notifications-restore.json').read_text());assert result['ok'],result
                    if observer:memory.append(observer.close());observer=None
                    assert not active,active
                    assert json.loads((root/'config/codex-thread-studio/gnome.json').read_text())['notificationIds']==[]
                    assert json.loads((root/'config/codex-thread-studio/settings.json').read_text())['desktopNotifications'] is False
                    loop.quit();return False
        except BaseException as error:errors.append(str(error));loop.quit();return False
        return True
    GLib.timeout_add(40,poll)
    try:loop.run()
    finally:
        connection.unregister_object(registration)
        if observer:memory.append(observer.close())
        if child.poll() is None:child.terminate();child.wait(timeout=10)
        if service_pid and Path('/proc',str(service_pid)).exists():
            try:
                if Path('/proc',str(service_pid),'exe').resolve()==Path(native):os.kill(service_pid,signal.SIGTERM)
            except OSError:pass
    assert not errors,(root,errors)
    assert second_started
    assert not runtime.starts(root),'Notification fixture triggered a model turn'
    for name in ['backend.pid','opencode.pid']:
        if not (root/name).exists():continue
        pid=(root/name).read_text().strip()
        for _ in range(100):
            if not Path('/proc',pid).exists():break
            time.sleep(.05)
        assert not Path('/proc',pid).exists(),f'Owned backend {pid} survived notification test'
    for phase in ['emit','restore']:
        text=(root/f'app-{phase}.log').read_text()
        assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error']),(root,text[-2000:])
    raw=json.loads((root/'config/codex-thread-studio/settings.json').read_text());assert raw['futureSetting']['preserve'] and raw['futureNotification']['preserve']
    print(json.dumps({'ok':True,'privateGioTransport':True,'activationAfterExit':True,'startupLedgerBound':True,'physicalFocusAcceptance':'manual','notificationCount':len([v for v in records if v['method']=='add']),'backendCleanup':True,'memory':memory,'artifacts':str(root)}))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--api-only',action='store_true');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=map(lambda p:str(Path(p).resolve()),[args.native,args.gateway])
    if args.private_root:return private_bus(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='notifications-ui-',dir=base));runtime.create_profile(root);api_checks(gateway,root)
    if args.api_only:print(json.dumps({'ok':True,'notificationsApi':True,'artifacts':str(root)}));return
    backend=runtime.CODEX.replace("    elif method=='thread/list':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/list':",1).replace("if 'id' not in request:continue","if 'id' not in request or 'method' not in request:continue")
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:path=root/name;path.write_text(source);path.chmod(0o700)
    appid='io.github.messon007.codexthreadstudio.Gnome.SmokeNotifications'+''.join(c for c in root.name if c.isalnum())
    apps=root/'data/applications';apps.mkdir(parents=True,exist_ok=True);(apps/f'{appid}.desktop').write_text(f'[Desktop Entry]\nType=Application\nName=Private notification fixture\nExec={native}\nDBusActivatable=true\nX-GNOME-UsesNotifications=true\n')
    launcher=root/'notification-restore-launcher';launcher.write_text('#!/usr/bin/python3\nimport os\nos.environ["STUDIO_GNOME_SMOKE_NOTIFICATIONS"]="restore"\nlog=open('+repr(str(root/'app-restore.log'))+',"w")\nos.dup2(log.fileno(),1)\nos.dup2(log.fileno(),2)\nos.execv('+repr(native)+',['+repr(native)+',"--gapplication-service"])\n');launcher.chmod(0o700)
    services=root/'data/dbus-1/services';services.mkdir(parents=True,exist_ok=True);(services/f'{appid}.service').write_text(f'[D-BUS Service]\nName={appid}\nExec="{launcher}"\n')
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_APPLICATION_ID=appid)
    env.pop('DISPLAY',None);env.pop('XAUTHORITY',None)
    with isolated_environment(env) as private:
        subprocess.run(['dbus-run-session','--','/usr/bin/python3',str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=120)

if __name__=='__main__':main()
