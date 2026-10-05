"""Opt-in preflight; explicit evidence that retained source execution stays unchanged."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--candidate', type=Path, required=True)
    ap.add_argument('--report', type=Path, required=True)
    ap.add_argument('--group', choices=['source','argv','permissions','parity'], required=True)
    a = ap.parse_args(); release = a.candidate.resolve(); rows = []
    with tempfile.TemporaryDirectory(prefix='pb-retained-profile-') as temp:
        root = Path(temp)
        colon = 'for i 1 {let p=map{run:fn(self){return 7;}};print (try {p:run()} catch 9);}'
        fixtures = {
            'colon.sapp': colon,
            'explicit.sapp': colon.replace('p:run()', 'p.run(p)'),
            'valid.sapp': 'fn add(x){return x+1;}print add(6);',
            'plain-method.sapp': 'let p=map{run:fn(self){return 7;}};print p:run();',
            'comment.sapp': '// for i 1 { try p:run() catch 9; } import\nprint 7;',
            'string.sapp': 'print "try p:run() catch 9 import";',
            'effect.sapp': 'fs_write("must-not-execute", "PRIVATE_CANARY");',
            'semantic.sapp': 'print unknown_variable;',
            'type.sapp': 'let n:number="wrong";',
            'import.sapp': 'let m=import("PRIVATE_CANARY");',
            'alias.sapp': 'let m=import;',
            'empty.sapp': '',
            'broken.sapp': 'let = ;',
        }
        for name,source in fixtures.items(): (root/name).write_text(source)
        box = ['bwrap','--unshare-all','--die-with-parent']
        for d in ['/usr','/bin','/lib','/lib64']:box += ['--ro-bind',d,d]
        box += ['--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(release),str(release),
                '--bind',temp,temp,'--chdir',temp]
        profile = ['--check','--via-pblang','--retained-source']

        def check(name,args,code=0,text='self-hosted check ok',native=False,env=None):
            before={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in root.glob('*.sapp')}
            binary=release/('bin/pblang-native' if native else 'bin/pblang')
            p=subprocess.run(box+[str(binary),*map(str,args)],capture_output=True,text=True,
                             timeout=15,env=os.environ|(env or {}))
            after={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in root.glob('*.sapp')}
            good=(p.returncode==code if code is not None else p.returncode!=0) and text in p.stdout+p.stderr
            good=good and before==after and not (root/'must-not-execute').exists()
            row=dict(name=name,passed=good,exit=p.returncode,stdout=p.stdout,stderr=p.stderr,source_preserved=before==after)
            rows.append(row);a.report.write_text(json.dumps(rows,indent=2,ensure_ascii=False));print(name,good,flush=True);assert good,row
            return p

        if a.group=='source':
            for name in ['valid.sapp','plain-method.sapp','comment.sapp','string.sapp','effect.sapp']:
                check(name,profile+[name])
            for name in ['colon.sapp','explicit.sapp']:
                p=check(name,profile+[name],1,'retained-nested-try')
                assert 'source was not executed' in p.stdout and 'PBCK' in p.stdout
            for name in ['import.sapp','alias.sapp']:
                p=check(name,profile+[name],1,'retained-unchecked-import');assert 'PRIVATE_CANARY' not in p.stdout+p.stderr
            for name in ['semantic.sapp','type.sapp']:
                check(name,profile+[name],1,'self-hosted check failed')
            for name in ['empty.sapp','broken.sapp']:
                check(name,profile+[name],1,'retained-invalid-source')
            check('missing',profile+['missing.sapp'],1,'failed to read source file')
            # Caller modules/manifests must not replace the installed checker.
            (root/'stdlib/compiler').mkdir(parents=True)
            (root/'stdlib/compiler/retained_source_check.sapp').write_text('print "DECOY_EXECUTED";fn check(s){return map{ok:true};}export check;')
            (root/'pblang.toml').write_text('[package]\nname="decoy"\nversion="0.1.0"\nentry="valid.sapp"\n[dependencies]\nstdlib={path="stdlib"}\n')
            (root/'pblang.lock').write_text('intentionally invalid caller lock')
            p=check('trusted installed origin',profile+['colon.sapp'],1,'retained-nested-try');assert 'DECOY_EXECUTED' not in p.stdout+p.stderr
        elif a.group=='argv':
            for args in [
                ['--self-host','--retained-source','--check','valid.sapp'],
                ['--deny-net','--check','valid.sapp','--retained-source','--via-pblang'],
                ['--check','--via-pblang','--self-host','--retained-source','valid.sapp'],
            ]:check('supported order '+str(args),args)
            for name in ['δύο λέξεις.sapp','line\nnext.sapp','--retained-source']:
                (root/name).write_text(fixtures['valid.sapp']);check('literal '+repr(name),profile+['--',name])
            for args in [
                ['--retained-source','--check','--via-pblang','valid.sapp'],
                ['--retained-source','valid.sapp'],['--check','--retained-source','valid.sapp'],
                ['--via-pblang','--retained-source','valid.sapp'],profile+['--retained-source','valid.sapp'],
                profile+['valid.sapp','other.sapp'],profile+['--unknown','valid.sapp'],profile+[''],profile,
            ]:check('invalid order '+str(args),args,2,'--retained-source')
        elif a.group=='permissions':
            for name in ['exec','ffi','net','http','env','spawn','gfx','sqlite']:
                check('deny '+name,profile+['--deny-'+name,'valid.sapp'])
            for name in ['fs','import']:
                check('deny '+name,profile+['--deny-'+name,'valid.sapp'],None,'')
            check('locked sandbox',profile+['--sandbox','--allow-all','valid.sapp'],None,'')
            check('env cannot select profile',['--check','--via-pblang','colon.sapp'],env={'PBLANG_CHECK_PROFILE':'retained-source'})
            check('env cannot disable explicit profile',profile+['colon.sapp'],1,'retained-nested-try',env={'PBLANG_CHECK_PROFILE':'default'})
            grants=['--ffi-lib=/usr/lib/x86_64-linux-gnu/libc.so.6']+['--ffi-symbol='+s for s in ['free','close','dup','strdup','openat','memfd_create','sendfile','lseek']]
            check('bounded valid composition',profile+['--bounded-source=4096',*grants,'valid.sapp'])
            check('bounded retained rejection',profile+['--bounded-source=4096',*grants,'colon.sapp'],1,'retained-nested-try')
            check('byte bound before profile',profile+['--bounded-source=1',*grants,'colon.sapp'],1,'exceeds byte limit')
        else:
            for native in [False,True]:
                check('retained colon still 9 '+str(native),['run','colon.sapp','--sandbox'],0,'9\n',native)
                check('explicit call still 7 '+str(native),['run','explicit.sapp','--sandbox'],0,'7\n',native)
            check('default checker unchanged',['--check','--via-pblang','colon.sapp'])
            check('PBLang PBCK compile',['run',release/'tools/pbc.sapp','--','colon.sapp','--target','pbck','--out','out.pbck'],0,'compiled native PBCK')
            check('PBCK yields 7',['run','out.pbck','--sandbox'],0,'7\n')
            example=release/'examples/retained_nested_method.sapp'
            check('documented example preflight',profile+[example],1,'retained-nested-try')
            check('documented example compilation',['run',release/'tools/pbc.sapp','--',example,'--target','pbck','--out','example.pbck'],0,'compiled native PBCK')
            check('documented example PBCK',['run','example.pbck','--sandbox'],0,'7\n')
            check('bare script remains retained',['colon.sapp','--sandbox'],0,'9\n')
            check('program argument is not a profile',['run','valid.sapp','--','--retained-source'],0,'7\n')
        a.report.write_text(json.dumps(rows,indent=2,ensure_ascii=False))

if __name__=='__main__':main()
