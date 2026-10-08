"""Zero-byte STORE acceptance, complete validation, and narrow FFI resource boundary."""
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile

repo = Path(__file__).resolve().parents[2]
runtime = str(Path(sys.argv[1]).resolve())
symbols = ['free','close','strdup','openat','getpid','dup','memfd_create','sendfile','lseek','fchmod','fcntl','fsync']
# No calloc/pread/write FFI grants: the empty path must skip the copy buffer.
flags = ['--deny-'+s for s in ['net','http','spawn','gfx','sqlite']] + ['--allow-exec','--ffi-lib=/usr/lib/x86_64-linux-gnu/libc.so.6'] + ['--ffi-symbol='+s for s in symbols]
checks = []
def archive(rows, method=zipfile.ZIP_STORED):
    b=io.BytesIO()
    with zipfile.ZipFile(b,'w',method) as z:
        for name,data in rows:z.writestr(name,data)
    return b.getvalue()
with tempfile.TemporaryDirectory(prefix='archive-empty-') as tmp:
    root=Path(tmp)
    def run(label,data,ok=False,name='empty',dest=None):
        src=root/'input.zip';src.write_bytes(data)
        dst=dest if dest is not None else root/label
        p=subprocess.run([runtime,'run',str(repo/'tools/archive_extract.sapp'),*flags,'--','--extract',str(src),name,str(dst)],cwd=repo,capture_output=True,text=True,timeout=10,env={'PATH':'/usr/bin:/bin','LANG':'C.UTF-8'})
        assert p.returncode==(0 if ok else 1) and p.stderr=='',(label,p.stdout,p.stderr)
        r=json.loads(p.stdout);assert r['ok']==ok and str(root) not in p.stdout
        assert src.read_bytes()==data
        if ok:assert dst.is_file() and dst.read_bytes()==b'' and dst.stat().st_mode&0o777==0o600 and r['durability_verified']
        elif dest is None:assert not dst.exists() and not r['publication_attempted'],(label,r)
        checks.append(label);return r,dst
    good=archive([('empty',b''),('later',b'valid')])
    _,dst=run('zero-no-copy-grants',good,True)
    before=dst.stat()
    run('zero-collision',good,dest=dst)
    assert dst.stat().st_ino==before.st_ino and dst.stat().st_mtime_ns==before.st_mtime_ns and dst.read_bytes()==b''
    corrupt=bytearray(good)
    with zipfile.ZipFile(io.BytesIO(good)) as z:off=z.getinfo('later').header_offset
    corrupt[off+30+len('later')]^=1
    run('unselected-crc',corrupt)
    crc=bytearray(good);central=crc.index(b'PK\x01\x02');crc[14]=1;crc[central+16]=1
    run('empty-crc',crc)
    run('truncated',good[:-1])
    run('oversized-input',b'x'*4194305)
    for label,name in [('traversal','../escape'),('case-collision','EMPTY'),('ancestor','empty/child')]:run(label,archive([('empty',b''),(name,b'bad')]))
    link=zipfile.ZipInfo('empty');link.create_system=3;link.external_attr=0o120777<<16
    run('empty-symlink',archive([(link,b'')]))
    run('directory',archive([('empty/',b'')]),name='empty/')
    run('empty-deflate',archive([('empty',b'')],zipfile.ZIP_DEFLATED))
    run('unselected-expanded-limit',archive([('empty',b''),('later',b'x'*1048577)],zipfile.ZIP_DEFLATED))
    run('entry-limit',archive([('empty',b'')]+[(f'f{i}',b'') for i in range(256)]))
    # Even zero-size selected data cannot skip encrypted/unselected policy checks.
    encrypted=bytearray(good);encrypted[6]|=1;encrypted[central+8]|=1
    run('encrypted-empty',encrypted)
print(json.dumps({'passed':checks,'count':len(checks),'copy_ffi_symbols_denied':['calloc','pread','write'],'profile_symbols':symbols}))
