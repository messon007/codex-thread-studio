#!/usr/bin/env python3
"""Isolated CPU/raster comparison. Output contains private diagram content.

No renderer replacement: exact production Pele code and pinned Merman are probed
in fresh bounded subprocesses. Measurements include cold and warm raster work.
"""
import argparse,hashlib,json,os,resource,signal,statistics,subprocess,time
from pathlib import Path
from xml.etree import ElementTree

def cases():
    rows=[('flowchart','flowchart LR\nA[Native 中文] --> B[Preview]'),
          ('sequence','sequenceDiagram\nAlice->>Bob: Hello 中文'),
          ('class','classDiagram\nAnimal <|-- Duck\nAnimal: +String name'),
          ('state','stateDiagram-v2\n[*] --> Idle\nIdle --> Running'),
          ('er','erDiagram\nCUSTOMER ||--o{ ORDER : places'),
          ('pie','pie title Results\n"Pass": 8\n"Fail": 2'),
          ('mindmap','mindmap\n  root((Native))\n    Reader\n    Editor'),
          ('gantt','gantt\n  dateFormat YYYY-MM-DD\n  section Native\n  Reader :2026-10-24, 2d'),
          ('git','gitGraph\ncommit\nbranch native\ncheckout native\ncommit'),
          ('journey','journey\n  title Native migration\n  section Reader\n  Open file: 5: User'),
          ('timeline','timeline\n  title Native\n  2026 : Reader : Editor'),
          ('xychart','xychart-beta\n  x-axis [A, B, C]\n  y-axis 0 --> 10\n  bar [3, 7, 5]'),
          ('quadrant','quadrantChart\n  title Coverage\n  x-axis Low --> High\n  y-axis Low --> High\n  Reader: [0.7, 0.8]'),
          ('block','block-beta\n  columns 2\n  A B'),
          ('packet','packet-beta\n  0-7: "Header"\n  8-15: "Body"'),
          ('sankey','sankey-beta\n\nSource,Target,10'),
          ('math-flow','flowchart LR\nA["$$x^2$$"] --> B["$$\\frac{a}{b}$$"]'),
          ('math-sequence','sequenceDiagram\nparticipant A as $$x^2$$\nA->>B: $$\\frac{a}{b}$$'),
          ('math-message','sequenceDiagram\nAlice->>Bob: $$\\frac{a}{b}$$'),
          ('math-participant','sequenceDiagram\nparticipant A as $$x^2$$\nA->>B: check'),
          ('math-actor','sequenceDiagram\nactor A as $$x^2$$\nparticipant B as $$\\frac{a}{b}$$\nA->>B: $$\\sqrt{x}$$'),
          ('math-mixed','sequenceDiagram\nparticipant A as Input $$x^2$$\nparticipant B as Output $$\\frac{a}{b}$$\nA->>B: result $$x$$ 中文'),
          ('math-actor-dark','%%{init: {"theme":"dark","look":"classic"}}%%\nsequenceDiagram\nparticipant A as $$\\frac{a}{b}$$\nA->>B: $$x^2$$'),
          ('math-multiline','sequenceDiagram\nparticipant A as $$x^2$$<br/>$$y^2$$\nA->>B: $$\\sqrt{x}$$'),
          ('math-flow-mixed','flowchart LR\nA["输入 $$x^2$$"] --> B["输出 $$\\frac{a}{b}$$"]'),
          ('math-flow-multiline','flowchart LR\nA["$$x^2$$<br/>$$y^2$$"] --> B["$$\\sqrt{x}$$"]'),
          ('math-note-loop','sequenceDiagram\nparticipant A as $$x^2$$\nnote over A: $$\\frac{a}{b}$$\nloop $$x^2$$\nA->>B: $$\\sqrt{x}$$\nend'),
          ('math-unsupported-actor','sequenceDiagram\nparticipant A as $$\\unknownNativeMacro{x}$$\nA->>B: literal fallback'),
          ('long-label','flowchart TB\nA["验证中文内容与English mixed labels<br/>Second line with punctuation: (x), y / z"] --> B["Done"]'),
          ('malformed','flowchart LR\nA[unclosed')]
    graph='flowchart TB\nsubgraph Native\nA[Source] --> B{Validated?}\nB -->|yes| C[Preview 中文]\nB -->|no| D[Error]\nend\nC --> E[End]'
    for name,config in [('classic',{'theme':'default','look':'classic'}),('neo',{'theme':'neo','look':'neo'}),
                        ('document',{'theme':'neutral','look':'classic'}),('hand-drawn',{'theme':'neutral','look':'handDrawn','handDrawnSeed':1}),
                        ('dark',{'theme':'dark','look':'classic'}),('neo-dark',{'theme':'neo-dark','look':'neo'}),
                        ('elk',{'layout':'elk','theme':'neo','look':'neo'})]:
        rows.append((name,'%%{init: '+json.dumps(config)+'}%%\n'+graph))
    crossing='flowchart TB\nA-->B & C\nB-->D & E\nC-->D & E\nD-->F\nE-->F'
    rows.extend([('crossing-dagre','%%{init: {"layout":"dagre"}}%%\n'+crossing),
                 ('crossing-elk','%%{init: {"layout":"elk"}}%%\n'+crossing)])
    dense='flowchart TB\n'+'\n'.join(f'N{i}[Stage {i} 中文] --> N{i+5}' for i in range(75))
    dense+='\n'+'\n'.join(f'N{i} --> N{i+6}'for i in range(0,70,5))+'\nN78 --> N12'
    rows.extend([('large-dagre','%%{init: {"layout":"dagre"}}%%\n'+dense),('large-elk','%%{init: {"layout":"elk"}}%%\n'+dense)])
    return [dict(id=name,source=source,dark=name in ['dark','neo-dark','math-actor-dark'])for name,source in rows]

def references(paths):
    rows=[];fingerprints={}
    for file in paths:
        path=Path(file).resolve();body=path.read_bytes();assert len(body)<=5*1024*1024,path
        fingerprints[str(path)]=hashlib.sha256(body).hexdigest()
        opened=None;lines=[];count=0
        for number,line in enumerate(body.decode().splitlines(),1):
            if opened is None and line.strip().startswith('```mermaid'):
                opened=number;lines=[]
            elif opened is not None:
                if line.strip()=='```':
                    rows.append(dict(id=f'reference-{len(fingerprints)}-{count}',source='\n'.join(lines),path=str(path),line=opened+1,dark=False))
                    opened=None;count+=1
                else:lines.append(line)
    return rows,fingerprints

def paint_cases():
    """Additional palette/author-color cases for the pinned downstream patch."""
    rows=[
      ('paint-role-palette',{'theme':'base','look':'classic','themeVariables':{'actorTextColor':'#00ff00','signalTextColor':'#0000ff','noteTextColor':'#ff00ff','loopTextColor':'#00ffff'}},
       'sequenceDiagram\nparticipant A as Input $$x^2$$\nnote over A: $$x^2$$\nloop $$x^2$$\nA->>B: $$x^2$$\nend',False),
      ('paint-tex-colors-dark',{'theme':'dark','look':'classic'},
       r'sequenceDiagram'+'\n'+r'participant A as Input $$x+\color{red}{y}+\color{black}{z}$$'+'\n'+r'A->>B: $$x^2$$',True),
      ('paint-flow-class',{'theme':'dark','look':'classic'},
       'flowchart LR\nA["$$x^2$$"]:::custom --> B["$$y^2$$"]\nclassDef custom color:#00ff00;',True),
      ('paint-alpha',{'theme':'base','look':'classic','themeVariables':{'actorTextColor':'rgba(0,255,0,0.5)'}},
       'sequenceDiagram\nparticipant A as Input $$x^2$$\nA->>B: done',False),
      ('paint-class-dark',{'theme':'dark','look':'classic'},
       'classDiagram\nclass A["$$x^2$$"]\nA : +$$y^2$$',True),
      ('paint-flow-authored-black',{'theme':'dark','look':'classic'},
       'flowchart LR\nA["$$\\color{black}{x}$$"]:::custom --> B["$$y^2$$"]\nclassDef custom color:#00ff00;',True),
      ('paint-neo-dark',{'theme':'neo-dark','look':'neo'},
       'sequenceDiagram\nparticipant A as Input $$x^2$$\nparticipant B as Output $$y^2$$\nA->>B: $$x^2$$',True),
      ('paint-hand-drawn',{'theme':'neutral','look':'handDrawn','handDrawnSeed':1},
       'flowchart LR\nA["Input $$x^2$$"] --> B["$$\\frac{a}{b}$$"]',False),
      ('paint-edge-style',{'theme':'dark','look':'classic'},
       'flowchart LR\nA -->|"$$x^2$$"| B\nlinkStyle 0 color:#0000ff',True),
    ]
    return [dict(id=name,source='%%{init: '+json.dumps(config)+'}%%\n'+source,dark=dark)for name,config,source,dark in rows]

def limits():
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    resource.setrlimit(resource.RLIMIT_AS,(2*1024**3,2*1024**3))
    resource.setrlimit(resource.RLIMIT_CPU,(12,12))
    resource.setrlimit(resource.RLIMIT_FSIZE,(8*1024**2,8*1024**2))
    resource.setrlimit(resource.RLIMIT_NOFILE,(128,128))

def inspect_svg(path):
    """Structural checks complement raster inspection; parse success is not parity."""
    root=ElementTree.parse(path).getroot()
    return dict(
        nativeMathElements=sum('merman-native-math-svg' in x.attrib.get('class','').split()for x in root.iter()),
        literalMathTextElements=sum('$$'in ''.join(x.itertext())for x in root.iter()if x.tag.split('}')[-1]=='text'),
        paths=sum(x.tag.split('}')[-1]=='path'for x in root.iter()),
        foreignObjects=sum(x.tag.split('}')[-1]=='foreignObject'for x in root.iter()))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--merman',required=True);p.add_argument('--pele',required=True)
    p.add_argument('--output',required=True);p.add_argument('--reference',action='append',default=[])
    p.add_argument('--native-fonts',action='store_true',help='Probe Merman native system-font measurement (DejaVu Sans)')
    p.add_argument('--font',action='append',default=[],help='Explicit measurement font file; avoids whole-system discovery')
    p.add_argument('--prefer-math-switches',action='store_true',help='Preserve generated actor formula branches before native SVG lowering')
    p.add_argument('--outline-text',action='store_true',help='Normalize glyphs with measurement fonts before the display rasterizer')
    p.add_argument('--paint-cases',action='store_true',help='Include role/author-color fixtures for the downstream paint patch')
    a=p.parse_args()
    out=Path(a.output).resolve();out.mkdir(parents=True,exist_ok=False)
    extra,originals=references(a.reference);corpus=cases()+(paint_cases()if a.paint_cases else [])+extra;results=[]
    metadata=dict(executables={name:dict(path=str(Path(exe).resolve()),sha256=hashlib.sha256(Path(exe).read_bytes()).hexdigest())
                              for name,exe in [('merman',a.merman),('pele',a.pele)]},
                  host=dict(zip(['sysname','nodename','release','version','machine'],os.uname())),iterations=3,concurrentChildren=1,
                  limits=dict(addressSpaceBytes=2*1024**3,cpuSeconds=12,wallSeconds=18,fileBytes=8*1024**2),
                  harnessSha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    metadata['nativeFontMeasurement']=a.native_fonts
    metadata['preferMathSwitches']=a.prefer_math_switches
    metadata['outlinedText']=a.outline_text
    metadata['paintCases']=a.paint_cases
    metadata['fontFiles']={str(Path(file).resolve()):hashlib.sha256(Path(file).read_bytes()).hexdigest()for file in a.font}
    for case in corpus:
        prefix=out/case['id'];prefix.mkdir();(prefix/'source.mmd').write_text(case['source'])
        row={k:v for k,v in case.items()if k!='source'};row['sourceBytes']=len(case['source'].encode());row['engines']={}
        for name,executable in [('merman',a.merman),('pele',a.pele)]:
            request=dict(source=case['source'],dark=case['dark'],nativeFonts=a.native_fonts,fontFiles=list(metadata['fontFiles']),preferMathSwitches=a.prefer_math_switches,outlineText=a.outline_text,iterations=3,pngPath=str(prefix/(name+'.png')),svgPath=str(prefix/(name+'.svg')),layoutSvgPath=str(prefix/(name+'.layout.svg')))
            command=['/usr/bin/time','-f','%M','-o',str(prefix/(name+'-rss.txt')),str(Path(executable).resolve())]
            started=time.monotonic()
            with subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
                                  text=True,start_new_session=True,preexec_fn=limits)as child:
                try:
                    stdout,stderr=child.communicate(json.dumps(request)+'\n',timeout=18)
                    value=json.loads(stdout)if child.returncode==0 else dict(ok=False,error='exit '+str(child.returncode))
                except subprocess.TimeoutExpired:
                    # Kill the owned session too: /usr/bin/time can have a renderer
                    # and Pele's native math child below it. Never leave orphans.
                    try:os.killpg(child.pid,signal.SIGKILL)
                    except ProcessLookupError:pass
                    stdout,stderr=child.communicate()
                    value=dict(ok=False,error='18-second outer deadline')
                (prefix/(name+'-stderr.txt')).write_text(stderr)
            value['processMs']=round((time.monotonic()-started)*1000,3)
            rss=prefix/(name+'-rss.txt')
            if rss.exists():
                text=rss.read_text().strip().splitlines()
                if text and text[-1].isdigit():value['peakRssKiB']=int(text[-1])
            if value.get('ok'):
                times=value['totalUs'];value['coldTotalUs']=times[0];value['warmMedianUs']=statistics.median(times[1:])
                svg=prefix/(name+'.svg')
                if svg.exists():value['inspection']=inspect_svg(svg)
                layout_svg=prefix/(name+'.layout.svg')
                if layout_svg.exists():value['layoutInspection']=inspect_svg(layout_svg)
            row['engines'][name]=value
        results.append(row)
        (out/'results.json').write_text(json.dumps(dict(cases=results,references=originals,metadata=metadata),ensure_ascii=False,indent=2))
        print(json.dumps(dict(case=case['id'],merman=row['engines']['merman']['ok'],pele=row['engines']['pele']['ok'])),flush=True)
    for path,expected in originals.items():assert hashlib.sha256(Path(path).read_bytes()).hexdigest()==expected,path
    summary=dict(cases=len(results),success={name:sum(r['engines'][name]['ok']for r in results)for name in ['merman','pele']},
        references=len(extra),productionRendererChanged=False,applicationMsrvChanged=False,output=str(out))
    summary['mermanLiteralMathCases']=[r['id']for r in results if r['engines']['merman'].get('layoutInspection',r['engines']['merman'].get('inspection',{})).get('literalMathTextElements')]
    (out/'summary.json').write_text(json.dumps(summary,indent=2));print(json.dumps(summary))
if __name__=='__main__':main()
