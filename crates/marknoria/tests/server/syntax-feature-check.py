#!/usr/bin/env python3
"""Resolved dependency/profile contract; no CI or downstream checkout required."""
import json,re,subprocess
EXCLUDED={'arborium-'+name for name in ['cobol','d','hlsl','fsharp','ocaml','idris','odin','c-sharp','perl','ruby','elixir','vb','php']}
def graph(profile):
    command=['cargo','tree','--locked','--offline','-p','marknoria','--no-default-features','--features','web,syntax-'+profile,'--prefix','none','-e','normal']
    result=subprocess.run(command,text=True,capture_output=True,check=True,timeout=120)
    return set(re.findall(r'^(arborium-[\w-]+) v2\.18\.2\b',result.stdout,re.M))
full,reduced=graph('full'),graph('reduced')
assert len(full)==110 and len(reduced)==97,(len(full),len(reduced))
assert full-reduced==EXCLUDED and not EXCLUDED&reduced
assert {'arborium-lean','arborium-verilog'}<=reduced
# Another dependency enabling full is an explicit build error, not a silent union.
conflict=subprocess.run(['cargo','check','--locked','--offline','-p','marknoria-syntax','--features','reduced'],text=True,capture_output=True,timeout=180)
assert conflict.returncode!=0 and 'syntax-reduced conflicts with full/optional syntax features' in conflict.stderr,conflict.stderr
print(json.dumps({'ok':True,'full':len(full),'reduced':len(reduced),'excluded':sorted(EXCLUDED),'unificationRejected':True}))
