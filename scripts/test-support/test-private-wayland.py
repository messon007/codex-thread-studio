"""Mount teardown must never detach the user's desktop services."""
from pathlib import Path
import subprocess
import unittest
import sys
import os
import tempfile
from unittest.mock import patch
import private_wayland

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
