"""Isolated selfhost parity and regressions with hard streaming capture.

Requires Linux Bubblewrap, an explicit retained runtime and an existing shared
lock identity. Creates a new report/snapshot directory; never installs or edits
source/fixture inputs. Every case has assertions, finite resources and receipts.
"""
import argparse, fcntl, hashlib, json, os, resource, selectors, shutil, signal, subprocess, tarfile, time
from pathlib import Path
CAP=65536

def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()

def limits():
    for typ,val in [(resource.RLIMIT_AS,768*1024*1024),(resource.RLIMIT_CPU,28),(resource.RLIMIT_FSIZE,1024*1024),(resource.RLIMIT_CORE,0)]:
        resource.setrlimit(typ,(val,val))
def capture(command,seconds):
    start=time.monotonic();p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True,preexec_fn=limits)
    streams={'stdout':bytearray(),'stderr':bytearray()};total=0;reason=None
    sel=selectors.DefaultSelector()
    for name,stream in [('stdout',p.stdout),('stderr',p.stderr)]:
        os.set_blocking(stream.fileno(),False);sel.register(stream,selectors.EVENT_READ,name)
    def stop():
        try:os.killpg(p.pid,signal.SIGKILL)
        except ProcessLookupError:pass
    try:
        while sel.get_map():
            remaining=seconds-(time.monotonic()-start)
            if remaining<=0:reason='timeout';stop();break
            for key,_ in sel.select(min(.2,remaining)):
                data=os.read(key.fd,8192)
                if not data:sel.unregister(key.fileobj);key.fileobj.close();continue
                available=CAP-total
                streams[key.data].extend(data[:available]);total+=min(len(data),available)
                if len(data)>available:reason='output_cap';stop();break
            if reason:break
        if p.poll() is None:
            try:p.wait(timeout=max(.01,seconds-(time.monotonic()-start)))
            except subprocess.TimeoutExpired:reason='timeout';stop()
    finally:
        stop() # reap descendants sharing the owned session even after main exit
        p.wait(timeout=5)
        sel.close()
        for stream in [p.stdout,p.stderr]:stream.close()
    group_remaining=False
    # Bubblewrap's namespace children can remain briefly visible while reaped.
    # Record actual group disappearance with a finite cleanup grace interval.
    cleanup_deadline=time.monotonic()+.5
    while True:
        try:os.killpg(p.pid,0)
        except ProcessLookupError:break
        if time.monotonic()>=cleanup_deadline:group_remaining=True;break
        time.sleep(.01)
    return {'owned_group_remaining':group_remaining,'command':command,'exit':p.returncode,'stop_reason':reason,'stdout':streams['stdout'].decode(errors='replace'),'stderr':streams['stderr'].decode(errors='replace'),'capture_bytes':total,'capture_cap':CAP,'seconds':time.monotonic()-start,'child_reaped':p.poll() is not None,'pid':p.pid}
PRODUCTION=['vm.sapp','vm_builtins.sapp','typeck.sapp','selfhost_frontend.sapp','module_cache.sapp','import_roots.sapp']
NEW_TESTS=['selfhost_data_builtins','selfhost_data_bounds','selfhost_import_roots']
REGRESSIONS=['selfhost_vm_builtin_registry','selfhost_vm_call_admission','selfhost_default_values','selfhost_vm_bounded','selfhost_exception_lifetime','selfhost_compile_lexical','selfhost_modules','selfhost_memory_failure_cleanup','selfhost_numeric_cleanup','selfhost_utf8_bytes','selfhost_string_bounds','selfhost_join_bounds','selfhost_typeck','selfhost_typeck_parity']
FIXTURES=['original.sapp','native-original.sapp','src/sensory_tools.sapp','src/sensory_series.sapp']

def tree_digest(root):
    h=hashlib.sha256(b'pblang-package-tree-v2\0')
    for p in sorted(root.rglob('*'),key=lambda p:p.relative_to(root).as_posix().encode()):
        name=p.relative_to(root).as_posix().encode();data=p.read_bytes() if p.is_file() else b''
        h.update(b'f' if p.is_file() else b'd');h.update(len(name).to_bytes(8,'big'));h.update(name)
        h.update(len(data).to_bytes(8,'big'));h.update(data)
    return h.hexdigest()

def write_lock(root,digest,dependency_path='../pblang/stdlib'):
    text=(root/'pblang.toml').read_text()
    (root/'pblang.lock').write_text('manifest_sha256='+json.dumps(hashlib.sha256(text.encode()).hexdigest())+'\ncontent_hash_algorithm="pblang-package-tree-v2"\n[dependencies.stdlib]\npath='+json.dumps(dependency_path)+'\nsha256='+json.dumps(digest)+'\n')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['repo','runtime','report','lock']:parser.add_argument('--'+name,required=True,type=Path)
    for name in ['lock-dev','lock-inode']:parser.add_argument('--'+name,required=True,type=int)
    parser.add_argument('--only',nargs='+',choices=NEW_TESTS+REGRESSIONS,help='Repeat only selected focused/regression suites')
    parser.add_argument('--design',type=Path,help='Optional frozen reviewed design to hash in the local receipt')
    parser.add_argument('--group',choices=['original','focused','regression','cache','resolver'],default='original')
    args=parser.parse_args();os.umask(0o077)
    repo=args.repo.resolve();runtime=args.runtime.resolve();out=args.report.resolve();out.mkdir(exist_ok=False)
    work=out/'snapshot';work.mkdir();pb=work/'pblang';pb.mkdir()
    archive=out/'base.tar'
    with archive.open('wb') as f:
        subprocess.run(['git','archive','HEAD','stdlib','tests/semantics','tests/fixtures','pblang.toml','pblang.lock'],cwd=repo,stdout=f,check=True,timeout=30)
    with tarfile.open(archive) as t:t.extractall(pb,filter='data')
    for name in PRODUCTION:shutil.copy2(repo/'stdlib/compiler'/name,pb/'stdlib/compiler'/name)
    for name in NEW_TESTS:shutil.copy2(repo/'tests/semantics'/(name+'_test.sapp'),pb/'tests/semantics'/(name+'_test.sapp'))
    digest=tree_digest(pb/'stdlib');write_lock(pb,digest)
    manifest='[package]\nname="selfhost-sensory"\nversion="0.0.0"\nentry="original.sapp"\n[dependencies]\nstdlib={path="../pblang/stdlib",paths=["stdlib"]}\n'
    fixture_pins={}
    for directory in ['product','local']:
        root=work/directory;(root/'src').mkdir(parents=True)
        for name in FIXTURES:
            source=repo/'tests/fixtures/selfhost_sensory'/name
            shutil.copy2(source,root/name);fixture_pins[name]=sha(source)
        (root/'pblang.toml').write_text(manifest);write_lock(root,digest)
    pure=work/'pure';pure.mkdir()
    (pure/'pblang.toml').write_text(manifest.replace('stdlib={','"stdlib"={'))
    write_lock(pure,digest)
    head='let F=import("stdlib:compiler/selfhost_frontend.sapp");'
    basic='let r=F.frontend_run("print 7;");assert(r.ok and r.output==[7],"pure execution");'
    (pure/'denied.sapp').write_text(head+'assert((try {fs_read("pblang.toml");false}catch true),"manifest read really denied");'+basic+'print "pure_denied_pass";')
    (pure/'malformed.sapp').write_text(head+basic+'assert((try {F.frontend_run("let M=import(\\\"stdlib:json.sapp\\\");");false}catch true),"explicit unsupported declaration rejects only import execution");print "pure_malformed_pass";')
    (pure/'cache-denied.sapp').write_text(head+'assert((try {fs_read("pblang.toml");false}catch true),"manifest read really denied");F.frontend_run_bounded("print 1;",map{stdlib_root:"/unavailable",module_cache:true});')
    (work/'local/stdlib').mkdir()
    legacy=work/'legacy';(legacy/'stdlib').mkdir(parents=True)
    (legacy/'pblang.toml').write_text('[package]\nname="legacy-cache"\nversion="0.0.0"\nentry="driver.sapp"\n')
    (pb/'tests/semantics/cache_adapter_control.sapp').write_text('let D=import("stdlib:linux_dirfs.sapp");let C=import("stdlib:compiler/module_cache_file.sapp");print json_stringify(D.available());let d=D.open_absolute("/tmp",false,false);print json_stringify(map{ok:d.ok,error:try d.error catch nil});if d.ok {assert(D.close(d),"close preflight");}let r=C.write_text_atomic("/tmp/cache-preflight/value","x");print json_stringify(r);assert(r.ok,"cache adapter preflight");print "cache_adapter_pass";')
    identity_control=None
    if args.group=='resolver':
        # Execute compiler A and a byte-distinct A' against the same guest root B.
        # Never reuse B's fingerprint as proof of the executing compiler identity.
        changed=work/'changed';changed.mkdir()
        shutil.copytree(pb/'stdlib',changed/'stdlib')
        modified=changed/'stdlib/compiler/vm_builtins.sapp'
        modified.write_text(modified.read_text()+'\n// Qualification-only executing compiler A-prime marker.\n')
        (changed/'pblang.toml').write_text(manifest.replace('../pblang/stdlib','../changed/stdlib'))
        write_lock(changed,tree_digest(changed/'stdlib'),'../changed/stdlib')
        guest=work/'guest-b';guest.mkdir();(guest/'value.sapp').write_text('let value=42;export value;')
        control=head+'let src="let m=import(\\\"stdlib:value.sapp\\\");print m.value;";let opts=map{stdlib_root:"/work/guest-b",module_cache_root:"/tmp/unproven-cache"};assert((try {F.frontend_run_bounded(src,opts);false}catch true),"cross-root cache rejection");assert(not fs_exists("/tmp/unproven-cache"),"rejection before cache I/O");assert(F.frontend_run(src,map{stdlib_root:"/work/guest-b"}).output==[42],"uncached guest root B");print "compiler_identity_pass";'
        (work/'product/identity.sapp').write_text(control)
        (changed/'identity.sapp').write_text(control)
        identity_control={'compiler_A_sha256':sha(pb/'stdlib/compiler/vm_builtins.sapp'),'compiler_A_prime_sha256':sha(modified),'guest_B_sha256':sha(guest/'value.sapp'),'mutation':'comment appended only to disposable compiler A-prime; production candidate unchanged'}
    before={str(p.relative_to(work)):sha(p) for p in work.rglob('*') if p.is_file()}
    report={'compiler_identity_control':identity_control,'design_sha256':sha(args.design) if args.design else None,'design_status':'implementation/testing; final source delivery remains separately reviewed','format':'pblang.selfhost.sensory.qualification.v1','group':args.group,'production_edits':True,'base':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'runtime_sha256':sha(runtime),'runner_sha256':sha(Path(__file__)),'fixture_pins':fixture_pins,'source_pins':before,'cases':[],'lock_identity':[args.lock_dev,args.lock_inode]}
    fd=os.open(args.lock,os.O_RDWR);acquired=False
    try:
        st=os.fstat(fd)
        if (st.st_dev,st.st_ino)!=(args.lock_dev,args.lock_inode):raise RuntimeError('unexpected lock identity')
        deadline=time.monotonic()+45
        while time.monotonic()<deadline:
            try:fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB);acquired=True;break
            except BlockingIOError:time.sleep(.2)
        report['lock_acquired']=acquired
        if not acquired:
            report['disposition']='lock_busy_no_tests';raise TimeoutError('shared lock wait exceeded')
        start=time.monotonic()
        def run(name,script,expected,cwd='/work/pblang',local=False,cache=False,deny_fs=False,expected_exit=0):
            remaining=180-(time.monotonic()-start)
            if remaining<=0:raise TimeoutError('phase budget exhausted')
            command=['bwrap','--unshare-all','--die-with-parent','--clearenv']
            for p in ['/usr','/bin','/lib','/lib64']:command+=['--ro-bind',p,p]
            command+=['--proc','/proc','--dev','/dev','--tmpfs','/tmp','--dir','/runtime','--ro-bind',str(runtime),'/runtime/pblang','--ro-bind',str(work),'/work']
            if local:command+=['--ro-bind',str(pb/'stdlib'),'/work/local/stdlib']
            if cwd=='/work/legacy':command+=['--ro-bind',str(pb/'stdlib'),'/work/legacy/stdlib']
            command+=['--chdir',cwd,'--setenv','PATH','/usr/bin:/bin','--setenv','LANG','C.UTF-8','/runtime/pblang','run',script]
            denies=['net','http','env','spawn','gfx','sqlite']
            if deny_fs:denies+=['fs']
            if cache:
                # Existing cache adapter only; original sensory route denies FFI.
                command+=['--allow-exec','--allow-ffi','--ffi-lib=/usr/lib/x86_64-linux-gnu/libc.so.6']
                command+=['--ffi-symbol='+s for s in ['free','close','dup','strdup','openat','mkdirat','unlinkat','renameat','fchmod','fsync','memfd_create','sendfile','lseek','getpid','getuid','calloc','read','write']]
            else:denies+=['exec','ffi']
            command+=['--deny-'+s for s in denies]
            row=capture(command,min(30,remaining));row['name']=name
            row['expected_marker']=expected;row['expected_exit']=expected_exit
            completed=row['exit']==expected_exit and row['stop_reason'] is None and not row['owned_group_remaining']
            if expected_exit==0:
                matched=any(line==expected or (line.startswith(expected+' ') and line[len(expected)+1:].isdigit()) for line in row['stdout'].splitlines())
            else:
                matched=expected in row['stderr']
            row['assertion_passed']=completed and matched
            report['cases'].append(row);print(name,row['assertion_passed'],row['exit'],round(row['seconds'],3),flush=True)
        if args.group=='original':
            run('original-external','original.sapp','sensory_selfhost_pass','/work/product')
            run('original-matching-layout','original.sapp','sensory_selfhost_pass','/work/local',local=True)
            run('native-oracle','native-original.sapp','2000','/work/product')
            report['cases'][-1]['assertion_passed'] &= report['cases'][-1]['stdout'].strip()=='2000\n2\nfalse'
        elif args.group in ['focused','regression']:
            names=args.only or (NEW_TESTS if args.group=='focused' else NEW_TESTS+REGRESSIONS)
            for name in names:
                run(name,'tests/semantics/'+name+'_test.sapp',name+'_pass')
        elif args.group=='resolver':
            run('pure-denied-manifest','denied.sapp','pure_denied_pass','/work/pure',deny_fs=True)
            run('pure-unsupported-manifest','malformed.sapp','pure_malformed_pass','/work/pure')
            run('noimport-selected-cache-denied-fs','cache-denied.sapp','selfhost cache: executing compiler origin unavailable for selected stdlib root','/work/pure',deny_fs=True,expected_exit=1)
            run('compiler-A-guest-B','identity.sapp','compiler_identity_pass','/work/product')
            run('changed-compiler-A-guest-B','identity.sapp','compiler_identity_pass','/work/changed')
        else:
            run('cache-adapter','/work/pblang/tests/semantics/cache_adapter_control.sapp','cache_adapter_pass','/work/legacy',cache=True)
            run('legacy-cache','/work/pblang/tests/semantics/module_cache_test.sapp','module_cache_pass','/work/legacy',cache=True)
        report['test_seconds']=time.monotonic()-start
        report['all_passed']=bool(report['cases']) and all(c['assertion_passed'] for c in report['cases'])
    finally:
        if acquired:fcntl.flock(fd,fcntl.LOCK_UN)
        st=os.fstat(fd);report['lock_identity_after']=[st.st_dev,st.st_ino];os.close(fd)
        report['lock_released']=acquired
        after={str(p.relative_to(work)):sha(p) for p in work.rglob('*') if p.is_file()}
        report['source_pins_after']=after;report['source_unchanged']=before==after
        with (out/'receipt.json').open('x') as f:json.dump(report,f,indent=2);f.write('\n')
    if not report.get('all_passed'):raise SystemExit(1)
if __name__=='__main__':main()
