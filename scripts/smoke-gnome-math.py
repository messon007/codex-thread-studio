#!/usr/bin/env python3
"""Private native math worker checks; no desktop, gateway, profile or model calls."""
import argparse,json,os,resource,subprocess,tempfile,time
from pathlib import Path

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');args=parser.parse_args()
    binary=str(Path(args.native).resolve());resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    with tempfile.TemporaryDirectory(prefix='studio-math-worker-',dir='/tmp') as directory:
        root=Path(directory);env=dict(os.environ,XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),DISPLAY='',WAYLAND_DISPLAY='')
        p=subprocess.Popen([binary,'--native-math-worker'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env)
        try:
            end=time.monotonic()+3;limited=False
            while p.poll() is None and time.monotonic()<end:
                limits=Path('/proc',str(p.pid),'limits').read_text()
                rows={tuple(line.split()[:3]):line.split() for line in limits.splitlines() if line.startswith('Max ')}
                data=rows.get(('Max','data','size'),[]);cpu=rows.get(('Max','cpu','time'),[]);core=rows.get(('Max','core','file'),[])
                if len(data)>4 and data[3:5]==['268435456','268435456']:
                    assert cpu[3:5]==['2','2'] and core[4:6]==['0','0'],limits
                    limited=True;break
                time.sleep(.01)
            assert limited,'Math worker did not install its resource limits before reading input'
            output,error=p.communicate(json.dumps({'source':r'\frac{1}{2}','dark':False}).encode(),timeout=5)
            assert p.returncode==0 and b'<path' in output,(p.returncode,error)
        finally:
            if p.poll() is None:p.kill();p.wait(timeout=5)
        def render(source,dark=False,raw=None,**options):
            p=subprocess.run([binary,'--native-math-worker'],input=raw if raw is not None else json.dumps(dict(source=source,dark=dark,**options)).encode(),capture_output=True,env=env,timeout=5)
            assert len(p.stdout)<=2*1024*1024 and len(p.stderr)<=16384
            return p
        formulas=[r'x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}',r'\int_0^\infty e^{-x^2}\,dx=\frac{\sqrt{\pi}}{2}',r'\begin{pmatrix}a&b\\c&d\end{pmatrix}',r'\ce{2H2 + O2 -> 2H2O}']
        for source in formulas:
            p=render(source);assert p.returncode==0 and b'<path' in p.stdout and b'<text' not in p.stdout,(source,p.stderr)
        assert render('x^2').stdout!=render('x^2',True).stdout,'Dark math kept light glyph colors'
        for display in [False,True]:
            p=render(r'\sum_{i=0}^n x_i',inline=True,displayStyle=display)
            assert p.returncode==0 and b'data-studio-math="' in p.stdout and b'<path' in p.stdout,(display,p.stderr)
            values=p.stdout.split(b'data-studio-math="',1)[1].split(b'"',1)[0].split()
            assert len(values)==4 and all(float(v)>0 for v in values),values
        assert render('x',inline='wrong').returncode!=0
        assert render('x',inline=True,displayStyle='wrong').returncode!=0
        for source in [r'\input{/etc/passwd}',r'\write18{echo forbidden}',r'\def\a{\a}\a',r'\includegraphics{file:///etc/passwd}',r'\unsupportedNativeMacro{x}','{'*1000+'x'+'}'*1000,'x'*16385]:
            for inline in [False,True]:
                p=render(source,inline=inline);assert p.returncode!=0 and not p.stdout,(inline,source[:80],p.returncode,p.stdout[:100])
        assert render('',raw=b'{').returncode!=0
        assert render('',raw=b'x'*65537).returncode!=0
        p=render(r'\href{javascript:alert(1)}{x}');assert p.returncode==0 and b'javascript:' not in p.stdout and b'<a ' not in p.stdout
        assert not any(root.iterdir()),'Rendering started a profile, browser or gateway'
        print(json.dumps({'ok':True,'headlessWorker':True,'resourceLimits':True,'outlinedGlyphs':True,'themeColors':True,'badInputsRejected':True,'noAmbientLink':True,'noUserProfile':True,'inlineMetrics':True,'textAndDisplayStyles':True}))

if __name__=='__main__':main()
