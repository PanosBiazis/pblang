"""Whole-module bounded-read cleanup differential qualification.

Run under the coordinator's existing bounded heavy lock. Only disposable
synthetic native-effect boundaries are modeled; no production fault hooks,
new grants, source execution, or actual kernel close-fault claim.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import sys
import tempfile
import tomllib
sys.dont_write_bytecode = True
from check_cli_exact_argv import bounded_process

MODEL = r'''let state=map{};
fn configure(mode) {state.mode=mode;state.closes=[];state.calls=[];state.parents=0;state.opens=0;state.reads=0;state.copied=0;}
fn root() {return map{ok:true,closed:false,root_fd:1,libc:1,child_fds:map{},lock_fds:map{}};}
fn parent(fs) {if fs.closed {return map{ok:false,error:"directory root is closed"};}state.parents=state.parents+1;if state.mode=="parent-reject" {return map{ok:false,error:"parent rejected"};}return map{ok:true,fd:10,name:"synthetic"};}
fn open() {state.opens=state.opens+1;if state.mode=="open-throw" {throw "modeled open exception";}if str_contains(state.mode,"open-fail") {return -1;}return 20;}
fn call(symbol,values) {
 array_push(state.calls,symbol);
 if symbol=="close" {
  let fd=values[0];array_push(state.closes,fd);
  let tag="root";if fd==10 {tag="parent";} elif fd==20 {tag="source";} elif fd==30 {tag="snapshot";}
  if str_contains(state.mode,tag+"-throw") {throw "synthetic cleanup exception";}
  if str_contains(state.mode,tag+"-fail") {return -1;}return 0;
 }
 if symbol=="memfd_create" {if str_contains(state.mode,"create-fail") {return -1;}return 30;}
 if symbol=="sendfile" {
  let remaining=values[3];if state.mode=="copy-throw" {throw "modeled copy exception";}
  if str_contains(state.mode,"copy-fail") {return -1;}
  if state.mode=="count-over" {state.copied=4;return remaining+1;}
  if state.mode=="count-frac" {if state.copied==0 {state.copied=4;return 0.5;}return 0;}
  if state.mode=="count-nan" {state.copied=4;return nan_value(0);}
  if state.mode=="count-inf" {state.copied=4;return 1.0/nan_zero(0);}
  let total=4;if state.mode=="empty" {total=0;} elif state.mode=="overflow" {total=5;}
  let n=total-state.copied;if n>remaining {n=remaining;}
  if state.mode=="short-copy" and n>1 {n=1;}
  state.copied=state.copied+n;return n;
 }
 if symbol=="lseek" {
  if values[2]==2 {
   if state.mode=="size-throw" {throw "modeled size exception";}
   if str_contains(state.mode,"size-fail") {return -1;}
   if state.mode=="size-frac" {return 1.5;}
   if state.mode=="size-nan" {return nan_value(0);}
   return state.copied;
  }
  if str_contains(state.mode,"rewind-fail") {return -1;}
  if state.mode=="rewind-positive" {return 1;}return 0;
 }
 throw "unexpected modeled symbol";
}
fn nan_zero(v) {return v;}
fn nan_value(v) {return 0.0/v;}
fn read() {
 state.reads=state.reads+1;
 if str_contains(state.mode,"read-throw") {throw "synthetic byte read exception";}
 if state.mode=="read-type" {return "wrong type";}
 if state.mode=="read-size" {return [65];}
 if state.mode=="empty" {return [];}
 return [65,0,206,177];
}
fn library_close() {return true;}
export state,configure,root,parent,open,call,read,library_close;
'''

def body_replace(text,name,body):
 start=text.index('fn '+name+'(');opening=text.index('{',start);depth=1;quoted=False;escaped=False
 for end in range(opening+1,len(text)):
  c=text[end]
  if quoted:
   if escaped:escaped=False
   elif c=='\\':escaped=True
   elif c=='"':quoted=False
  elif c=='"':quoted=True
  elif c=='{':depth+=1
  elif c=='}':
   depth-=1
   if depth==0:return text[:opening+1]+'\n'+body+'\n'+text[end:]
 raise AssertionError(name)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime',type=Path,required=True)
    parser.add_argument('--baseline',type=Path,required=True)
    parser.add_argument('--report',type=Path,required=True)
    args=parser.parse_args();os.umask(0o077)
    runtime=args.runtime.resolve();candidate=Path(__file__).resolve().parents[2]
    report={'rows':[], 'passed':False, 'real_kernel_close_fault':False,
            'source_hidden':True,'network_isolated':True,
            'effect_substitutions':['_parent','_openat','_call','_close_library','open','open_absolute','snapshot fs_read_bytes'],
            'unchanged_effect_logic':['_close_fd','close','bounded_source.read_source']}
    args.report.parent.mkdir(parents=True,exist_ok=True)
    def save():
        args.report.write_text(json.dumps(report,indent=2)+'\n');args.report.chmod(0o600)
    for name,source in [('baseline',args.baseline.resolve()),('candidate',candidate)]:
        raw=(source/'stdlib/linux_dirfs.sapp').read_text();original=raw
        for fn,body in {'_parent':'return B.parent(fs);','_openat':'return B.open();',
                        '_call':'return B.call(symbol,args);','_close_library':'return B.library_close();',
                        'open':'return B.root();','open_absolute':'return B.root();'}.items():
            raw=body_replace(raw,fn,body)
        raw='let B=import("model.sapp");\n'+raw
        start=raw.index('fn _bounded_read_owned(') if 'fn _bounded_read_owned(' in raw else raw.index('fn _read_bytes_relative(')
        end=raw.index('\nfn ',start+1);part=raw[start:end]
        expression='try fs_read_bytes(_fd_path(snapshot)) catch nil'
        assert part.count(expression)==1
        raw=raw[:start]+part.replace(expression,'try B.read() catch nil')+raw[end:]
        raw+='\nfn _test_reset(poisoned=false) {_mount_admission_cleanup_poisoned=poisoned;}\nexport _test_reset;\n'
        reader=(source/'stdlib/compiler/bounded_source.sapp').read_text()
        needle='import("stdlib:linux_dirfs.sapp")';assert reader.count(needle)==1
        reader=reader.replace(needle,'import("dirfs.sapp")')
        semantic=(candidate/'tests/semantics/bounded_read_cleanup_test.sapp').read_text()
        main='let D=import("dirfs.sapp");let B=import("model.sapp");let R=import("reader.sapp");let T=import("semantic.sapp");print json_stringify(T.run(D,B,R,'+str(name=='baseline').lower()+'));'
        files={'dirfs.sapp':raw,'model.sapp':MODEL,'reader.sapp':reader,'semantic.sapp':semantic,'main.sapp':main}
        frozen=args.report.parent/(args.report.stem+'-'+name+'-fixtures');frozen.mkdir(mode=0o700)
        for file,text in files.items():
            p=frozen/file;p.write_text(text);p.chmod(0o600)
        with tempfile.TemporaryDirectory(prefix='bounded-read-model-') as tmp:
            t=Path(tmp)
            for file,text in files.items():(t/file).write_text(text)
            stdlib=runtime/'stdlib'
            manifest='[package]\nname="bounded-read-model"\nversion="1.0.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path='+json.dumps(str(stdlib))+'}\n'
            (t/'pblang.toml').write_text(manifest)
            digest=tomllib.loads((runtime/'pblang.lock').read_text())['dependencies']['stdlib']['sha256']
            (t/'pblang.lock').write_text('manifest_sha256='+json.dumps(hashlib.sha256(manifest.encode()).hexdigest())+'\ncontent_hash_algorithm="pblang-package-tree-v2"\n[dependencies.stdlib]\npath='+json.dumps(str(stdlib))+'\nsha256='+json.dumps(digest)+'\n')
            box=['bwrap','--unshare-all','--die-with-parent','--clearenv']
            for q in ['/usr','/bin','/lib','/lib64']:box+=['--ro-bind',q,q]
            box+=['--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(runtime),str(runtime),'--ro-bind',tmp,tmp,'--chdir',tmp,'--setenv','PATH','/usr/bin:/bin','--setenv','LANG','C.UTF-8']
            flags=['--deny-'+x for x in ['net','http','env','spawn','gfx','sqlite','ffi','exec']]
            r=bounded_process(box+[str(runtime/'bin/pblang'),'run','main.sapp',*flags])
            r.update(name=name,source_sha256=hashlib.sha256(original.encode()).hexdigest(),fixture_sha256={k:hashlib.sha256(v.encode()).hexdigest() for k,v in files.items()})
            r['passed']=r['exit']==0 and r['failure'] is None and not r['group_survived_normal_exit'] and r['stderr']=='' and '\ufffd' not in r['stdout']
            if r['passed']:
                r['result']=json.loads(r.pop('stdout'))
                r['passed']=r['result']['checks']>0 and len(r['result']['rows'])==(36 if name=='baseline' else 40)
            report['rows'].append(r);save();print(name,r['passed'],flush=True)
            if not r['passed']:raise AssertionError(name+' qualification failed; evidence retained')
    report['passed']=True;save()

if __name__=='__main__':main()
