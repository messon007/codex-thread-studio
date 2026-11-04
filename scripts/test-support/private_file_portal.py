"""Private FileChooser/OpenURI transport; returns fixture URIs and launches nothing."""
import fcntl, json, os, uuid
from pathlib import Path
from private_uri_portal import UriPortal

class FilePortal(UriPortal):
    def __init__(self, root, plan):
        super().__init__(root)
        self.plan=list(plan);self.calls=[];self.responses={};self.failure=None
        xml='''<node>
        <interface name="org.freedesktop.portal.OpenURI">
          <method name="OpenURI"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
          <method name="OpenFile"><arg type="s" direction="in"/><arg type="h" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
          <method name="OpenDirectory"><arg type="s" direction="in"/><arg type="h" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
          <property name="version" type="u" access="read"/>
        </interface>
        <interface name="org.freedesktop.portal.FileChooser">
          <method name="OpenFile"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
          <method name="SaveFile"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
          <property name="version" type="u" access="read"/>
        </interface></node>'''
        info=self.Gio.DBusNodeInfo.new_for_xml(xml)
        self.bus.unregister_object(self.registrations[0])
        self.registrations[0]=self.bus.register_object('/org/freedesktop/portal/desktop',info.interfaces[0],self.method,lambda *args:self.GLib.Variant('u',5),None)
        self.registrations.append(self.bus.register_object('/org/freedesktop/portal/desktop',info.interfaces[1],self.method,lambda *args:self.GLib.Variant('u',4),None))
    def record(self):
        temp=self.root/'file-portal.tmp'
        temp.write_text(json.dumps({'calls':self.calls,'closed':self.closed,'trace':self.trace,'failure':self.failure},ensure_ascii=False));temp.replace(self.root/'file-portal.json')
    def method(self, connection, sender, path, interface, method, parameters, invocation):
        if method in ['Read','ReadOne','ReadAll','Close']:
            entry=self.objects.get(path);case=self.responses.get(path,{})
            super().method(connection,sender,path,interface,method,parameters,invocation)
            if method=='Close' and entry and case.get('lateUris'):
                # Deliberately race a successful result after cancellation;
                # the client must have invalidated its pending selection.
                destination=entry[1]
                def late():
                    self.trace.append({'event':'late-response','handle':path,'status':0});self.record()
                    self.bus.emit_signal(destination,path,'org.freedesktop.portal.Request','Response',self.GLib.Variant('(ua{sv})',(0,{'uris':self.GLib.Variant('as',case['lateUris'])})))
                    return False
                self.GLib.timeout_add(80,late)
            self.record();return
        try:
            parent,selected,options=parameters.unpack()
            assert parent.startswith('wayland:'),('Missing Wayland parent',parent)
            request={'interface':interface.rsplit('.',1)[-1],'method':method,'parent':parent,'options':options}
            if interface.endswith('FileChooser'):request['title']=selected
            elif method=='OpenURI':raise AssertionError('Local file handoff used OpenURI instead of an FD')
            else:
                descriptor=invocation.get_message().get_unix_fd_list().get(selected)
                try:
                    target=Path(os.readlink('/proc/self/fd/'+str(descriptor))).resolve()
                    assert target.is_relative_to(self.root),('FD escaped private fixture',str(target))
                    request['path']=str(target)
                    assert fcntl.fcntl(descriptor,fcntl.F_GETFL)&os.O_ACCMODE==os.O_RDONLY,'File-manager transport requested a write descriptor'
                    request['readOnlyFd']=True
                finally:os.close(descriptor)
            self.calls.append(request)
            assert self.plan,('Unexpected portal call',request)
            case=self.plan.pop(0)
            assert request['interface']==case['interface'] and method==case['method'],(case,request)
            if 'title' in case:assert request['title']==case['title'],(case,request)
            if 'path' in case:assert request['path']==case['path'],(case,request)
            for key,value in case.get('options',{}).items():assert options.get(key,False)==value,(key,options,case)
            token=options.get('handle_token') or 'private_'+uuid.uuid4().hex
            handle='/org/freedesktop/portal/desktop/request/'+sender.lstrip(':').replace('.','_')+'/'+token
            request['handle']=handle
            self.objects[handle]=(self.bus.register_object(handle,self.info.interfaces[2],self.method,None,None),sender)
            self.responses[handle]=case
            invocation.return_value(self.GLib.Variant('(o)',(handle,)))
            self.record()
            if not case.get('hold'):
                def finish():self.finish(handle,case.get('status',0));return False
                self.GLib.timeout_add(80,finish)
        except Exception as error:
            self.failure=str(error);self.record()
            invocation.return_dbus_error('org.freedesktop.portal.Error.Failed','Private file portal rejected request')
    def finish(self, handle, status):
        entry=self.objects.pop(handle,None)
        if entry is None:return
        case=self.responses.pop(handle,{})
        # GTK's decoder consumes this field for cancellation responses too.
        results={'uris':self.GLib.Variant('as',case.get('uris',[]) if status==0 else [])} if case.get('interface')=='FileChooser' else {}
        registration,destination=entry
        self.trace.append({'event':'response','handle':handle,'status':status})
        self.bus.emit_signal(destination,handle,'org.freedesktop.portal.Request','Response',self.GLib.Variant('(ua{sv})',(status,results)))
        self.bus.unregister_object(registration);self.record()
