import argparse,json,os,resource,subprocess,time
from pathlib import Path
parser=argparse.ArgumentParser(description='Compare isolated native math engines on identical inputs');parser.add_argument('binary');parser.add_argument('--output',default=str(Path(__file__).resolve().parents[2]/'target/native-math-benchmark'));args=parser.parse_args()
root=Path(args.output);root.mkdir(parents=True,exist_ok=True);out=root/'images';out.mkdir(exist_ok=True)
binary=str(Path(args.binary).resolve())
cases={
 'fraction':r'\frac{1}{2}',
 'quadratic':r'x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}',
 'integral':r'\int_0^\infty e^{-x^2}\,dx=\frac{\sqrt{\pi}}{2}',
 'sum':r'\sum_{n=1}^{\infty}\frac{1}{n^2}=\frac{\pi^2}{6}',
 'matrix':r'\begin{pmatrix}a&b\\c&d\end{pmatrix}',
 'aligned':r'\begin{aligned}a&=b+c\\d&=e-f\end{aligned}',
 'cases':r'f(x)=\begin{cases}x^2&x\geq 0\\-x&x<0\end{cases}',
 'electronics':r'\eta=\frac{V_{\mathrm{out}}I_{\mathrm{out}}}{V_{\mathrm{in}}I_{\mathrm{in}}}\times100\%',
 'mathbb':r'\forall x\in\mathbb{R},\quad\left|\frac{x}{1+x^2}\right|\leq\frac12',
 'accent':r'\overbrace{a+\cdots+a}^{n\text{ times}}=na',
 'chemistry':r'\ce{2H2 + O2 -> 2H2O}',
 'color':r'\color{red}{x^2}+\color{blue}{y^2}=z^2',
 'tag':r'\begin{equation}E=mc^2\tag{1}\end{equation}',
 'unicode':r'\text{电流}=I_{\mathrm{out}}\quad\alpha+\beta=\gamma',
 'unknown':r'\unsupportedNativeMacro{x}',
 'file':r'\input{/etc/passwd}',
 'command':r'\write18{touch /tmp/native-math-sentinel}',
 'recursive':r'\def\a{\a}\a',
 'deep':'{'*1000+'x'+'}'*1000,
}
def limits():
 resource.setrlimit(resource.RLIMIT_CORE,(0,0));resource.setrlimit(resource.RLIMIT_AS,(512*1024*1024,512*1024*1024));resource.setrlimit(resource.RLIMIT_CPU,(3,3))
rows=[]
for engine in ['ratex','latex']:
 for name,source in cases.items():
  start=time.perf_counter()
  try:
   p=subprocess.run(['/usr/bin/time','-f','native-bench-maxrss:%M',binary,engine,str(out/(engine+'-'+name+'.png'))],input=json.dumps({'source':source}),text=True,capture_output=True,timeout=5,preexec_fn=limits)
   try:r=json.loads(p.stdout)
   except ValueError:r={'ok':False,'error':p.stderr[-500:],'exit':p.returncode}
   rss=next((line.split(':',1)[1] for line in p.stderr.splitlines() if line.startswith('native-bench-maxrss:')),None)
   if rss:r['peakRssKiB']=int(rss)
  except subprocess.TimeoutExpired:r={'ok':False,'error':'process timeout'}
  r.update(engine=engine,case=name,processMs=round((time.perf_counter()-start)*1000,2));rows.append(r)
  print(json.dumps(r),flush=True)
(root/'report.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2))
