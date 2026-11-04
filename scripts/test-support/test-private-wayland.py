"""Mount teardown must never detach the user's desktop services."""
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch
import private_wayland

ROOT='/tmp/studio-wayland-private-test'
def mount(path,kind):return f'1 0 0:1 / {path} rw - {kind} private rw\n'
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
