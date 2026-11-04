"""Mount teardown must never detach the user's desktop services."""
from pathlib import Path
import subprocess
import unittest
import sys
import os
import tempfile
import time
from unittest.mock import patch
from types import SimpleNamespace
import private_wayland
import private_orca
import private_displays
import local_codex_provider
from native_accessibility import ProcessPeer

ROOT='/tmp/studio-wayland-private-test'
def mount(path,kind):return f'1 0 0:1 / {path} rw - {kind} private rw\n'
class InputOwnership(unittest.TestCase):
    def test_arbitrary_objects_cannot_enable_virtual_input(self):
        with self.assertRaisesRegex(RuntimeError, 'live owned compositor'):
            private_wayland.OwnedMutterInput(object())

    def test_an_owned_non_compositor_process_cannot_enable_input(self):
        child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(10)'])
        try:
            with self.assertRaisesRegex(RuntimeError, 'supported compositor'):
                private_wayland.OwnedMutterInput(child)
        finally:
            child.terminate()
            child.wait(timeout=5)

    def test_default_mode_rejects_non_mutter_parents_before_bus_access(self):
        with patch.object(private_wayland.os, 'getppid', return_value=os.getpid()):
            with self.assertRaisesRegex(RuntimeError, 'Mutter parent'):
                private_wayland.OwnedMutterInput()

class ProcessObservation(unittest.TestCase):
    def test_exited_child_is_observed_before_its_zombie_is_reaped(self):
        child=subprocess.Popen([sys.executable,'-c','import sys; sys.stdin.read()'],stdin=subprocess.PIPE)
        try:
            peer=ProcessPeer(child.pid,sys.executable)
            self.assertIsNone(peer.poll())
            child.stdin.close()
            deadline=time.monotonic()+5
            while peer.poll() is None and time.monotonic()<deadline:
                time.sleep(.01)
            self.assertIsNotNone(peer.poll())
            fields=Path('/proc',str(child.pid),'stat').read_text().rsplit(')',1)[1].split()
            self.assertEqual(fields[0],'Z')
            child.wait(timeout=5)
        finally:
            if child.returncode is None:
                child.kill()
                child.wait(timeout=5)

class ShellOwnership(unittest.TestCase):
    def test_login_bus_cannot_start_private_shell(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(private_wayland.os, 'getppid', return_value=os.getpid()), patch.object(private_wayland.subprocess, 'Popen') as spawn:
            with self.assertRaisesRegex(RuntimeError, 'owned session bus'):
                with private_wayland.owned_gnome_shell(directory):
                    self.fail('Unexpected desktop access')
            spawn.assert_not_called()

class ImeOwnership(unittest.TestCase):
    def test_direct_ime_entry_cannot_touch_login_configuration(self):
        script=Path(__file__).resolve().parents[1]/'smoke-gnome-ime.py'
        with tempfile.TemporaryDirectory() as directory:
            result=subprocess.run([sys.executable,str(script),'/nonexistent-bundle','--private-root',directory],capture_output=True,text=True,timeout=10)
            self.assertNotEqual(result.returncode,0)
            self.assertIn('owned session bus and private profile',result.stderr)
            self.assertEqual(list(Path(directory).iterdir()),[])

class OrcaOwnership(unittest.TestCase):
    def test_arbitrary_compositor_cannot_start_screen_reader(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError,'live owned compositor'):
                with private_orca.private_orca(directory,object()):
                    self.fail('Unexpected screen reader startup')
            self.assertEqual(list(Path(directory).iterdir()),[])

    def test_login_bus_is_rejected_before_settings_or_processes(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(private_orca,'_input_compositor_pid',return_value=1), patch.object(private_orca.subprocess,'Popen') as spawn:
            with self.assertRaisesRegex(RuntimeError,'owned session bus and private profile'):
                with private_orca.private_orca(directory,object()):
                    self.fail('Unexpected screen reader startup')
            spawn.assert_not_called()
            self.assertEqual(list(Path(directory).iterdir()),[])

class ProviderOwnership(unittest.TestCase):
    def test_real_provider_entries_reject_login_profile_before_writes(self):
        script=Path(__file__).resolve().parents[1]/'smoke-gnome-real-backend.py'
        for entry in ['--private-root','--sandbox-root']:
            with self.subTest(entry=entry),tempfile.TemporaryDirectory() as directory:
                env=dict(os.environ)
                env.pop('STUDIO_REAL_BACKEND_NAMESPACE',None)
                result=subprocess.run([sys.executable,str(script),directory,entry,directory],env=env,capture_output=True,text=True,timeout=10)
                self.assertNotEqual(result.returncode,0)
                self.assertIn('requires its private desktop/profile',result.stderr)
                self.assertEqual(list(Path(directory).iterdir()),[])

    def test_codex_controller_rejects_login_profile_before_writes(self):
        script=Path(__file__).with_name('codex-native-fixture.py')
        with tempfile.TemporaryDirectory() as directory:
            env=dict(os.environ)
            env.pop('STUDIO_REAL_BACKEND_NAMESPACE',None)
            result=subprocess.run([sys.executable,str(script),directory],env=env,capture_output=True,text=True,timeout=10)
            self.assertNotEqual(result.returncode,0)
            self.assertIn('requires its private desktop/profile',result.stderr)
            self.assertEqual(list(Path(directory).iterdir()),[])

class CodexCleanupOwnership(unittest.TestCase):
    def test_unmarked_profile_cannot_stop_a_daemon(self):
        with tempfile.TemporaryDirectory() as directory,patch.object(local_codex_provider.subprocess,'run') as run:
            with self.assertRaises(AssertionError):
                local_codex_provider.stop_private_daemon('codex',directory,{'CODEX_HOME':directory},None)
            run.assert_not_called()

    def test_foreign_home_cannot_stop_a_daemon(self):
        with tempfile.TemporaryDirectory() as directory,patch.object(local_codex_provider.subprocess,'run') as run:
            home=Path(directory)/'owned';home.mkdir();(home/'studio-smoke-fixture').touch()
            other=Path(directory)/'other';other.mkdir()
            with self.assertRaises(AssertionError):
                local_codex_provider.stop_private_daemon('codex',home,{'CODEX_HOME':str(other)},None)
            run.assert_not_called()

    def test_unknown_remaining_process_is_never_signaled(self):
        rows=[dict(pid=123,started='1',executable='/private/codex',roleArguments=['app-server','unknown'])]
        with patch.object(local_codex_provider,'fixture_homes'),patch.object(local_codex_provider,'private_daemon_processes',return_value=rows),patch.object(local_codex_provider.subprocess,'run',return_value=SimpleNamespace(returncode=0)),patch.object(local_codex_provider.time,'monotonic',side_effect=[0,20,20,40]),patch.object(local_codex_provider.os,'pidfd_open') as opened:
            result=local_codex_provider.stop_private_daemon('codex','/private',{},None)
            self.assertFalse(result['ok']);opened.assert_not_called()

    def test_reused_pid_is_rejected_before_signaling(self):
        rows=[dict(pid=123,started='old',executable='/private/codex',roleArguments=['app-server','daemon','pid-update-loop'])]
        with patch.object(local_codex_provider,'fixture_homes'),patch.object(local_codex_provider,'private_daemon_processes',return_value=rows),patch.object(local_codex_provider.subprocess,'run',return_value=SimpleNamespace(returncode=0)),patch.object(local_codex_provider.os,'pidfd_open',return_value=9),patch.object(local_codex_provider,'ProcessPeer',return_value=SimpleNamespace(started='new')),patch.object(local_codex_provider.signal,'pidfd_send_signal') as sent,patch.object(local_codex_provider.os,'close') as closed:
            with self.assertRaises(AssertionError):
                local_codex_provider.stop_private_daemon('codex','/private',{},None)
            sent.assert_not_called();closed.assert_called_once_with(9)

    def test_non_linux_preserves_cli_stop_without_proc_or_pidfds(self):
        with tempfile.TemporaryDirectory() as directory:
            (Path(directory)/'studio-smoke-fixture').touch()
            with patch.object(local_codex_provider.sys,'platform','darwin'),patch.object(local_codex_provider.subprocess,'run',return_value=SimpleNamespace(returncode=0)),patch.object(local_codex_provider,'private_daemon_processes') as inspected:
                result=local_codex_provider.stop_private_daemon('codex',directory,{'CODEX_HOME':directory},None)
                self.assertTrue(result['ok']);self.assertFalse(result['pidHelperCleanupAvailable'])
                inspected.assert_not_called()

class DisplayOwnership(unittest.TestCase):
    def test_display_configuration_rejects_an_arbitrary_compositor(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError,'live owned compositor'):
                private_displays.OwnedDisplays(directory,object())
            self.assertEqual(list(Path(directory).iterdir()),[])

    def test_display_configuration_rejects_login_bus_before_query(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(private_displays,'_input_compositor_pid',return_value=1), patch.object(private_displays.OwnedDisplays,'_bus_call') as bus:
            with self.assertRaisesRegex(RuntimeError,'owned session bus and private profile'):
                private_displays.OwnedDisplays(directory,object())
            bus.assert_not_called()

    def test_private_virtual_monitor_resources_are_bounded(self):
        for monitors in [[],['1600x1000']*3,['9999x9999'],['1600x1000 --replace'],'1600x1000']:
            with self.subTest(monitors=monitors),self.assertRaises(RuntimeError):
                private_wayland._virtual_monitor_args(monitors)
        self.assertEqual(private_wayland._virtual_monitor_args(('1920x1200','2560x1600')),['--virtual-monitor','1920x1200','--virtual-monitor','2560x1600'])

class Teardown(unittest.TestCase):
    def test_only_exact_owned_portal_and_gvfs_mounts_are_detached(self):
        rows=mount(ROOT+'/doc','fuse.portal')+mount(ROOT+'/gvfs','fuse.gvfsd-fuse')+mount('/run/user/1001/gvfs','fuse.gvfsd-fuse')+mount(ROOT+'-other/doc','fuse.portal')
        with patch.object(Path,'read_text',return_value=rows),patch.object(private_wayland.subprocess,'run',return_value=subprocess.CompletedProcess([],0)) as run:
            private_wayland.detach_owned_mounts(ROOT)
            self.assertEqual([c.args[0] for c in run.call_args_list],[['fusermount3','-uz','--',ROOT+'/doc'],['fusermount3','-uz','--',ROOT+'/gvfs']])
    def test_wrong_filesystem_and_non_private_roots_are_rejected(self):
        with patch.object(Path,'read_text',return_value=mount(ROOT+'/doc','ext4')),patch.object(private_wayland.subprocess,'run') as run:
            with self.assertRaises(RuntimeError):private_wayland.detach_owned_mounts(ROOT)
            with self.assertRaises(RuntimeError):private_wayland.detach_owned_mounts('/run/user/1001')
            run.assert_not_called()
    def test_race_is_tolerated_only_after_the_mount_has_disappeared(self):
        rows=mount(ROOT+'/gvfs','fuse.gvfsd-fuse')
        failure=subprocess.CompletedProcess(['fusermount3'],1,stdout='',stderr='already detached')
        with patch.object(Path,'read_text',side_effect=[rows,'']),patch.object(private_wayland.subprocess,'run',return_value=failure):
            private_wayland.detach_owned_mounts(ROOT)
        with patch.object(Path,'read_text',return_value=rows),patch.object(private_wayland.subprocess,'run',return_value=failure):
            with self.assertRaises(subprocess.CalledProcessError):private_wayland.detach_owned_mounts(ROOT)

if __name__=='__main__':unittest.main()
