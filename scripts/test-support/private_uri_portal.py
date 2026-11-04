"""A private OpenURI portal for GTK fixtures; records URIs and launches nothing."""
import fnmatch,json,os,uuid
from pathlib import Path
class UriPortal:
    def __init__(self,root):
        from gi.repository import Gio,GLib
        runtime=Path(os.environ.get('XDG_RUNTIME_DIR','')).resolve()
        if runtime.parent!=Path('/tmp') or not runtime.name.startswith('studio-wayland-'):
            raise RuntimeError('URI portal fixture requires its isolated Wayland runtime')
        self.Gio,self.GLib=Gio,GLib;self.root=Path(root);self.requests=[];self.closed=[];self.objects={}
        self.bus=Gio.bus_get_sync(Gio.BusType.SESSION,None)
        self.trace=[]
        # GTK Wayland stores this portal factor as 16.16. An empty successful
        # ReadAll leaves it at zero; subsequent WebKit views then get zero DPI.
        # Keep the private desktop deterministic instead of reading host settings.
        self.settings={'org.gnome.desktop.interface':{'text-scaling-factor':GLib.Variant('d',1.0)}}
        name=self.bus.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','RequestName',GLib.Variant('(su)',('org.freedesktop.portal.Desktop',4)),GLib.VariantType.new('(u)'),Gio.DBusCallFlags.NONE,5000,None).unpack()[0]
        if name!=1:raise RuntimeError('Private portal name is already owned')
        xml='''<node><interface name="org.freedesktop.portal.OpenURI"><method name="OpenURI"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method><property name="version" type="u" access="read"/></interface><interface name="org.freedesktop.portal.Settings"><method name="ReadAll"><arg type="as" direction="in"/><arg type="a{sa{sv}}" direction="out"/></method><method name="Read"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="out"/></method><method name="ReadOne"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="out"/></method><property name="version" type="u" access="read"/></interface><interface name="org.freedesktop.portal.Request"><method name="Close"/><signal name="Response"><arg type="u"/><arg type="a{sv}"/></signal></interface></node>'''
        self.info=Gio.DBusNodeInfo.new_for_xml(xml)
        self.registrations=[]
        for interface in self.info.interfaces[:2]:
            version=2 if interface.name=='org.freedesktop.portal.Settings' else 3
            self.registrations.append(self.bus.register_object('/org/freedesktop/portal/desktop',interface,self.method,lambda *args,_version=version:GLib.Variant('u',_version),None))
    def method(self,connection,sender,path,interface,method,parameters,invocation):
        if method=='ReadAll':
            patterns=parameters.unpack()[0]
            values={namespace:settings for namespace,settings in self.settings.items() if not patterns or '' in patterns or any(fnmatch.fnmatchcase(namespace,pattern) for pattern in patterns)}
            invocation.return_value(self.GLib.Variant('(a{sa{sv}})',(values,)));return
        if method in ('Read','ReadOne'):
            namespace,key=parameters.unpack();value=self.settings.get(namespace,{}).get(key)
            if value is None:invocation.return_dbus_error('org.freedesktop.portal.Error.NotFound','No private fixture setting')
            else:
                if method=='Read':value=self.GLib.Variant('v',value)
                invocation.return_value(self.GLib.Variant('(v)',(value,)))
            return
        if method=='Close':
            self.closed.append(path);self.trace.append({'event':'close','handle':path});(self.root/'portal-trace.json').write_text(json.dumps(self.trace));invocation.return_value(None)
            entry=self.objects.pop(path,None)
            if entry:self.bus.unregister_object(entry[0])
            return
        parent,uri,options=parameters.unpack();token=options.get('handle_token') or 'private_'+uuid.uuid4().hex
        handle='/org/freedesktop/portal/desktop/request/'+sender.lstrip(':').replace('.','_')+'/'+token
        self.trace.append({'event':'request','sender':sender,'handle':handle,'options':options});(self.root/'portal-trace.json').write_text(json.dumps(self.trace));self.requests.append(uri);(self.root/'portal-uris.json').write_text(json.dumps(self.requests))
        self.objects[handle]=(self.bus.register_object(handle,self.info.interfaces[2],self.method,None,None),sender)
        invocation.return_value(self.GLib.Variant('(o)',(handle,)))
        def response():
            if not (self.root/'hold-uri').exists():self.finish(handle,0)
            return False
        self.GLib.timeout_add(60,response)
    def finish(self,handle,status):
        entry=self.objects.pop(handle,None)
        if entry is None:return  # A timer after Close did not deliver a response.
        self.trace.append({'event':'response','handle':handle,'status':status});(self.root/'portal-trace.json').write_text(json.dumps(self.trace))
        registration,destination=entry
        self.bus.emit_signal(destination,handle,'org.freedesktop.portal.Request','Response',self.GLib.Variant('(ua{sv})',(status,{})))
        self.bus.unregister_object(registration)
    def iterate(self):
        context=self.GLib.MainContext.default()
        while context.pending():context.iteration(False)
    def close(self):
        for handle in list(self.objects):self.finish(handle,1)
        for registration in self.registrations:self.bus.unregister_object(registration)
        self.bus.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','ReleaseName',self.GLib.Variant('(s)',('org.freedesktop.portal.Desktop',)),None,self.Gio.DBusCallFlags.NONE,3000,None)
