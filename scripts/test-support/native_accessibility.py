"""Bounded AT-SPI queries/actions restricted to one owned native process."""
import time
class Client:
    def __init__(self,process,iterate=None):
        import gi
        gi.require_version('Atspi','2.0')
        from gi.repository import Atspi,GLib
        self.api,self.glib,self.process=Atspi,GLib,process
        self.iterate=iterate or (lambda:None)
        Atspi.init();Atspi.set_timeout(1500,1500)
    def application(self):
        desktop=self.api.get_desktop(0)
        for index in range(desktop.get_child_count()):
            app=desktop.get_child_at_index(index)
            if app and app.get_process_id()==self.process.pid:return app
    def tree(self):
        pending=[self.application()];result=[]
        while pending:
            node=pending.pop()
            if node is None:continue
            if len(result)>=2500:raise RuntimeError('Owned accessibility tree exceeded its bound')
            try:
                node.clear_cache();result.append(node)
                pending.extend(node.get_child_at_index(i) for i in reversed(range(node.get_child_count())))
            except self.glib.Error:continue
        return result
    def find(self,predicate):
        for node in self.tree():
            try:
                if predicate(node):return node
            except self.glib.Error:continue
    def describe(self):
        result=[]
        for node in self.tree():
            try:result.append(dict(name=node.get_name(),role=node.get_role_name()))
            except self.glib.Error:continue
        return result
    def showing(self,node):return node.get_state_set().contains(self.api.StateType.SHOWING)
    def sensitive(self,node):return node.get_state_set().contains(self.api.StateType.SENSITIVE)
    def wait(self,name,test,seconds=25):
        deadline=time.monotonic()+seconds
        while time.monotonic()<deadline:
            self.iterate()
            if self.process.poll() is not None:raise RuntimeError((name,'Owned application exited',self.process.returncode))
            try:
                result=test()
                if result:return result
            except self.glib.Error:pass
            time.sleep(.05)
        raise RuntimeError(('Native control timed out',name))
    def activate(self,node):
        assert node.get_application().get_process_id()==self.process.pid
        interface=node.get_action_iface()
        names=[self.api.Action.get_action_name(interface,i) for i in range(self.api.Action.get_n_actions(interface))] if interface else []
        for name in ['click','activate']:
            if name in names:
                assert self.api.Action.do_action(interface,names.index(name));return
        raise RuntimeError(('Owned native control has no activation action',node.get_name(),names))
    def click(self,name):
        def actionable(node):
            if node.get_name()!=name or not self.showing(node) or not self.sensitive(node):return False
            interface=node.get_action_iface()
            return interface and any(self.api.Action.get_action_name(interface,i) in ('click','activate') for i in range(self.api.Action.get_n_actions(interface)))
        # GtkMenuButton exports a named wrapper and a named toggle child. Only
        # the child has an action; labels can likewise duplicate button names.
        node=self.wait(name,lambda:self.find(actionable))
        self.activate(node);return node
    def text(self,value):return self.find(lambda n:self.showing(n) and value in n.get_name())
    def within(self,node,parent):
        for _ in range(40):
            if node==parent:return True
            node=node.get_parent()
            if node is None:return False
        return False
    def contents(self,node):
        assert node.get_application().get_process_id()==self.process.pid
        interface=node.get_text_iface()
        return self.api.Text.get_text(interface,0,-1) if interface else None
    def edit(self,node,value):
        assert node.get_application().get_process_id()==self.process.pid
        interface=node.get_editable_text_iface()
        assert interface and self.api.EditableText.set_text_contents(interface,value),'Owned control rejected native text editing'
