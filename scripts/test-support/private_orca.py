"""Real Orca with a private, silent Speech Dispatcher generic module.

Observe generated announcements, not audible voice quality. Never replace the
login screen reader; Orca's global process-name check sees only its PID namespace.
"""
from contextlib import contextmanager
from pathlib import Path
import os
import subprocess
import time
from private_wayland import _input_compositor_pid


@contextmanager
def private_orca(root, compositor):
    root=Path(root).resolve(strict=True)
    _input_compositor_pid(compositor)
    if (Path('/proc',str(os.getppid()),'exe').resolve()!=Path('/usr/bin/dbus-run-session').resolve()
            or os.environ.get('XDG_CONFIG_HOME')!=str(root/'config')):
        raise RuntimeError('Orca acceptance requires the owned session bus and private profile')
    from gi.repository import Gio
    base=root/'assistive';base.mkdir()
    config=base/'speech-config';config.mkdir()
    logs=base/'speech-logs';logs.mkdir()
    home=base/'home';home.mkdir()
    runtime=Path(os.environ['XDG_RUNTIME_DIR'])
    socket=runtime/'speechd.sock'
    modules=config/'modules';modules.mkdir()
    (modules/'silent.conf').write_text('GenericExecuteSynth "/usr/bin/true"\nGenericLanguage "en" "en"\nGenericLanguage "C" "en"\nGenericDefaultCharset "utf-8"\nAddVoice "en" "MALE1" "silent"\nAddVoice "C" "MALE1" "silent"\n')
    (config/'speechd.conf').write_text('LogLevel 3\nAudioOutputMethod "alsa"\nAudioALSADevice "null"\nAddModule "silent" "sd_generic" "silent.conf"\nDefaultModule "silent"\n')
    for schema,path,key,value in [('org.gnome.Orca.Sound','sound','enabled',False),('org.gnome.Orca.Braille','braille','enabled',False)]:
        settings=Gio.Settings.new_with_path(schema,'/org/gnome/orca/default/'+path+'/')
        assert settings.set_boolean(key,value)
    settings=Gio.Settings.new_with_path('org.gnome.Orca.Speech','/org/gnome/orca/default/speech/')
    assert settings.set_string('synthesizer','silent')
    Gio.Settings.sync()
    env=dict(os.environ,HOME=str(home),SPEECHD_ADDRESS='unix_socket:'+str(socket),SPEECHD_CMD='/bin/false',
        PULSE_SERVER='unix:'+str(runtime/'no-pulse'),PIPEWIRE_REMOTE='no-pipewire',ALSA_CONFIG_PATH='/usr/share/alsa/alsa.conf')
    speech_log=(base/'speech.log').open('w')
    sandbox=['bwrap','--die-with-parent','--unshare-pid','--unshare-net','--ro-bind','/','/',
        '--proc','/proc','--dev','/dev','--tmpfs','/dev/shm','--bind',str(root),str(root),'--bind',str(runtime),str(runtime),'--']
    speech=subprocess.Popen([*sandbox,'/usr/bin/speech-dispatcher','-s','-c','unix_socket','-S',str(socket),
        '-C',str(config),'-L',str(logs),'-P',str(base/'speech.pid'),'-t','0'],env=env,stdout=speech_log,stderr=speech_log)
    orca=None
    output=(base/'orca.log').open('w')
    debug=base/'orca-debug.log'
    def wait(name,test):
        end=time.monotonic()+30
        while time.monotonic()<end:
            if speech.poll() is not None:raise RuntimeError(('Private Speech Dispatcher exited',speech.returncode,str(base)))
            if orca is not None and orca.poll() is not None:raise RuntimeError(('Private Orca exited',orca.returncode,str(base)))
            if test():return
            time.sleep(.05)
        raise RuntimeError(('Private assistive startup timed out',name,str(base)))
    try:
        wait('private speech socket',socket.is_socket)
        import speechd
        peer=speechd.SSIPClient('native-orca-acceptance',autospawn=False,address=env['SPEECHD_ADDRESS'])
        try:
            assert 'silent' in peer.list_output_modules(), 'Silent speech module did not initialize'
            peer.set_output_module('silent')
        finally:
            peer.close()
        command=[*sandbox,'/usr/bin/orca','--debug-file',str(debug)]
        orca=subprocess.Popen(command,env=env,stdout=output,stderr=output)
        wait('actual Orca event loop',lambda:debug.is_file() and 'Starting Atspi main event loop' in debug.read_text())
        yield debug
    finally:
        for process in [orca,speech]:
            if process is None:continue
            if process.poll() is None:process.terminate()
            try:process.wait(timeout=10)
            except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
        output.close();speech_log.close()
