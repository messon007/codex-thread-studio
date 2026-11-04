"""Temporary monitor configuration, restricted to an owned headless compositor."""
from pathlib import Path
import os
import time
from private_wayland import _input_compositor_pid


class OwnedDisplays:
    INTERFACE='org.gnome.Mutter.DisplayConfig'
    PATH='/org/gnome/Mutter/DisplayConfig'

    def __init__(self,root,compositor):
        self.root=Path(root).resolve(strict=True)
        self.compositor=compositor
        expected=_input_compositor_pid(compositor)
        if (Path('/proc',str(os.getppid()),'exe').resolve()!=Path('/usr/bin/dbus-run-session').resolve()
                or os.environ.get('XDG_CONFIG_HOME')!=str(self.root/'config')):
            raise RuntimeError('Display acceptance requires the owned session bus and private profile')
        from gi.repository import Gio,GLib
        self.Gio,self.GLib=Gio,GLib
        self.bus=Gio.bus_get_sync(Gio.BusType.SESSION,None)
        self.target=self._bus_call('GetNameOwner',GLib.Variant('(s)',(self.INTERFACE,))).unpack()[0]
        self._verify_owner(expected)

    def _bus_call(self,method,args):
        return self.bus.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus',
            'org.freedesktop.DBus',method,args,None,self.Gio.DBusCallFlags.NONE,3000,None)

    def _verify_owner(self,expected=None):
        actual=self._bus_call('GetConnectionUnixProcessID',self.GLib.Variant('(s)',(self.target,))).unpack()[0]
        expected=_input_compositor_pid(self.compositor) if expected is None else expected
        if actual!=expected:raise RuntimeError('Display service is not the owned compositor')

    def _call(self,method,args=None):
        if method not in ['GetCurrentState','ApplyMonitorsConfig']:
            raise RuntimeError('Unsupported private display operation')
        self._verify_owner()
        return self.bus.call_sync(self.target,self.PATH,self.INTERFACE,method,args,None,
            self.Gio.DBusCallFlags.NONE,5000,None)

    def state(self):
        return self._call('GetCurrentState').unpack()

    def apply_scales(self,scales):
        state=self.state()
        serial,monitors,_,properties=state
        if len(scales)!=len(monitors) or not 1<=len(scales)<=2:
            raise RuntimeError('A scale is required for each owned virtual monitor')
        if properties.get('layout-mode',1)!=1 and not properties.get('supports-changing-layout-mode',False):
            raise RuntimeError('The owned compositor does not support logical display layout')
        logical=[];x=0
        for index,(monitor,scale) in enumerate(zip(monitors,scales)):
            spec,modes,_=monitor
            mode=next((m for m in modes if m[6].get('is-current')),None) or next(m for m in modes if m[6].get('is-preferred'))
            if isinstance(scale,bool) or not isinstance(scale,(int,float)) or not any(abs(value-scale)<1e-6 for value in mode[5]):
                raise RuntimeError('Requested scale is not advertised by the owned monitor')
            logical.append((x,0,float(scale),0,index==0,[(spec[0],mode[0],{})]))
            x+=round(mode[1]/scale)
        args=lambda method:self.GLib.Variant('(uua(iiduba(ssa{sv}))a{sv})',
            (serial,method,logical,{'layout-mode':self.GLib.Variant('u',1)} if properties.get('supports-changing-layout-mode') else {}))
        # Verify first; only temporary application is exposed. Never persist.
        self._call('ApplyMonitorsConfig',args(0))
        self._call('ApplyMonitorsConfig',args(1))
        end=time.monotonic()+10
        while time.monotonic()<end:
            current=self.state()
            if len(current[2])==len(scales) and all(abs(a[2]-b)<1e-6 for a,b in zip(current[2],scales)):
                return current
            time.sleep(.05)
        raise RuntimeError('Temporary display configuration was not observed')


DISPLAY_SOURCE='# Display scaling 中文😀\r\n\r\nReadable text retains its context across displays.\r\n\r\n| Name | Value |\r\n|---|---|\r\n| Alpha | One |\r\n| Unicode | 中文😀 |\r\n\r\n$$x^2 + y^2 = z^2$$\r\n'


def native_surface_state(path):
    """Read the native toplevel's real compositor output/fractional-scale events."""
    import re
    text=Path(path).read_text()
    top=re.search(r'xdg_surface#(\d+)\.get_toplevel\(',text)
    if not top:return None
    surface=re.search(r'get_xdg_surface\(new id xdg_surface#'+top[1]+r', wl_surface#(\d+)\)',text)
    if not surface:return None
    sid=surface[1]
    outputs={m[1]:m[2] for m in re.finditer(r'wl_output#(\d+)\.name\("([^"]+)"\)',text)}
    entered=set()
    for m in re.finditer(r'wl_surface#'+sid+r'\.(enter|leave)\(wl_output#(\d+)\)',text):
        if m[1]=='enter':entered.add(m[2])
        else:entered.discard(m[2])
    fraction=re.search(r'get_fractional_scale\(new id wp_fractional_scale_v1#(\d+), wl_surface#'+sid+r'\)',text)
    scales=re.findall(r'wp_fractional_scale_v1#'+fraction[1]+r'\.preferred_scale\((\d+)\)',text) if fraction else []
    return dict(surface=int(sid),outputs=sorted(outputs[i] for i in entered if i in outputs),
        scale=int(scales[-1])/120 if scales else None)


def exercise_native_displays(displays,root,client,composer,draft,keys,open_document):
    import json
    trace=root/'private-desktop.log'
    source=root/'project/display-scaling.md'
    original=source.read_bytes()
    assert original==DISPLAY_SOURCE.encode()
    initial=displays.state()
    assert len(initial[1])==2
    def wait(name,predicate):
        end=time.monotonic()+15
        while time.monotonic()<end:
            value=predicate()
            if value:return value
            time.sleep(.05)
        raise RuntimeError(('Display workflow timed out',name,str(root)))
    def bounds(node):
        b=node.get_component_iface().get_extents(client.api.CoordType.WINDOW)
        return dict(x=b.x,y=b.y,width=b.width,height=b.height)
    def control(name):
        return client.wait(name,lambda:client.find(lambda n:n.get_name()==name and client.showing(n) and n.get_role_name() in ['button','toggle button']))
    def screenshot():
        # GNOME's ordinary Alt+Print window capture: no privileged D-Bus caller
        # impersonation, application hook or access to the login display.
        home=root/'home'
        assert os.environ.get('HOME')==str(home)
        before=set(home.rglob('*.png'))
        keys([0xffe9,0xff61])
        image=wait('private GNOME window screenshot',lambda:next((p for p in home.rglob('*.png') if p not in before and p.stat().st_size>24),None))
        content=image.read_bytes()
        assert content[:8]==b'\x89PNG\r\n\x1a\n'
        return dict(path=str(image.relative_to(root)),width=int.from_bytes(content[16:20],'big'),height=int.from_bytes(content[20:24],'big'))
    open_document(source)
    client.wait('native scaling document',lambda:client.contains_text('Display scaling 中文😀'))
    samples=[]
    for scales in [(1.,1.25),(1.25,2.),(1.5,1.),(2.,1.25)]:
        state=displays.apply_scales(scales)
        for index in [0,1]:
            keys([0xffeb,0xffe1,0xff51 if index==0 else 0xff53])
            keys([0xffeb,0xff52])
            connector=state[1][index][0][0]
            actual=wait('native output and scale '+connector,lambda:(s if (s:=native_surface_state(trace)) and s['outputs']==[connector] and s['scale'] is not None and abs(s['scale']-scales[index])<1e-6 else None))
            frame=client.wait('active native frame',lambda:client.find(lambda n:n.get_role_name()=='frame' and n.get_state_set().contains(client.api.StateType.ACTIVE)))
            time.sleep(.2)
            box=bounds(frame)
            mode=next(m for m in state[1][index][1] if m[6].get('is-current'))
            assert box['width']<=round(mode[1]/scales[index])+2,('Native window exceeds logical monitor',box,scales,index)
            controls={name:bounds(control(name)) for name in ['Main menu','Close document','Source','Preview']}
            split=client.find(lambda n:n.get_name()=='Send' and client.showing(n)) is not None
            if split:
                controls['Send']=bounds(control('Send'))
                controls['composer']=bounds(composer)
            else:
                assert box['width']<1100,('Conversation disappeared outside compact layout',box)
            for name,b in controls.items():
                assert b['width']>0 and b['height']>0,(name,b)
                assert b['x']>=box['x']-1 and b['y']>=box['y']-1 and b['x']+b['width']<=box['x']+box['width']+1 and b['y']+b['height']<=box['y']+box['height']+1,('Clipped native control',name,b,box,scales,index)
            if split:assert client.contents(composer)==draft
            client.click('Source')
            client.wait('exact scaling source',lambda:client.find(lambda n:n.get_role_name()=='text' and client.showing(n) and client.contents(n)==DISPLAY_SOURCE))
            client.click('Preview')
            client.wait('scaling Preview ready',lambda:client.find(lambda n:n.get_name()=='Inspect math diagram' and n.get_role_name()=='button' and client.showing(n)))
            reading_image=screenshot()
            client.click('Main menu')
            client.wait('named Settings action at scale',lambda:client.find(lambda n:n.get_name()=='Settings…' and n.get_role_name()=='menu item' and client.showing(n)))
            keys([0xff1b])
            client.wait('scaled menu dismissed',lambda:not client.find(lambda n:n.get_name()=='Settings…' and n.get_role_name()=='menu item' and client.showing(n)))
            client.click('Close document')
            client.wait('reading pane actually closed',lambda:not client.find(lambda n:n.get_name()=='Close document' and client.showing(n)))
            send=control('Send')
            composer=client.wait('restored composer',lambda:client.find(lambda n:n.get_role_name()=='text' and n.get_name().startswith('Write a message.') and client.showing(n)))
            assert client.contents(composer)==draft
            conversation={'Send':bounds(send),'composer':bounds(composer)}
            for name,b in conversation.items():
                assert b['width']>0 and b['height']>0 and b['x']>=box['x']-1 and b['y']>=box['y']-1 and b['x']+b['width']<=box['x']+box['width']+1 and b['y']+b['height']<=box['y']+box['height']+1,('Clipped conversation after document close',name,b,box)
            conversation_image=screenshot()
            samples.append(dict(readingScreenshot=reading_image,conversationScreenshot=conversation_image,requestedScales=scales,monitor=connector,observed=actual,frame=box,controls=controls,readingLayout='split' if split else 'document-only',conversationAfterClose=conversation,sourceExact=True,draftRetained=True))
            open_document(source)
            client.wait('scaling document reopened',lambda:client.contains_text('Display scaling 中文😀'))
            (root/'display-samples.json').write_text(json.dumps(samples,ensure_ascii=False,indent=2))
    assert source.read_bytes()==original
    client.click('Close document')
    client.wait('final reading pane closed',lambda:not client.find(lambda n:n.get_name()=='Close document' and client.showing(n)))
    assert client.contents(composer)==draft
    # Restore a simple temporary layout before the existing desktop close/Open
    # checks. No persistent monitor configuration is ever requested.
    displays.apply_scales([1.,1.])
    result=dict(ok=True,realGnomeShell=True,virtualMonitors=True,physicalDisplays=False,
        applicationHooks=False,persistentConfiguration=False,temporaryMethod=1,
        noScaleEnvironmentOverride=True,samples=samples,sourceUnchanged=True,draftRetained=True)
    (root/'display-result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    return result
