"""Read-only source-hidden GUI journey through an actual installed reader launcher.

Review screenshots separately: this driver does not certify their visual content.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
from reader_package import LAUNCH_PROBE, snapshot


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root',type=Path,required=True)
    parser.add_argument('--bin-dir',type=Path,required=True)
    parser.add_argument('--evidence',type=Path,required=True)
    args=parser.parse_args()
    root=args.root.resolve(); bin_dir=args.bin_dir.resolve(); evidence=args.evidence.absolute()
    evidence.mkdir(mode=0o700)
    launcher=bin_dir/'pb-document-reader'; original=launcher.read_bytes()
    lines=original.decode().splitlines(); assert lines[1]=='# pblang.reader.install.v1'
    version=lines[2].removeprefix('# current='); assert '/' not in version and '..' not in version
    release=root/'releases'/version; before=snapshot(release)
    repo=Path(__file__).resolve().parents[2]
    with tempfile.TemporaryDirectory(prefix='reader-installed-probe-') as tmp:
        probe=Path(tmp); (probe/'launch.py').write_text(LAUNCH_PROBE)
        sandbox=['bwrap','--unshare-all','--die-with-parent','--ro-bind','/usr','/usr',
                 '--ro-bind','/bin','/bin','--ro-bind','/lib','/lib','--ro-bind','/lib64','/lib64',
                 '--ro-bind','/etc/fonts','/etc/fonts','--proc','/proc','--dev','/dev',
                 '--tmpfs','/tmp','--dir','/tmp/foreign','--ro-bind',str(release),str(release),
                 '--ro-bind',str(launcher),str(launcher),'--bind',str(probe),'/probe',
                 '--chdir','/tmp/foreign','--clearenv','--setenv','PATH','/usr/bin:/bin',
                 '--setenv','HOME','/tmp','--setenv','LANG','C.UTF-8']
        for system in ('/etc/alternatives','/etc/ImageMagick-6','/etc/ImageMagick-7'):
            if Path(system).is_dir():sandbox+=['--ro-bind',system,system]
        commands={
            'source-hidden':['/bin/sh','-c','test ! -e "$1" && test ! -e "$2/data"','sh',str(repo),str(root)],
            'offline':[str(launcher),'--check'],
            'gui':['xvfb-run','-a','-s','-screen 0 1000x1000x24 -nolisten tcp',
                   'env','XMODIFIERS=@im=none','python3','/probe/launch.py',str(launcher)]}
        for name,command in commands.items():
            result=subprocess.run(sandbox+command,capture_output=True,text=True,timeout=55)
            (evidence/(name+'.log')).write_text(result.stdout+result.stderr)
            for screenshot in probe.glob('reader-*.png'):shutil.copy2(screenshot,evidence/screenshot.name)
            assert result.returncode==0,(name,result.stdout,result.stderr)
        for screenshot in probe.glob('reader-*.png'):shutil.copy2(screenshot,evidence/screenshot.name)
    assert original==launcher.read_bytes() and before==snapshot(release)
    result=dict(passed=True,version=version,source_hidden=True,user_data_not_mounted=True,
                installed_launcher=True,content_unchanged=True,style_pagination_keys_exercised=True,
                visual_review_required=True,launcher_sha256=hashlib.sha256(original).hexdigest())
    (evidence/'acceptance.json').write_text(json.dumps(result,indent=2));print(json.dumps(result))


if __name__=='__main__':main()
