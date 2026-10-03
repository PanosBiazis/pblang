"""Real CLI packaging and source-integrity checks; fixtures are disposable only."""
from pathlib import Path
import argparse, hashlib, json, os, shutil, subprocess, tempfile, threading, time

def tree(root):
    result={}
    for p in sorted(root.rglob('*')):
        key=p.relative_to(root).as_posix()
        if p.is_symlink(): result[key]=['link',os.readlink(p)]
        elif p.is_file(): result[key]=['file',hashlib.sha256(p.read_bytes()).hexdigest()]
        elif p.is_dir(): result[key]=['dir']
        else: result[key]=['special']
    return result

def tree_digest(root):
    h=hashlib.sha256(b'pblang-package-tree-v2\0')
    for p in sorted(root.rglob('*'),key=lambda p:p.relative_to(root).as_posix().encode()):
        rel=p.relative_to(root).as_posix().encode();data=p.read_bytes() if p.is_file() else b''
        h.update(b'f' if p.is_file() else b'd');h.update(len(rel).to_bytes(8,'big'));h.update(rel)
        h.update(len(data).to_bytes(8,'big'));h.update(data)
    return h.hexdigest()

def fixtures(root,dependency=False):
    root.mkdir();(root/'src').mkdir();(root/'assets').mkdir();(root/'assets/empty-dir').mkdir()
    (root/'assets/empty.bin').write_bytes(b'');(root/'assets/payload.bin').write_bytes(bytes([0,255,128,7]))
    (root/'assets/message.txt').write_text('portable fixture\n')
    (root/'.private').write_text('not-for-the-package')
    manifest='[package]\nname="package-fixture"\nversion="1.0.0"\nentry="src/main.sapp"\n'
    source='let value=42;\n'
    if dependency:
        (root/'dep').mkdir();(root/'dep/nested').mkdir();(root/'dep/empty').mkdir()
        (root/'dep/nested/value.sapp').write_text('let value=42;export value;\n')
        (root/'dep/binary.dat').write_bytes(bytes([0,255,128]))
        manifest+='[dependencies]\ncalc={path="dep"}\n';source='let d=import("calc:nested/value.sapp");let value=d.value;\n'
    (root/'pblang.toml').write_text(manifest)
    (root/'src/main.sapp').write_text(source+'print "packaged: "+str(value);\n')
    if dependency:
        lock=f'manifest_sha256 = "{hashlib.sha256(manifest.encode()).hexdigest()}"\ncontent_hash_algorithm = "pblang-package-tree-v2"\n\n[dependencies.calc]\npath = "{root}/dep"\nsha256 = "{tree_digest(root/"dep")}"\n'
        (root/'pblang.lock').write_text(lock)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--runtime',required=True);parser.add_argument('--repo',required=True);parser.add_argument('--report',required=True)
    opts=parser.parse_args();binary=Path(opts.runtime).resolve();repo=Path(opts.repo).resolve();report=Path(opts.report);rows=[]
    def record(label,passed,**details):
        row=dict(label=label,passed=passed,**details);rows.append(row);report.write_text(json.dumps(rows,indent=2,ensure_ascii=False));print(label,passed,flush=True)
        assert passed,row
    with tempfile.TemporaryDirectory(prefix='pblang-package-contract-') as temp:
        base=Path(temp)
        installed=base/'installed';installed.mkdir();(installed/'tools').mkdir()
        shutil.copytree(repo/'stdlib',installed/'stdlib')
        shutil.copy2(repo/'tools/pbpackage.sapp',installed/'tools/pbpackage.sapp')
        (installed/'main.sapp').write_text('print "fixture";')
        (installed/'pblang.toml').write_text('[package]\nname="installed-package-tool"\nversion="1.0.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path="'+str(installed/'stdlib')+'"}\n')
        built=subprocess.run([str(binary),'build',str(installed)],cwd=installed,capture_output=True,text=True,timeout=45)
        record('installed tool fixture',built.returncode==0,exit=built.returncode,stdout=built.stdout,stderr=built.stderr)
        def case(label,source_arg='absolute',mutate=None,expected=None,dependency=False,flags=(),compete=False,execute=True,mount_alias=False):
            root=base/label;root.mkdir(mode=0o700);source=root/'source';fixtures(source,dependency)
            output=root/'bundle';runtime=binary
            if mutate:
                changed=mutate(source,output,root)
                if isinstance(changed,tuple):output,runtime=changed
            if mount_alias:
                source.chmod(0o700);(root/'alias').mkdir(mode=0o700);output=root/'alias/out'
            if compete:
                for i in range(15):(source/f'z-after-{i:02}.txt').write_text('publication fixture')
            before=tree(source);existing=tree(output) if output.is_dir() and not output.is_symlink() else None
            argument='.' if source_arg=='dot' else './src/..' if source_arg=='nested' else str(source)
            stop=threading.Event();competed=[]
            def conflict():
                while not stop.is_set():
                    if (output/'pblang.toml').is_file():
                        try:
                            with (output/'pblang').open('xb') as f:f.write(b'competing output preserved')
                            competed.append(True);return
                        except FileExistsError:return
                    stop.wait(.001)
            thread=None
            if compete:thread=threading.Thread(target=conflict);thread.start()
            args=[str(binary),'run',str(installed/'tools/pbpackage.sapp'),'--allow-all',*flags,'--',argument,str(output),'--runtime',str(runtime)]
            if mount_alias:
                args=['bwrap','--unshare-all','--die-with-parent','--ro-bind','/usr','/usr','--ro-bind','/bin','/bin','--ro-bind','/lib','/lib','--ro-bind','/lib64','/lib64','--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(binary),str(binary),'--bind',str(base),str(base),'--bind',str(source),str(root/'alias'),'--chdir',str(source),*args]
            started=time.monotonic()
            try:p=subprocess.run(args,cwd=source,capture_output=True,text=True,timeout=90)
            finally:
                stop.set()
                if thread:thread.join(2)
            lines=[line for line in p.stdout.splitlines() if line.startswith('{')]
            result=json.loads(lines[-1]) if lines else None
            unchanged=tree(source)==before
            if expected is None:
                passed=p.returncode==0 and result and result['ok'] and result['complete'] and result['owned_handles_closed'] and unchanged
                record(label,passed,exit=p.returncode,result=result,source_unchanged=unchanged,seconds=round(time.monotonic()-started,3),stderr=p.stderr)
                assert not (output/'.private').exists()
                for name,value in before.items():
                    if name.startswith('.') or name=='pblang.lock':continue
                    assert tree(output)[name]==value,(name,value,tree(output).get(name))
                assert hashlib.sha256((output/'pblang').read_bytes()).digest()==hashlib.sha256(binary.read_bytes()).digest()
                assert (output/'pblang').stat().st_mode&0o777==0o700
                if dependency:assert str(source) not in (output/'pblang.lock').read_text()
                if execute:
                    # Only OS and finished artifact are visible. No checkout or source.
                    sandbox=['bwrap','--unshare-all','--die-with-parent','--ro-bind','/usr','/usr','--ro-bind','/bin','/bin','--ro-bind','/lib','/lib','--ro-bind','/lib64','/lib64','--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(output),str(output),'--chdir',str(output)]
                    run=subprocess.run(sandbox+[str(output/'pblang'),'run','.','--deny-net','--deny-http','--deny-exec','--deny-ffi'],capture_output=True,text=True,timeout=30)
                    record(label+' artifact source-hidden',run.returncode==0 and run.stdout.strip()=='packaged: 42',exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
            else:
                passed=p.returncode!=0 and result and not result['ok'] and result.get('code')==expected and unchanged
                if existing is not None:passed=passed and tree(output)==existing
                if compete:passed=passed and bool(competed) and (output/'pblang').read_bytes()==b'competing output preserved' and result['partial_output_possible']
                record(label,passed,exit=p.returncode,result=result,source_unchanged=unchanged,stderr=p.stderr)
            return root,output
        case('relative','dot');case('absolute');case('nested-relative','nested');case('portable-local-dependency',dependency=True)
        def no_execution(s,o,r):(s/'src/main.sapp').write_text('throw "must not execute while packaging";')
        case('source-not-executed',mutate=no_execution,execute=False)
        case('same-root',mutate=lambda s,o,r:(s,binary),expected='overlap')
        case('inside-source',mutate=lambda s,o,r:(s/'out',binary),expected='overlap')
        case('ancestor-source',mutate=lambda s,o,r:(r,binary),expected='overlap')
        def existing(s,o,r):o.mkdir();(o/'sentinel').write_text('preserve')
        case('existing-output',mutate=existing,expected='output_exists')
        case('output-link',mutate=lambda s,o,r:o.symlink_to(s),expected='output_exists')
        def parent_link(s,o,r):
            (r/'alias').symlink_to(s);return r/'alias/out',binary
        case('parent-link',mutate=parent_link,expected='root_unavailable')
        def source_link(s,o,r):(r/'external').write_text('external');(s/'linked').symlink_to(r/'external')
        case('source-link',mutate=source_link,expected='unsupported_entry')
        case('source-fifo',mutate=lambda s,o,r:os.mkfifo(s/'fifo'),expected='unsupported_entry')
        case('missing-entry',mutate=lambda s,o,r:(s/'src/main.sapp').unlink(),expected='entry_missing')
        case('malformed-manifest',mutate=lambda s,o,r:(s/'pblang.toml').write_text('[package\n'),expected='invalid_metadata')
        case('duplicate-manifest-key',mutate=lambda s,o,r:(s/'pblang.toml').write_text('[package]\nname="a"\nname="b"\nversion="1.0.0"\nentry="src/main.sapp"\n'),expected='invalid_metadata')
        case('traversing-entry',mutate=lambda s,o,r:(s/'pblang.toml').write_text('[package]\nname="a"\nversion="1.0.0"\nentry="../main.sapp"\n'),expected='invalid_relative_path')
        case('nested-package-metadata',mutate=lambda s,o,r:(s/'assets/pblang.toml').write_text('[package]\nname="nested"\n'),expected='nested_metadata')
        case('source-file-limit',mutate=lambda s,o,r:(s/'large.bin').write_bytes(b'x'*1048577),expected='source_unavailable')
        case('stale-dependency',dependency=True,mutate=lambda s,o,r:(s/'dep/nested/value.sapp').write_text('let value=43;export value;'),expected='dependency_hash')
        case('missing-lock',dependency=True,mutate=lambda s,o,r:(s/'pblang.lock').unlink(),expected='lock_missing')
        def wrong_lock_path(s,o,r):
            p=s/'pblang.lock';p.write_text(p.read_text().replace(str(s)+'/dep',str(r)+'/outside'))
        case('wrong-lock-path',dependency=True,mutate=wrong_lock_path,expected='lock_path')
        def duplicate_dependency(s,o,r):
            p=s/'pblang.toml';p.write_text(p.read_text()+'calc={path="dep"}\n')
        case('duplicate-inline-dependency',dependency=True,mutate=duplicate_dependency,expected='invalid_metadata')

        case('remote-dependency',mutate=lambda s,o,r:(s/'pblang.toml').write_text('[package]\nname="a"\nversion="1.0.0"\nentry="src/main.sapp"\n[dependencies]\nx="1.0.0"\n'),expected='unsupported_dependency')
        def bad_runtime(s,o,r):
            p=r/'not-runtime';p.write_text('not ELF');return o,p
        case('non-elf-runtime',mutate=bad_runtime,expected='runtime_format')
        def linked_runtime(s,o,r):
            p=r/'linked-runtime';p.symlink_to(binary);return o,p
        case('runtime-link',mutate=linked_runtime,expected='runtime_unavailable')
        case('missing-runtime',mutate=lambda s,o,r:(o,r/'missing-runtime'),expected='runtime_unavailable')
        case('non-private-parent',mutate=lambda s,o,r:r.chmod(0o755),expected='root_unavailable')
        case('no-ffi',flags=('--deny-ffi',),expected='root_unavailable')
        case('no-exec',flags=('--deny-exec',),expected='identity_unavailable')
        case('bind-alias-parent',mount_alias=True,expected='mount_mismatch')
        case('competing-runtime',compete=True,expected='runtime_publication_failed')
    print('ALL PASS',len(rows),flush=True)

if __name__=='__main__':main()
