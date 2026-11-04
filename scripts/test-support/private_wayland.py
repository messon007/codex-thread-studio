"""Private desktop services for GUI fixtures using the real Wayland compositor.

Use the yielded environment with dbus-run-session. A short runtime path keeps
AT-SPI's UNIX socket within its path limit and avoids touching the user's bus.
"""
from contextlib import contextmanager
from pathlib import Path
import tempfile
import subprocess
import os
import time


def _virtual_monitor_args(monitors):
    import re
    if not isinstance(monitors,(tuple,list)) or not 1<=len(monitors)<=2:
        raise RuntimeError('Private Shell requires one or two bounded virtual monitors')
    result=[]
    for monitor in monitors:
        match=re.fullmatch(r'(\d{3,4})x(\d{3,4})',monitor) if isinstance(monitor,str) else None
        if not match or not (1280<=int(match[1])<=3840 and 720<=int(match[2])<=2160):
            raise RuntimeError('Virtual monitor geometry exceeds the private fixture bounds')
        result+=['--virtual-monitor',monitor]
    return result


@contextmanager
def owned_gnome_shell(root, monitors=('1600x1000',)):
    """Run one real GNOME Shell on the caller's private bus and Wayland runtime.

    Reuse this for desktop activation, IME and accessibility acceptance. It must
    never start on a login bus or attach input to the user's compositor.
    """
    root = Path(root).resolve(strict=True)
    runtime = Path(os.environ.get('XDG_RUNTIME_DIR', ''))
    if (Path('/proc', str(os.getppid()), 'exe').resolve() != Path('/usr/bin/dbus-run-session').resolve()
            or runtime.parent != Path('/tmp') or not runtime.name.startswith('studio-wayland-')
            or not runtime.is_dir() or os.environ.get('XDG_CONFIG_HOME') != str(root/'config')):
        raise RuntimeError('GNOME Shell requires the owned session bus, runtime and configuration')
    from gi.repository import Gio, GLib
    monitor_args = _virtual_monitor_args(monitors)
    previous = os.environ.get('WAYLAND_DISPLAY')
    os.environ['WAYLAND_DISPLAY'] = 'studio-shell'
    controlled = None
    with (root/'gnome-shell.log').open('w') as log:
        shell = subprocess.Popen(['/usr/bin/gnome-shell', '--headless', '--wayland',
            '--no-x11', '--wayland-display', 'studio-shell', *monitor_args],
            stdout=log, stderr=log, start_new_session=True)
        try:
            bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
            ready = None
            deadline = time.monotonic()+30
            while time.monotonic()<deadline:
                if shell.poll() is not None:
                    raise RuntimeError(('Private GNOME Shell exited', shell.returncode, str(root)))
                try:
                    owner = bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                        'org.freedesktop.DBus', 'GetNameOwner', GLib.Variant('(s)', ('org.gnome.Shell',)),
                        GLib.VariantType.new('(s)'), Gio.DBusCallFlags.NONE, 1000, None).unpack()[0]
                    pid = bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                        'org.freedesktop.DBus', 'GetConnectionUnixProcessID', GLib.Variant('(s)', (owner,)),
                        GLib.VariantType.new('(u)'), Gio.DBusCallFlags.NONE, 1000, None).unpack()[0]
                    bus.call_sync(owner, '/org/gnome/Shell', 'org.freedesktop.DBus.Properties',
                        'Get', GLib.Variant('(ss)', ('org.gnome.Shell', 'OverviewActive')),
                        GLib.VariantType.new('(v)'), Gio.DBusCallFlags.NONE, 1000, None)
                    if pid==shell.pid and (runtime/'studio-shell').is_socket():
                        ready = ready or time.monotonic()
                        if time.monotonic()-ready>=2:
                            break
                except GLib.Error:
                    ready = None
                time.sleep(.05)
            else:
                raise RuntimeError(('Private GNOME Shell did not become ready', str(root)))
            controlled = OwnedMutterInput(compositor=shell)
            yield shell, controlled
        finally:
            try:
                if controlled is not None and shell.poll() is None:
                    controlled.close()
            finally:
                if shell.poll() is None:
                    shell.terminate()
                    try:
                        shell.wait(timeout=15)
                    except subprocess.TimeoutExpired:
                        shell.kill()
                        shell.wait(timeout=5)
                if previous is None:
                    os.environ.pop('WAYLAND_DISPLAY', None)
                else:
                    os.environ['WAYLAND_DISPLAY'] = previous


def _input_compositor_pid(compositor):
    if compositor is None:
        parent = os.getppid()
        if Path('/proc', str(parent), 'exe').resolve() != Path('/usr/bin/mutter').resolve():
            raise RuntimeError('Virtual input requires the owned Mutter parent process')
        return parent
    if not isinstance(compositor, subprocess.Popen) or compositor.poll() is not None:
        raise RuntimeError('Virtual input requires a live owned compositor subprocess')
    pid = compositor.pid
    fields = Path('/proc', str(pid), 'stat').read_text().rsplit(')', 1)[1].split()
    if int(fields[1]) != os.getpid():
        raise RuntimeError('Virtual input compositor is not a child of this fixture')
    executable = Path('/proc', str(pid), 'exe').resolve(strict=True)
    if executable not in [Path('/usr/bin/mutter').resolve(), Path('/usr/bin/gnome-shell').resolve()]:
        raise RuntimeError('Virtual input subprocess is not a supported compositor')
    argv = Path('/proc', str(pid), 'cmdline').read_bytes().split(b'\0')
    if not all(flag in argv for flag in [b'--headless', b'--wayland', b'--no-x11']) or b'--replace' in argv:
        raise RuntimeError('Owned compositor must be a separate headless Wayland server')
    runtime = Path(os.environ.get('XDG_RUNTIME_DIR', ''))
    if runtime.parent != Path('/tmp') or not runtime.name.startswith('studio-wayland-') or not runtime.is_dir():
        raise RuntimeError('Owned compositor requires the fixture private runtime')
    return pid


class OwnedMutterInput:
    """Input on the parent Mutter or a verified owned headless compositor.

    Mutter's standalone plugin can map an application without focusing it.
    Activate the sole test window through a private remote keyboard session.
    """
    def __init__(self, compositor=None):
        expected = _input_compositor_pid(compositor)
        from gi.repository import Gio, GLib
        self.Gio,self.GLib=Gio,GLib
        self.bus=Gio.bus_get_sync(Gio.BusType.SESSION,None)
        self.service='org.gnome.Mutter.RemoteDesktop'
        owner=self.bus.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','GetNameOwner',GLib.Variant('(s)',(self.service,)),GLib.VariantType.new('(s)'),Gio.DBusCallFlags.NONE,3000,None).unpack()[0]
        identity=self.bus.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','GetConnectionUnixProcessID',GLib.Variant('(s)',(owner,)),GLib.VariantType.new('(u)'),Gio.DBusCallFlags.NONE,3000,None).unpack()[0]
        if identity!=expected:raise RuntimeError('Remote input service is not the owned compositor')
        self.target=owner
        self.path=self.bus.call_sync(self.target,'/org/gnome/Mutter/RemoteDesktop',self.service,'CreateSession',None,GLib.VariantType.new('(o)'),Gio.DBusCallFlags.NONE,3000,None).unpack()[0]
        self.call('Start')

    def call(self,method,args=None):
        return self.bus.call_sync(self.target,self.path,self.service+'.Session',method,args,None,self.Gio.DBusCallFlags.NONE,3000,None)

    def focus_only_window(self):
        for key,state in [(0xffe9,True),(0xff09,True),(0xff09,False),(0xffe9,False)]:
            self.call('NotifyKeyboardKeysym',self.GLib.Variant('(ub)',(key,state)))

    def activate_focused_control(self):
        for state in (True,False):
            self.call('NotifyKeyboardKeysym',self.GLib.Variant('(ub)',(0x20,state)))

    def enable_clipboard_reader(self):
        # No mime-types: observe/read only, without becoming clipboard owner.
        self.call('EnableClipboard',self.GLib.Variant('(a{sv})',({},)))

    def respond(self, root):
        """Service one fixture request without consulting any host desktop bus."""
        import json,time
        path=Path(root)/'clipboard-request.json'
        if not path.exists():return
        request=json.loads(path.read_text());path.unlink()
        try:
            if request['action']=='pointer':
                control=request.get('control',False)
                if not isinstance(control,bool):raise RuntimeError('Pointer control modifier must be boolean')
                if control:self.call('NotifyKeyboardKeysym',self.GLib.Variant('(ub)',(0xffe3,True)))
                try:self.pointer(request['x'],request['y'],request.get('dx',0),request.get('dy',0),request.get('button',272),request.get('hold_ms',40),request.get('clicks',1))
                finally:
                    if control:self.call('NotifyKeyboardKeysym',self.GLib.Variant('(ub)',(0xffe3,False)))
                response={'ok':True}
            elif request['action']=='read':response={'text':self.clipboard_text()}
            elif request['action']=='read-primary':
                # Separate Wayland client, connected only to this owned display.
                text=subprocess.run(['wl-paste','--primary','--no-newline','--type','text/plain;charset=utf-8'],capture_output=True,check=True,timeout=5).stdout
                if len(text)>2097152:raise RuntimeError('Private primary selection exceeds 2 MiB')
                response={'text':text.decode('utf-8')}
            elif request['action']=='focus':self.focus_only_window();response={'ok':True}
            elif request['action']=='key':
                keys={'copy':[0xffe3,ord('c')],'translate':[0xffe3,0xffe1,ord('t')],'find':[0xffe3,ord('f')],'find-next':[0xffc0],'find-previous':[0xffe1,0xffc0],'escape':[0xff1b],'space':[0x20],'enter':[0xff0d],'menu':[0xffe1,0xffc7],'open-link':[0xffe3,0xff0d],'close-window':[0xffe9,0xffc1],'document-start':[0xffe3,0xff50],'document-end':[0xffe3,0xff57],'select-document-end':[0xffe3,0xffe1,0xff57],'page-up':[0xff55]}[request['key']]
                for key in keys:self.call('NotifyKeyboardKeysym',self.GLib.Variant('(ub)',(key,True)))
                # Let GTK handle the press before a release changes the seat's
                # last serial; native popup grabs must use a valid input serial.
                time.sleep(.06)
                for key in reversed(keys):self.call('NotifyKeyboardKeysym',self.GLib.Variant('(ub)',(key,False)))
                response={'ok':True}
            else:raise RuntimeError('Unknown private input action')
        except Exception as error:response={'error':str(error)}
        temporary=Path(root)/'clipboard-response.tmp'
        temporary.write_text(json.dumps(response));temporary.replace(Path(root)/'clipboard-response.json')

    def clipboard_text(self):
        """Read only this owned compositor's clipboard, independently of GTK."""
        import select,time
        reply,descriptors=self.bus.call_with_unix_fd_list_sync(
            self.target,self.path,self.service+'.Session','SelectionRead',
            self.GLib.Variant('(s)',('text/plain;charset=utf-8',)),
            self.GLib.VariantType.new('(h)'),self.Gio.DBusCallFlags.NONE,3000,None,None)
        descriptor=descriptors.get(reply.unpack()[0]);chunks=[];size=0;deadline=time.monotonic()+5
        try:
            while True:
                remaining=deadline-time.monotonic()
                if remaining<=0 or not select.select([descriptor],[],[],remaining)[0]:
                    raise TimeoutError('Private clipboard transfer did not complete')
                chunk=os.read(descriptor,65536)
                if not chunk:break
                size+=len(chunk)
                if size>2097152:raise RuntimeError('Private clipboard exceeds 2 MiB')
                chunks.append(chunk)
            return b''.join(chunks).decode('utf-8')
        finally:os.close(descriptor)

    def pointer(self, x, y, dx=0.0, dy=0.0, button=272, hold_ms=40, clicks=1):
        """Coordinates in the fixture's sole, scale-one fullscreen monitor.

        Only callers that verify their own fullscreen allocation should use this.
        Relative motion starts at the clamped monitor origin, so no host pointer
        position, desktop service or global automation API is consulted.
        """
        import time,math
        if type(clicks) is not int or not 1<=clicks<=3 or (clicks>1 and (dx or dy)):raise RuntimeError('Invalid private pointer click count')
        if not isinstance(hold_ms,(int,float)) or not 40<=hold_ms<=500:raise RuntimeError('Invalid private pointer hold time')
        if button not in (272,273,274):raise RuntimeError('Unsupported private pointer button')
        if not all(math.isfinite(v) for v in [x,y,dx,dy]) or not (0<=x<1600 and 0<=y<1000 and 0<=x+dx<1600 and 0<=y+dy<1000):
            raise RuntimeError('Pointer coordinates escape the owned 1600x1000 monitor')
        self.call('NotifyPointerMotionRelative',self.GLib.Variant('(dd)',(-10000.0,-10000.0)))
        self.call('NotifyPointerMotionRelative',self.GLib.Variant('(dd)',(x,y)))
        time.sleep(.04)
        self.call('NotifyPointerButton',self.GLib.Variant('(ib)',(button,True)))
        time.sleep(hold_ms/1000)
        if dx or dy:
            for _ in range(4):
                self.call('NotifyPointerMotionRelative',self.GLib.Variant('(dd)',(dx/4,dy/4)))
                time.sleep(.02)
        self.call('NotifyPointerButton',self.GLib.Variant('(ib)',(button,False)))
        for _ in range(clicks-1):
            time.sleep(.08)
            self.call('NotifyPointerButton',self.GLib.Variant('(ib)',(button,True)))
            time.sleep(.04)
            self.call('NotifyPointerButton',self.GLib.Variant('(ib)',(button,False)))

    def close(self):
        self.call('Stop')


@contextmanager
def isolated_environment(environment, headless=False):
    private = dict(environment)
    # GUI fixtures use JSONL fake CLIs, not the shared-daemon WebSocket proxy.
    # Do not inherit a user's live daemon mode into an isolated fake profile.
    private['CODEX_THREAD_STUDIO_CODEX_CONNECTION'] = 'embedded'
    display = None
    if not headless:
        display = Path(private.get('WAYLAND_DISPLAY', 'wayland-0'))
        if not display.is_absolute():
            if not private.get('XDG_RUNTIME_DIR'):
                raise RuntimeError('Run the fixture inside a Wayland desktop session')
            display = Path(private['XDG_RUNTIME_DIR']) / display
        if not display.is_socket():
            raise RuntimeError('The Wayland display socket is unavailable')
    with tempfile.TemporaryDirectory(prefix='studio-wayland-', dir='/tmp') as directory:
        private['XDG_RUNTIME_DIR'] = directory
        if display is not None:private['WAYLAND_DISPLAY'] = str(display)
        else:private.pop('WAYLAND_DISPLAY',None)
        for key in ['DISPLAY', 'XAUTHORITY', 'AT_SPI_BUS_ADDRESS']:
            private.pop(key, None)
        try:
            yield private
        finally:
            # Private portal/GVfs services can leave disconnected FUSE mounts
            # after their session bus exits. Detach only owned, typed mounts.
            detach_owned_mounts(directory)


def detach_owned_mounts(directory):
    root=Path(directory)
    if root.parent!=Path('/tmp') or not root.name.startswith('studio-wayland-'):
        raise RuntimeError('Mount cleanup requires its private fixture runtime')
    allowed={str(root/'doc'):'fuse.portal',str(root/'gvfs'):'fuse.gvfsd-fuse'}
    for line in Path('/proc/self/mountinfo').read_text().splitlines():
        fields=line.split();split=fields.index('-');mount=fields[4]
        if mount not in allowed:continue
        if fields[split+1]!=allowed[mount]:
            raise RuntimeError('Unexpected mount in the private fixture runtime')
        result=subprocess.run(['fusermount3','-uz','--',mount],capture_output=True,text=True,timeout=5)
        if result.returncode:
            # Either private service may finish detaching after the snapshot.
            remaining=[item.split() for item in Path('/proc/self/mountinfo').read_text().splitlines()]
            if any(item[4]==mount for item in remaining):
                raise subprocess.CalledProcessError(result.returncode,result.args,output=result.stdout,stderr=result.stderr)
