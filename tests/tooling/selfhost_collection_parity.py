"""Compare actual retained-host errors/results with PBLang VM dispatch routes."""
import argparse,json,re,subprocess,tempfile,shutil
from pathlib import Path

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--runtime',required=True);ap.add_argument('--repo',required=True);ap.add_argument('--report',required=True)
    args=ap.parse_args();binary=Path(args.runtime).resolve();repo=Path(args.repo).resolve();report=Path(args.report);rows=[]
    def record(name,passed,**details):
        rows.append(dict(name=name,passed=passed,**details));report.write_text(json.dumps(rows,indent=2,ensure_ascii=False));assert passed,rows[-1]
    def run(path,cwd):
        p=subprocess.run([str(binary),'run',str(path),'--allow-all'],cwd=cwd,text=True,capture_output=True,timeout=45)
        errors=re.findall(r'^error\[.*?\]: (.*)$',p.stderr,re.M)
        return dict(exit=p.returncode,stdout=p.stdout,error=errors[0] if errors else None,stderr=p.stderr)
    cases=[]
    for name,values in [('len','nil'),('array_pop','7'),('pop','false'),('array_sort','"abc"'),('map_keys','[]'),('array_push','nil,9'),('push','map{k:7},9'),('map_has','[],"k"'),('map_remove','map{k:7},9'),('has_member','nil,9')]:
        cases.append((name,'fn invoke(f){return f('+values+');}invoke('+name+');',False))
    for expr in ['bigint("42")','rational(3,2)','decimal("1.25")']:
        cases.append(('boxed map key '+expr,'fn invoke(f,k){return f(map{k:7},k);}invoke(map_remove,'+expr+');',False))
    cases += [
        ('generic operator map removal','let receiver=map{__add__:map_remove,k:7};receiver+9;',False),
        ('generic operator push','let receiver=map{__add__:push,k:7};receiver+9;',False),
        ('invalid iterator','fn values(){return nil;}for x in values(){print x;}',False),
        ('boxed map index','let m=map{k:7};m[bigint("42")]=9;',False),
        ('valid collections','let a=[];print array_pop(a);print push(a,false);print pop(a);let m=map{k:nil};print map_has(m,"k");print map_remove(m,"k");print map_remove(m,"missing");print has_member(7,"k");print map_keys(m);print len("αβ");print array_sort([3,1,2]);',True),
        ('valid exact numbers','fn id(x){return x;}let a=bigint("42");let b=rational(3,2);let c=decimal("1.25");print id(a+bigint("1"));print id(b);print id(c);print [a,b,c];',True),
        ('caught errors','fn invoke(f){return f(nil);}print (try invoke(array_pop) catch 17);print (try invoke(map_keys) catch false);',True),
        ('shadowed builtins','fn len(a){return 77;}fn array_pop(a){return false;}print len(nil);print array_pop(nil);',True),
    ]
    with tempfile.TemporaryDirectory(prefix='pblang-collection-parity-') as tmp:
        root=Path(tmp);shutil.copytree(repo/'stdlib',root/'stdlib');(root/'main.sapp').write_text('print "fixture";')
        (root/'pblang.toml').write_text('[package]\nname="collection-parity"\nversion="1.0.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path='+json.dumps(str(root/'stdlib'))+'}\n')
        p=subprocess.run([str(binary),'build',str(root)],cwd=root,text=True,capture_output=True,timeout=45)
        record('fixture build',p.returncode==0,exit=p.returncode,stderr=p.stderr)
        for name,source,valid in cases:
            native=root/'native.sapp';native.write_text(source);expected=run(native,root)
            record('retained '+name,(expected['exit']==0)==valid and (valid or expected['error'] is not None),**expected)
            for mode in ['plain','optimized','pbcs']:
                for bounded in [False,True]:
                    driver='let F=import("stdlib:compiler/selfhost_frontend.sapp");let V=import("stdlib:compiler/vm.sapp");let O=import("stdlib:compiler/optimizer.sapp");let P=import("stdlib:selfhost_pbck.sapp");\n'
                    driver+='let c=F.frontend_compile('+json.dumps(source,ensure_ascii=False)+');assert(c.ok,"fixture compile failed");let code=c.chunk;\n'
                    if mode=='optimized':driver+='code=O.optimize_safe(code).chunk;\n'
                    if mode=='pbcs':driver+='code=P.deserialize_chunk(P.serialize_chunk(code));\n'
                    driver+='let ctx=V.execution_context(true);V.run_seeded(F.frontend_compile("memory_alloc(7);").chunk,map{},ctx);\n'
                    driver+='let result=V.'+('run_seeded_bounded(code,map{},nil,ctx)' if bounded else 'run_seeded(code,map{},ctx)')+';V.close_execution(ctx);for value in result.output {print value;}\n'
                    path=root/'driver.sapp';path.write_text(driver);actual=run(path,root)
                    record(name+' '+mode+(' bounded' if bounded else ' unbounded'),actual['exit']==expected['exit'] and actual['stdout']==expected['stdout'] and actual['error']==expected['error'],**actual)
            print(name,'parity PASS',flush=True)
        # Explicitly retain the known host boundary instead of calling it fixed.
        source='for _i 1 {let p=map{run:fn(self){return 7;}};print (try {p:run()} catch 9);}'
        native=root/'native.sapp';native.write_text(source);actual=run(native,root)
        record('retained nested p:run remains blocked',actual['exit']==0 and actual['stdout'].strip()=='9',**actual)
        driver=root/'driver.sapp';driver.write_text('let F=import("stdlib:compiler/selfhost_frontend.sapp");let r=F.frontend_run('+json.dumps(source)+');assert(r.ok,"compile");for value in r.output {print value;}')
        actual=run(driver,root);record('explicit PBLang nested method route',actual['exit']==0 and actual['stdout'].strip()=='7',**actual)
    print('ALL PASS',len(rows),flush=True)

if __name__=='__main__':main()
