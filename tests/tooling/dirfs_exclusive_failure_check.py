"""Bounded paired exclusive-write tests; no production fault hooks or new grants.

Run under the existing coordinator heavy lock. Whole source modules are copied
privately; only native-effect boundaries are replaced for modeled cases. Linux
replacement tests use real files and a finite host/child rendezvous.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import tempfile
import time
import tomllib
from secure_file_exclusive_close_check import SYMBOLS, replace_once, snapshot, CALLER_ADAPTER

MODEL = r'''
let state=map{};
fn configure(wrote,fc,pc) {state.wrote=wrote;state.fc=fc;state.pc=pc;state.closes=[];state.writes=0;state.opens=0;state.parents=0;state.unlinks=0;state.bytes=nil;state.open_ok=true;state.parent_ok=true;}
fn parent() {state.parents=state.parents+1;return if state.parent_ok {map{ok:true,fd:10,name:"value"}} else {map{ok:false,error:"parent rejected"}};}
fn open() {state.opens=state.opens+1;return if state.open_ok {20} else {-1};}
fn close(fd) {array_push(state.closes,fd);let value=if fd==20 {state.fc} else {state.pc};if value=="throw" {throw "modeled close exception";}return value;}
fn unlink() {state.unlinks=state.unlinks+1;return 0;}
fn write(bytes) {state.writes=state.writes+1;state.bytes=bytes;if not state.wrote {throw "modeled write failure";}}
export state,configure,parent,open,close,unlink,write;
'''


def body_replace(text, name, body):
    start = text.index('fn '+name+'(')
    opening = text.index('{', start)
    depth = 1
    quoted = False
    escaped = False
    for end in range(opening+1, len(text)):
        ch = text[end]
        if quoted:
            if escaped: escaped = False
            elif ch == '\\': escaped = True
            elif ch == '"': quoted = False
        elif ch == '"': quoted = True
        elif ch == '{': depth += 1
        elif ch == '}':
            depth -= 1
            if depth == 0:
                return text[:opening+1]+'\n'+body+'\n'+text[end:]
    raise AssertionError(name)


def modeled(raw, posix):
    raw = body_replace(raw, '_parent', 'return B.parent();')
    raw = body_replace(raw, '_openat', 'return B.open();')
    raw = body_replace(raw, '_close_fd', 'return B.close(fd);')
    if posix and 'fn _exclusive_close_fd(' in raw:
        raw = body_replace(raw, '_exclusive_close_fd', 'return B.close(fd);')
    raw = body_replace(raw, '_unlinkat', 'return B.unlink();')
    call = 'fs_write_bytes(_fd_path(fs, fd), bytes)' if posix else 'fs_write_bytes(_fd_path(fd), bytes)'
    start = raw.index('fn write_new_exclusive(')
    tail = raw[start:]
    assert call in tail
    raw = raw[:start]+tail.replace(call, 'B.write(bytes)', 1)
    if not posix:
        raw += '\nfn profile(name) {return map{};}\nexport profile;\n'
    return 'let B=import("model.sapp");\n'+raw


def execute(runtime, files, real=False, cap=1048576, unlink=False, exchange=False):
    with tempfile.TemporaryDirectory(prefix='dirfs-exclusive-') as tmp:
        root=Path(tmp); scratch=root/'scratch'; scratch.mkdir(mode=0o700)
        (scratch/'input').write_bytes(b'KEEP INPUT'); (scratch/'existing').write_bytes(b'KEEP OUTPUT')
        os.link(scratch/'input',scratch/'alias'); (scratch/'link').symlink_to('input')
        (scratch/'dirlink').symlink_to('.',target_is_directory=True)
        for name,text in files.items(): (root/name).write_text(text)
        before={p.name:snapshot(p) for p in scratch.iterdir()}
        stdlib=runtime/'stdlib'
        manifest='[package]\nname="dirfs-exclusive-test"\nversion="1.0.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path='+json.dumps(str(stdlib))+'}\n'
        (root/'pblang.toml').write_text(manifest)
        digest=tomllib.loads((runtime/'pblang.lock').read_text())['dependencies']['stdlib']['sha256']
        (root/'pblang.lock').write_text('manifest_sha256='+json.dumps(hashlib.sha256(manifest.encode()).hexdigest())+'\ncontent_hash_algorithm="pblang-package-tree-v2"\n[dependencies.stdlib]\npath='+json.dumps(str(stdlib))+'\nsha256='+json.dumps(digest)+'\n')
        box=['bwrap','--unshare-all','--die-with-parent','--clearenv']
        for path in ['/usr','/bin','/lib','/lib64']:box+=['--ro-bind',path,path]
        box+=['--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(runtime),str(runtime),'--ro-bind',tmp,tmp,'--bind',str(scratch),str(scratch),'--chdir',tmp,'--setenv','PATH','/usr/bin:/bin','--setenv','LANG','C.UTF-8']
        flags=['--deny-'+s for s in ['net','http','env','spawn','gfx','sqlite']]
        if real:
            flags+=['--allow-exec','--allow-ffi','--ffi-lib=/usr/lib/x86_64-linux-gnu/libc.so.6']+['--ffi-symbol='+s for s in SYMBOLS]
            if unlink:flags+=['--ffi-symbol=unlinkat']
        else:flags+=['--deny-ffi','--deny-exec']
        def limits():
            for kind,value in [(resource.RLIMIT_AS,512*1024*1024),(resource.RLIMIT_CPU,10),(resource.RLIMIT_FSIZE,cap),(resource.RLIMIT_CORE,0)]:resource.setrlimit(kind,(value,value))
            signal.signal(signal.SIGXFSZ,signal.SIG_IGN)
        child=subprocess.Popen(box+[str(runtime/'bin/pblang'),'run','main.sapp',*flags],stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True,preexec_fn=limits)
        replaced=None; timed_out=False
        try:
            if exchange:
                deadline=time.monotonic()+8
                while not (scratch/'ready').exists() and child.poll() is None and time.monotonic()<deadline:time.sleep(.005)
                if (scratch/'ready').exists():
                    original=snapshot(scratch/'target');(scratch/'target').rename(scratch/'held-original')
                    (scratch/'target').write_bytes(b'FOREIGN REPLACEMENT')
                    replaced={'created':original,'renamed':snapshot(scratch/'held-original'),'replacement':snapshot(scratch/'target')}
                    (scratch/'ack').write_text('go')
            stdout,stderr=child.communicate(timeout=15)
        except subprocess.TimeoutExpired:
            timed_out=True;os.killpg(child.pid,signal.SIGKILL);stdout,stderr=child.communicate()
        finally:
            if child.poll() is None:os.killpg(child.pid,signal.SIGKILL);child.communicate()
        remaining=False;deadline=time.monotonic()+.5
        while True:
            try:os.killpg(child.pid,0)
            except ProcessLookupError:break
            if time.monotonic()>=deadline:remaining=True;os.killpg(child.pid,signal.SIGKILL);break
            time.sleep(.005)
        after={p.name:snapshot(p) for p in scratch.iterdir()}
        return dict(exit=child.returncode,stdout=stdout.decode(errors='replace'),stderr=stderr.decode(errors='replace'),bounded=not timed_out and not remaining and len(stdout)<65536 and len(stderr)<65536,inputs_unchanged=all(after.get(k)==v for k,v in before.items()),before=before,after=after,replacement=replaced,outputs={p.name:p.read_bytes().hex() for p in scratch.iterdir() if p.name not in before and p.is_file()},real_kernel_close_fault=False)


def instrument(raw, exchange=False):
    start=raw.index('fn write_new_exclusive(');end=raw.index('\nfn ',start+1)
    part=raw[start:end].replace('_close_fd(', '_tracked_close(').replace('_unlinkat(', '_tracked_unlink(')
    if exchange:
        part=replace_once(part,'fs_write_bytes(_fd_path(fd), bytes)','_exchange_write(fd, bytes)')
    helpers=r'''
let trace=map{closes:[],unlinks:0};
fn _tracked_close(fs,fd) {array_push(trace.closes,fd);return _close_fd(fs,fd);}
fn _tracked_unlink(fs,parent,name,flags) {trace.unlinks=trace.unlinks+1;return _unlinkat(fs,parent,name,flags);}
fn _exchange_write(fd,bytes) {
 fs_write_bytes(_fd_path(fd),slice(bytes,0,16));fs_write("scratch/ready","ready");
 let start=now_ms();while not fs_exists("scratch/ack") and now_ms()-start<5000 {sleep_ms(5);}
 if not fs_exists("scratch/ack") {throw "rendezvous timed out";}
 throw "injected write failure after real prefix and namespace replacement";
}
export trace;
'''
    return raw[:start]+part+raw[end:]+helpers


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--runtime',type=Path,required=True);ap.add_argument('--baseline',type=Path,required=True);ap.add_argument('--report',type=Path,required=True)
    a=ap.parse_args();os.umask(0o077);candidate=Path(__file__).resolve().parents[2];rows=[]
    def record(label,r,passed):
        rows.append(dict(label=label,passed=bool(passed and r['bounded'] and r['inputs_unchanged'] and r['exit']==0 and r['stderr']==''),result=r))
        a.report.write_text(json.dumps(rows,indent=2)+'\n');a.report.chmod(0o600);print(label,rows[-1]['passed'],flush=True)
        assert rows[-1]['passed'],label
    semantic=(candidate/'tests/semantics/dirfs_exclusive_failure_test.sapp').read_text()
    for variant,source in [('baseline',a.baseline),('candidate',candidate)]:
        baseline=variant=='baseline'
        for posix in [False,True]:
            raw=(source/'stdlib'/('posix_dirfs.sapp' if posix else 'linux_dirfs.sapp')).read_text()
            files={'dirfs.sapp':modeled(raw,posix),'model.sapp':MODEL,'semantic.sapp':semantic,'main.sapp':'let D=import("dirfs.sapp");let B=import("model.sapp");let T=import("semantic.sapp");print json_stringify(T.run(D,B,'+str(baseline).lower()+','+str(posix).lower()+'));'}
            r=execute(a.runtime,files);record(variant+('/posix-model' if posix else '/linux-model'),r,('"cases":38' if baseline else '"cases":57') in r['stdout'])
        raw=(source/'stdlib/linux_dirfs.sapp').read_text()
        for case in ['normal','partial-production','partial-unlink-test-only','replacement']:
            main='let D=import("dirfs.sapp");let fs=D.open("scratch",false);assert(fs.ok,"open root");'
            if case=='normal':
                main+='let r=D.write_new_exclusive(fs,"target",[0,127,255],false);assert(r.ok,"write");assert(D.write_new_exclusive(fs,"empty",[],false).ok,"empty");assert(D.write_new_exclusive(fs,"utf8",str_to_bytes("α"),false).ok,"utf8");for path in ["existing","alias","link","dirlink/new","missing/new"] {assert(not D.write_new_exclusive(fs,path,[65],false).ok,"refuse");}'
            else:main+='let r=D.write_new_exclusive(fs,"target",str_to_bytes("0123456789abcdef0123456789abcdef"),false);assert(not r.ok,"write failure");'
            main+='assert(D.close(fs),"root close");print json_stringify(map{result:r,trace:D.trace});'
            r=execute(a.runtime,{'dirfs.sapp':instrument(raw,case=='replacement'),'main.sapp':main},True,16 if case.startswith('partial') else 1048576,case in ['replacement','partial-unlink-test-only'],case=='replacement')
            try:data=json.loads(r['stdout'])
            except ValueError:data={}
            good=bool(data)
            if case=='normal':good=good and r['outputs']=={'target':'007fff','empty':'','utf8':'ceb1'} and all(r['after'][n]['mode'] & 0o777 == 0o600 for n in r['outputs'])
            else:
                good=good and len(data.get('trace',{}).get('closes',[]))==2 and data['trace']['unlinks']==int(baseline)
                if case=='replacement':
                    x=r['replacement'];good=good and x is not None and r['after'].get('held-original')==x['renamed'] and r['outputs'].get('held-original')==b'0123456789abcdef'.hex()
                    good=good and (('target' not in r['after']) if baseline else r['after'].get('target')==x['replacement'])
                else:
                    expected={} if baseline and case=='partial-unlink-test-only' else {'target':b'0123456789abcdef'.hex()}
                    good=good and r['outputs']==expected
                if not baseline:
                    result=data['result'];good=good and result['error']=='exclusive destination write failed; output may remain' and result['cleanup_uncertain'] and not result['rollback_attempted'] and result['file_close_confirmed'] and result['dirfs_parent_close_confirmed']
            record(variant+'/'+case,r,good)
        facade_real=replace_once((candidate/'stdlib/secure_file.sapp').read_text(),
            'let LinuxDirFS = import("stdlib:linux_dirfs.sapp");',
            'let LinuxDirFS = import("dirfs.sapp");')
        for api in ['bytes','text']:
            for partial in [False,True]:
                payload='str_to_bytes("0123456789abcdef0123456789abcdef")' if api=='bytes' else '"0123456789abcdef0123456789abcdef"'
                method='write_new_exclusive' if api=='bytes' else 'write_text_new_exclusive'
                main='let S=import("facade.sapp");let r=S.'+method+'("scratch/target",'+payload+',false,false);print json_stringify(r);'
                r=execute(a.runtime,{'dirfs.sapp':raw,'facade.sapp':facade_real,'main.sapp':main},True,16 if partial else 1048576)
                try:data=json.loads(r['stdout'])
                except ValueError:data={}
                expected=b'0123456789abcdef' if partial else b'0123456789abcdef0123456789abcdef'
                good=data.get('ok') is (not partial) and r['outputs']=={'target':expected.hex()} and r['after'].get('target',{}).get('mode',0)&0o777==0o600
                if partial and not baseline:good=good and data['cleanup_uncertain'] and not data['rollback_attempted'] and data['dirfs_parent_close_confirmed'] and data['file_close_confirmed']
                record(variant+'/facade-real-'+api+('-partial' if partial else '-normal'),r,good)
        if not baseline:
            # Execute complete effectful callers through unchanged facade and new backend.
            backend=modeled(raw,False)
            backend=body_replace(backend,'open','return map{ok:true};')
            backend=body_replace(backend,'open_absolute','return map{ok:true};')
            backend=body_replace(backend,'close','return true;')
            facade=replace_once((source/'stdlib/secure_file.sapp').read_text(),'let LinuxDirFS = import("stdlib:linux_dirfs.sapp");','let LinuxDirFS = import("dirfs.sapp");')
            for caller in ['polyglot_web','polyglot_build']:
                text=replace_once((source/'stdlib'/f'{caller}.sapp').read_text(),'let SECURE_FILE = import("stdlib:secure_file.sapp");','let SECURE_FILE = import("caller_adapter.sapp");')
                call='C.bundle("synthetic","","")' if caller=='polyglot_web' else 'C.build_language("python","sample","synthetic","/tmp",map{unsafe_polyglot:true,trusted_source:true,allow_toolchain_effects:true,unsafe_polyglot_sha256:H.sha256_hex_bytes(str_to_bytes("synthetic"))})'
                main='let B=import("model.sapp");let A=import("caller_adapter.sapp");let C=import("caller.sapp");let H=import("stdlib:sha256_pb.sapp");B.configure(false,true,true);let failed=try {'+call+';false} catch true;assert(failed and B.state.writes==1 and B.state.closes==[20,10] and B.state.unlinks==0 and A.state.removes==0 and A.state.atomic_writes==0,"new backend failure stops complete caller");print "caller-pass";'
                r=execute(a.runtime,{'model.sapp':MODEL,'dirfs.sapp':backend,'facade.sapp':facade,'caller_adapter.sapp':CALLER_ADAPTER,'caller.sapp':text,'main.sapp':main});record('candidate/'+caller,r,r['stdout']=='caller-pass\n')
            main='let B=import("model.sapp");let S=import("facade.sapp");B.configure(false,true,false);let r=S.write_new_exclusive("owned/value",[65],false);assert(not r.ok and r.cleanup_uncertain and r.file_close_confirmed and not r.dirfs_parent_close_confirmed and not r.rollback_attempted,"facade preserves backend failure");print "facade-pass";'
            r=execute(a.runtime,{'model.sapp':MODEL,'dirfs.sapp':backend,'facade.sapp':facade,'main.sapp':main});record('candidate/facade',r,r['stdout']=='facade-pass\n')

if __name__=='__main__':main()
