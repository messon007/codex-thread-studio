#!/usr/bin/env python3
"""Audit actual linked grammar tables; measure copied executables, never inputs.
Linux ELF64/LE analysis only. No private build paths or grammar sources shipped.
"""
import argparse, gzip, hashlib, json, mmap, re, struct, subprocess, tarfile
from pathlib import Path
EXCLUDED = {'cobol','d','hlsl','fsharp','ocaml','idris','odin','c-sharp','perl','ruby','elixir','vb','php'}
class Elf:
    def __init__(self, path):
        self.file = Path(path).open('rb')
        self.data = mmap.mmap(self.file.fileno(), 0, access=mmap.ACCESS_READ)
        if self.data[:6] != b'\x7fELF\x02\x01' or len(self.data) > 512*1024*1024:
            raise ValueError('Expected bounded ELF64 little-endian input')
        header = struct.unpack_from('<16sHHIQQQIHHHHHH', self.data)
        offset, width, count, names = header[6], header[11], header[12], header[13]
        if width != 64 or not 0 < count < 65535 or names >= count or offset+width*count > len(self.data):
            raise ValueError('Unsupported ELF section inventory')
        self.sections = [struct.unpack_from('<IIQQQQIIQQ',self.data,offset+i*width) for i in range(count)]
        self.section_names = self.sections[names]
    def close(self):
        self.data.close(); self.file.close()
    def string(self, table, offset):
        start, end = table[4]+offset, table[4]+table[5]
        if not table[4] <= start < end <= len(self.data): raise ValueError('Invalid ELF string offset')
        finish = self.data.find(b'\0',start,end)
        if finish < 0: raise ValueError('Unterminated ELF string')
        return self.data[start:finish].decode('utf-8',errors='replace')
    def tables(self):
        result=[]
        for section in self.sections:
            if section[1] != 2: continue # SHT_SYMTAB; stripped inputs must fail.
            if section[9] != 24 or section[5]//24 > 2000000: raise ValueError('Invalid symbol inventory')
            strings=self.sections[section[6]]
            for at in range(section[4],section[4]+section[5],24):
                name,info,other,index,value,size=struct.unpack_from('<IBBHQQ',self.data,at)
                if info&15 != 1 or index >= len(self.sections): continue # STT_OBJECT
                if self.string(strings,name) != 'ts_parse_table': continue
                owner=self.sections[index];begin=owner[4]+value-owner[3]
                if size<=0 or size>32*1024*1024 or begin<owner[4] or begin+size>owner[4]+owner[5]: raise ValueError('Invalid parse-table extent')
                digest=hashlib.sha256(memoryview(self.data)[begin:begin+size]).hexdigest()
                result.append((size,digest))
        return result
    def sizes(self):
        return {self.string(self.section_names,s[0]):s[5] for s in self.sections if s[0]}
def tables(path):
    value=Elf(path)
    try:return value.tables()
    finally:value.close()
def sha(path):
    with Path(path).open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()
def main():
    parser=argparse.ArgumentParser();parser.add_argument('full',type=Path);parser.add_argument('reduced',type=Path);parser.add_argument('--build',required=True,type=Path);parser.add_argument('--output',required=True,type=Path);args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=False)
    identities={}
    objects=list(args.build.glob('arborium-*/out/*parser.o'))
    if len(objects)>4096:raise ValueError('Parser object budget exceeded')
    for obj in objects:
        match=re.fullmatch(r'arborium-(.+)-[0-9a-f]+',obj.parent.parent.name)
        if not match:continue
        for key in tables(obj):identities.setdefault(key,set()).add(match[1])
    report={}
    for name,binary in [('full',args.full),('reduced',args.reduced)]:
        linked={}
        for key in tables(binary):
            owners=identities.get(key,set())
            if len(owners)!=1:raise ValueError(f'Ambiguous/unmatched final table: {key}: {owners}')
            owner=next(iter(owners))
            if owner in linked:raise ValueError(f'Duplicate final table: {owner}')
            linked[owner]={'bytes':key[0],'sha256':key[1]}
        if len(linked)!=(110 if name=='full' else 97):raise ValueError(f'Unexpected {name} parser count: {len(linked)}')
        if not {'lean','verilog'}<=linked.keys():raise ValueError('Retained Lean/Verilog missing')
        if name=='reduced' and EXCLUDED&linked.keys():raise ValueError('Excluded grammar linked')
        dest=args.output/(name+'-stripped');subprocess.run(['strip','--strip-all','-o',str(dest),str(binary)],check=True)
        packed=args.output/(name+'.tar.gz')
        with packed.open('wb') as stream,gzip.GzipFile(filename='',fileobj=stream,mode='wb',compresslevel=9,mtime=0) as gz,tarfile.open(fileobj=gz,mode='w|',format=tarfile.USTAR_FORMAT) as archive:
            info=tarfile.TarInfo('marknoria-server');info.size=dest.stat().st_size;info.mode=0o755;info.mtime=0
            with dest.open('rb') as source:archive.addfile(info,source)
        elf=Elf(binary)
        try:sizes=elf.sizes()
        finally:elf.close()
        report[name]={'unstrippedBytes':binary.stat().st_size,'strippedBytes':dest.stat().st_size,'tarGzipBytes':packed.stat().st_size,'strippedSha256':sha(dest),'sections':{key:sizes.get(key,0) for key in ['.text','.rodata']},'parseTableBytes':sum(item['bytes'] for item in linked.values()),'tables':linked}
    if set(report['full']['tables'])-set(report['reduced']['tables'])!=EXCLUDED:raise ValueError('Grammar set differs beyond the requested13')
    if any(value != report['full']['tables'][name] for name,value in report['reduced']['tables'].items()):raise ValueError('Retained parse-table fingerprints changed')
    report['deltaBytes']={key:report['full'][key]-report['reduced'][key] for key in ['strippedBytes','tarGzipBytes','parseTableBytes']}
    (args.output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'ok':True,'full':{k:v for k,v in report['full'].items() if k!='tables'},'reduced':{k:v for k,v in report['reduced'].items() if k!='tables'},'deltaBytes':report['deltaBytes'],'report':str(args.output/'report.json')}))
if __name__=='__main__':main()
