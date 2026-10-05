"""A headless admission must still reject an altered, unused stdlib GUI module."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path


def snapshot(root):
    return {p.relative_to(root).as_posix():hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob('*') if p.is_file()}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package',required=True,type=Path)
    parser.add_argument('--evidence',required=True,type=Path)
    args=parser.parse_args();package=args.package.resolve();before=snapshot(package)
    evidence=args.evidence.absolute();evidence.mkdir(mode=0o700)
    with tempfile.TemporaryDirectory(prefix='reader-admission-integrity-') as directory:
        probe=Path(directory)
        (probe/'input.html').write_text('<article><h1>PRIVATE_TITLE</h1><p>Test.</p></article>')
        (probe/'gui.sapp').write_bytes((package/'stdlib/gui.sapp').read_bytes()+b'\n// synthetic integrity mismatch\n')
        command=['bwrap','--unshare-all','--die-with-parent','--ro-bind','/usr','/usr',
                 '--ro-bind','/bin','/bin','--ro-bind','/lib','/lib','--ro-bind','/lib64','/lib64',
                 '--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(package),'/app',
                 '--ro-bind',str(probe),'/probe','--ro-bind',str(probe/'gui.sapp'),'/app/stdlib/gui.sapp',
                 '--chdir','/tmp','--clearenv','--setenv','PATH','/usr/bin:/bin','--setenv','HOME','/tmp',
                 '/bin/sh','/app/run-reader.sh','--check','--open','/probe/input.html']
        result=subprocess.run(command,capture_output=True,text=True,timeout=8)
        output=result.stdout+result.stderr
        assert result.returncode!=0 and 'content hash mismatch' in output,(result.returncode,output)
        assert 'PRIVATE_TITLE' not in output
        assert before==snapshot(package),'Real package changed'
        (evidence/'rejection.log').write_text(output)
        report=dict(passed=True,headless=True,source_hidden=True,unused_shared_gui_tamper_rejected=True,
                    original_package_unchanged=True,input_source_not_disclosed=True,exit=result.returncode)
        (evidence/'acceptance.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))


if __name__=='__main__':main()
