#!/usr/bin/env python3
"""Production native inline math, source anchors and label recycling on Wayland."""
import argparse,hashlib,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('inline_output_fixture',Path(__file__).with_name('smoke-gnome-outputs.py'))
outputs=importlib.util.module_from_spec(spec);spec.loader.exec_module(outputs)

def run(native,gateway,root):
    controlled=OwnedMutterInput();controlled.enable_clipboard_reader()
    paged=bool(os.environ.get('STUDIO_GNOME_SMOKE_PAGED_MATH'))
    original={name:hashlib.sha256((root/'project'/name).read_bytes()).hexdigest() for name in ['inline.md','crlf-math.md']+(['paged-math.md','oversize-math.md','dense-math.md'] if paged else [])}
    try:
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([native],cwd=root,env=dict(os.environ),stdout=log,stderr=log);memory=ProcessMemory(child.pid,gateway)
            try:
                end=time.monotonic()+120
                while child.poll() is None:
                    controlled.respond(root)
                    if time.monotonic()>end:raise TimeoutError(('Native inline labels timed out',root))
                    if 'panicked' in (root/'app.log').read_text():raise AssertionError((root,(root/'app.log').read_text()[-2000:]))
                    time.sleep(.01)
            except BaseException:child.terminate();child.wait(timeout=10);raise
            finally:measured=memory.close()
    finally:controlled.close()
    result=json.loads((root/'inline-labels.json').read_text())
    keys=['ok','chatPaging','readerPaging','exactCopy','partialFormulaSearch','sourceComment','themesRetainNonzeroPage','stalePageActionRejected','draftRetained','language','translationSource'] if paged else ['ok','excerptMath','excerptCopyExact','clippedMathLiteral','completeCellCopyExact','continuousTableMath','continuousMathVisible','continuousExcerptCopy','continuousUnmapCleanup','responsiveTable','resizePreservesState','idleLayout','nativePaneDrag','readerHeaderReachable','fullCellFind','cellFindKeyboard','pagedCellMath','pagedCellSelectionExact','pagedCellComment','pagedCellThemes','pagedCellUnmapCleanup','pagedHeaderMath','pagedHeaderComment','crlfTextView','crlfCopyExact','crlfSourceComment','crlfStyles','crlfLink']
    assert child.returncode==0 and all(result.get(key) for key in keys),(root,child.returncode,result,(root/'app.log').read_text()[-2000:])
    if paged:assert result['pages']>=4 and all(result.get(key) for key in ['rapidPageChanges','rebasedLink','boundedFailureReadable']),(root,result)
    if not paged:assert all(result['richParagraph'].get(key) for key in ['outsideBreak','sourceSearch','dynamicCapture','themes','foreignPrimaryPreserved','nativeDragExact','clickSelection','fontSizing','unmapRemap']),(root,result['richParagraph'])
    assert not outputs.runtime.starts(root),'Viewing formulas started a model turn'
    assert {name:hashlib.sha256((root/'project'/name).read_bytes()).hexdigest() for name in original}==original,'Inline viewing changed the Markdown file'
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
    text=(root/'app.log').read_text();assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error','Gtk-WARNING']),(root,text[-2000:])
    print(json.dumps(dict(result,sourceUnchanged=True,backendCleanup=True,memory=measured,artifacts=str(root))))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root');parser.add_argument('--paged-math',action='store_true');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return run(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='inline-labels-',dir=base));outputs.runtime.create_profile(root)
    paragraph=r'Native **中文😀** before $\frac{a}{b}$, after $\sum_{i=0}^{n}x_i$, and a [safe link](nested/note.md).'
    plain=r'Native 中文😀 before $\frac{a}{b}$, after $\sum_{i=0}^{n}x_i$, and a safe link.'
    currency='Currency $5 and $10.';unsupported=r'Unsupported $\unsupportedNativeMacro{x}$ stays readable.'
    source='# Inline math $x^2$\r\n\r\n'+paragraph+'\r\n\r\n'+currency+'\r\n\r\n'+r'Protected `$y$` and escaped \$x.'+'\r\n\r\n'+unsupported+'\r\n\r\n| Expression | Label |\r\n|---|---|\r\n| $x+y$ | Native table |\r\n'
    long_prefix=r'Long 中文😀 $\frac{a}{b}$ '
    long_tail=r' end $\prod_{i=1}^n i$ complete-cell-tail'
    pad='abcdef '*1600
    boundary=4096-4-len((long_prefix+'styled full link ').encode())
    cross=r'$\frac{c}{d}$'
    middle=pad[:boundary]+cross+pad[boundary:]
    long_source=long_prefix+'**styled** [full link](nested/note.md) '+middle+long_tail
    long_plain=long_prefix+'styled full link '+middle+long_tail
    clipped_plain='x'*250+'$xyz^2$ tail'
    excerpt=lambda value:value.encode()[:256].decode(errors='ignore')
    source+='| '+long_source+' | Complete prefix formula |\r\n| '+clipped_plain+' | Clipped formula remains literal |\r\n'
    header_source=r'**Header** $\sqrt{x}$ '+('header '*1300)
    source=source.replace('| Expression | Label |','| Expression | '+header_source+' |')
    (root/'project/inline.md').write_bytes(source.encode())
    (root/'project/nested').mkdir(exist_ok=True);(root/'project/nested/note.md').write_text('# Linked math target\n')
    crlf_source='Before **bold 中文😀** $\\frac{\r\na}{b}$ [safe link](nested/note.md) after.'
    crlf_plain='Before bold 中文😀 $\\frac{\r\na}{b}$ safe link after.'
    (root/'project/crlf-math.md').write_bytes(('# CRLF source\r\n\r\n'+crlf_source+'\r\n').encode())
    (root/'inline-fixture.json').write_text(json.dumps(dict(plain=plain,currency=currency,unsupported=unsupported,excerpt=excerpt(long_plain),clipped=excerpt(clipped_plain),completeCell=long_plain,crlfPlain=crlf_plain)))
    (root/'raw-items.json').write_text(json.dumps([{'id':'inline-assistant','type':'agentMessage','text':source+'\r\n'+crlf_source+'\r\n'}]))
    if args.paged_math:
        large='Long page begin **'+('word '*1632)+'字😀 '+r'$\frac{'+'\r\n'+r'a}{b}$ [after-link](nested/note.md) '+('context '*2500)+r' $x^2$ long-page-tail**'
        (root/'project/paged-math.md').write_bytes(('# Paged math\r\n\r\n'+large+'\r\n').encode())
        oversized=r'$\unsupportedNativeMacro{'+('x+'*6000)+r'x}$'
        (root/'project/oversize-math.md').write_bytes(('# Oversized formula\r\n\r\nstart '+oversized+' end\r\n').encode())
        (root/'project/dense-math.md').write_text('# Dense styles\n\n'+('**word** '*1200)+r'$x^2$'+'\n')
        (root/'raw-items.json').write_text(json.dumps([{'id':'paged-assistant','type':'agentMessage','text':large}]))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    for name,source in zip(['fake-codex.py','fake-opencode.py'],outputs.providers()):path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_OWNED_MUTTER_INPUT='1',STUDIO_GNOME_SMOKE_INLINE_LABELS='1')
    if args.paged_math:env['STUDIO_GNOME_SMOKE_PAGED_MATH']='1'
    else:env.pop('STUDIO_GNOME_SMOKE_PAGED_MATH',None)
    compositor=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
    with isolated_environment(env,headless=True) as private:subprocess.run(['dbus-run-session','--',*compositor,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=150)
if __name__=='__main__':main()
