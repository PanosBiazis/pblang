"""Release-only JSON number/grammar oracle and bounded acceptance timings."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import random
import re
import struct
import subprocess
import tempfile
import time
import tomllib


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--candidate', type=Path, required=True)
    ap.add_argument('--report', type=Path, required=True)
    ap.add_argument('--group', choices=['numbers', 'diagnostics', 'scale'], required=True)
    ap.add_argument('--start', type=int, default=0)
    ap.add_argument('--count', type=int, default=10)
    a = ap.parse_args()
    release = a.candidate.resolve()
    rows = []
    with tempfile.TemporaryDirectory(prefix='pb-json-numbers-') as name:
        root = Path(name)
        manifest = '[package]\nname="json-numbers"\nversion="1.0.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path=' + json.dumps(str(release/'stdlib')) + '}\n'
        (root/'pblang.toml').write_text(manifest)
        dep = tomllib.loads((release/'pblang.lock').read_text())['dependencies']['stdlib']
        (root/'pblang.lock').write_text('manifest_sha256 = "'+hashlib.sha256(manifest.encode()).hexdigest()+'"\ncontent_hash_algorithm = "pblang-package-tree-v2"\n[dependencies.stdlib]\npath = '+json.dumps(str(release/'stdlib'))+'\nsha256 = '+json.dumps(dep['sha256'])+'\n')
        box = ['bwrap','--unshare-all','--die-with-parent']
        for d in ['/usr','/bin','/lib','/lib64']:box += ['--ro-bind',d,d]
        box += ['--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(release),str(release),'--bind',name,name,'--chdir',name]
        denies = ['--deny-'+p for p in ['net','http','exec','ffi','spawn','gfx','sqlite']]
        def run(source, document=None):
            (root/'main.sapp').write_text(source)
            if document is not None:(root/'input.json').write_text(document)
            start = time.monotonic()
            try:
                p = subprocess.run(box+['/usr/bin/time','-f','%M','-o',str(root/'peak-rss-kib'),str(release/'bin/pblang'),'run',str(root/'main.sapp'),*denies],capture_output=True,text=True,timeout=30,env=os.environ|{'PBLANG_CHECK_PROFILE':'','PBLANG_SCRIPT_ARGS':''})
                errors = re.findall(r'^error\[.*?\]: (.*)$',p.stderr,re.M)
                return dict(exit=p.returncode,stdout=p.stdout,error=errors[0] if errors else None,seconds=time.monotonic()-start,max_rss_kib=int((root/'peak-rss-kib').read_text().splitlines()[-1]))
            except subprocess.TimeoutExpired:
                return dict(exit='timeout',stdout='',error='30 second acceptance deadline',seconds=time.monotonic()-start)
        def record(label, passed, **data):
            rows.append(dict(label=label,passed=passed,**data));a.report.write_text(json.dumps(rows,indent=2,ensure_ascii=False));print(label,passed,flush=True);assert passed,rows[-1]
        prefix='let D=import("stdlib:json_decode.sapp");let J=import("stdlib:json.sapp");'
        if a.group=='numbers':
            cases=['0','-0','0.0','-0.0','0e999','-0e-999','0.1','0.10000000000000001','0.10000000000000002','0.1000000000000000055511151231257827021181583404541015625','1.2345678901234567','1.7976931348623157e308','2.2250738585072014e-308','2.2250738585072011e-308','5e-324','2e-324','3e-324','1e-400','-1e-400']
            for n in [4503599627370495,4503599627370496,9007199254740990,9007199254740991,9007199254740992,9007199254740993,9007199254740994]:
                for sign in ['', '-']:
                    cases += [sign+str(n),sign+str(n)+'.0',sign+str(n)+'.00',sign+str(n)+'e0',sign+str(n*10)+'e-1']
            safe_rng=random.Random(110021)
            cases += ['0','1','-1','999999999999999','-999999999999999','1000000000000000']
            cases += [str(safe_rng.randint(-999999999999999,999999999999999)) for _ in range(100)]
            rng=random.Random(110020)
            while len(cases)<512:
                f=struct.unpack('<d',rng.getrandbits(64).to_bytes(8,'little'))[0]
                if math.isfinite(f):cases.append(repr(f))
            fixtures='['+','.join('map{text:'+json.dumps(t)+',bytes:'+json.dumps(list(struct.pack('<d',float(t))))+'}' for t in cases)+']'
            source=prefix+'let cases='+fixtures+';for c in cases {let v=D.parse(c.text);assert(num_to_le_bytes(v)==c.bytes,"oracle "+c.text);assert(num_to_le_bytes(J.parse(J.stringify(v)))==c.bytes,"roundtrip "+c.text);}print "json_oracle_pass";'
            result=run(source);record('512 decimal/exponent/binary64 byte oracles and 512 roundtrips',result['exit']==0 and 'json_oracle_pass' in result['stdout'],count=len(cases),result=result)
        elif a.group=='diagnostics':
            cases=['',' ','[','{','[1,]','{"a":1,}','01','-01','+1','.1','1.','1e','1e+','--1','NaN','Infinity','1e309','-1e309','{"n":1e999}','[0,\n1e999]','{1:2}','{-1.2e2:0}','{"a":0,2:1}','{"1":2,1:3}','{"a" 1}','true false','[1 2]','"\\uD800"','"\\udc00"','"\\x20"','"a\nb"','"unterminated','"bad\\"','['*128+'0'+']'*128,'{"n":1e999,}','{"x":01}','1e9999x','{"a":0} trailing','[null, false,, 2]','["x",+2]','{"x":1e9999,"x":1}','{"x":1,"x":1e9999}','{"x":-1e9999,"x":1}']
            inputs=[json.dumps(raw) for raw in cases]+['nil','true','7','[]','map{}','bigint("1")']
            for i, arg in enumerate(inputs[a.start:a.start+a.count],a.start):
                expected=run('fn call(x){return json_parse(x);}call('+arg+');')
                assert expected['exit']!=0 and expected['error'] is not None,(i,arg,expected)
                actual=run(prefix+'fn call(x){return D.parse(x);}call('+arg+');')
                equal=all(actual[k]==expected[k] for k in ['exit','stdout','error'])
                record('original diagnostic '+str(i),equal,expected=expected,actual=actual)
        else:
            size=2*1024*1024
            count=(size-2)//2
            cases=[('2MiB string','"'+'x'*(size-2)+'"','assert(len(v)==2097150,"string size");'),
                   ('2MiB dense numeric array','['+'0,'*(count-1)+'0]','assert(len(v)==1048575 and v[0]==0 and v[1048574]==0,"dense array");'),
                   ('2MiB whitespace',' '* (size-1)+'0','assert(v==0,"whitespace");'),
                   ('2MiB decimal boundary array','['+','.join(['9007199254740991.0']*110376)+']','assert(len(v)==110376 and v[0]==parse_number("9007199254740991"),"boundaries");'),
                   ('65536 mixed numeric cells','['+','.join(['0.1','9007199254740991.0','-1e-400','0']*16384)+']','assert(len(v)==65536 and v[1]==parse_number("9007199254740991") and num_to_le_bytes(v[2])==[0,0,0,0,0,0,0,128],"mixed cells");'),
                   ('126 array levels','['*126+'9007199254740991.0'+']'*126,'let depth=0;while is_array(v) {v=v[0];depth=depth+1;}assert(depth==126 and v==parse_number("9007199254740991"),"near depth limit");'),
                   ('1.8MiB escaped Unicode','"'+('\\u03b1\\ud83d\\ude00'*100000)+'"','assert(len(v)==200000 and str_slice(v,0,2)=="α😀","escaped Unicode");')]
            for label,doc,check in cases[a.start:a.start+a.count]:
                assert len(doc.encode())<=size
                result=run(prefix+'let v=D.parse(fs_read("input.json"));'+check+'print "json_scale_pass";',doc)
                record(label,result['exit']==0 and 'json_scale_pass' in result['stdout'] and result['seconds']<30 and result['max_rss_kib']<512*1024,input_bytes=len(doc.encode()),result=result)

if __name__=='__main__':main()
