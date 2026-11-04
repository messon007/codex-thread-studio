"""Bounded AT-SPI queries/actions restricted to one owned native process."""
import time
from pathlib import Path


class ProcessPeer:
    """Observe a bus-activated fixture process without claiming PID reuse."""
    def __init__(self, pid, executable):
        self.pid = pid
        self.executable = Path(executable).resolve(strict=True)
        self.started = self.identity()
        assert self.started is not None
        assert Path('/proc', str(pid), 'exe').resolve(strict=True) == self.executable

    def identity(self):
        try:
            fields = Path('/proc', str(self.pid), 'stat').read_text().rsplit(')', 1)[1].split()
            return None if fields[0] == 'Z' else fields[19]
        except OSError:
            return None

    def poll(self):
        return None if self.identity() == self.started else 1

    @property
    def returncode(self):
        # Not a child of this fixture; only termination is observable.
        return self.poll()


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
        for name in ['click','activate','toggle']:
            if name in names:
                assert self.api.Action.do_action(interface,names.index(name));return
        raise RuntimeError(('Owned native control has no activation action',node.get_name(),names))
    def click(self,name,parent=None):
        def actionable(node):
            if node.get_name()!=name or not self.showing(node) or not self.sensitive(node):return False
            if parent is not None and not self.within(node,parent):return False
            interface=node.get_action_iface()
            return interface and any(self.api.Action.get_action_name(interface,i) in ('click','activate','toggle') for i in range(self.api.Action.get_n_actions(interface)))
        # GtkMenuButton exports a named wrapper and a named toggle child. Only
        # the child has an action; labels can likewise duplicate button names.
        node=self.wait(name,lambda:self.find(actionable))
        self.activate(node);return node
    def text(self,value):return self.find(lambda n:self.showing(n) and value in n.get_name())
    def contains_text(self,value):
        """Read visible labels and TextViews; their text need not be a name."""
        return self.find(lambda n:self.showing(n) and (value in n.get_name()
            or (n.get_text_iface() and value in (self.contents(n) or ''))))
    def toggle(self,name,keys,parent=None):
        """Use a native check button, including GTK nodes without AT-SPI Action.

        `keys` must send input through the caller's owned private compositor.
        The fallback follows real Tab stops and observes the checked state.
        """
        node=self.wait(name,lambda:self.find(lambda n:n.get_name()==name and n.get_role_name() in ('check box','radio button') and self.showing(n) and self.sensitive(n) and (parent is None or self.within(n,parent))))
        def checked():
            node.clear_cache()
            return node.get_state_set().contains(self.api.StateType.CHECKED)
        before=checked()
        interface=node.get_action_iface()
        actions=[self.api.Action.get_action_name(interface,i) for i in range(self.api.Action.get_n_actions(interface))] if interface else []
        if any(action in actions for action in ('toggle','click','activate')):
            self.activate(node)
        else:
            # GTK 4.22 also omits Action from some radio/check nodes. This is
            # the same ordinary keyboard path used by installed agent-input QA.
            for _ in range(40):
                node.clear_cache()
                if node.get_state_set().contains(self.api.StateType.FOCUSED):
                    keys([0x20])
                    break
                focused=self.find(lambda n:n.get_state_set().contains(self.api.StateType.FOCUSED))
                # Code editors consume Tab as indentation. Ctrl+Tab is GTK's
                # native focus escape and leaves the user's expression intact.
                keys([0xffe3,0xff09] if focused and focused.get_role_name()=='text' else [0xff09])
            else:raise RuntimeError(('Owned check button was not in native Tab traversal',name))
        self.wait('checked state changed: '+name,lambda:checked()!=before)
        return node
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
    def open_context_menu(self,keys,labels,shortcut):
        """Reach a named context menu through the owned keyboard focus chain.

        GTK/WebKit can report a focused containing pane rather than document
        focus. Require the actual menu result, not arbitrary ancestor membership.
        Escape unrelated popups and leave text views with Ctrl+Tab to keep drafts.
        """
        self.context_focus_trace=[]
        for _ in range(100):
            current=self.find(lambda n:self.showing(n) and n.get_state_set().contains(self.api.StateType.FOCUSED))
            self.context_focus_trace.append(dict(name=current.get_name(),role=current.get_role_name()) if current else None)
            keys(shortcut)
            time.sleep(.12)
            rows={n.get_name():n for n in self.tree() if self.showing(n) and n.get_role_name()=='menu item'}
            if all(name in rows for name in labels):return {name:rows[name] for name in labels}
            keys([0xff1b])
            keys([0xffe3,0xff09] if current and current.get_role_name()=='text' else [0xff09])
        raise RuntimeError('Named context menu is not keyboard reachable')
