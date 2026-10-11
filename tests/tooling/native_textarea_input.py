"""HF03 real typed-input acceptance; requires an isolated Xvfb display."""
import argparse
import json
import os
import shlex
import signal
import subprocess
import time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', required=True, type=Path)
    parser.add_argument('--evidence', required=True, type=Path)
    parser.add_argument('--fixture', type=Path,
                        default=Path(__file__).resolve().parents[1] / 'fixtures/gui_textarea_native.sapp')
    args = parser.parse_args()
    display = os.environ.get('DISPLAY', '').split('.')[0]
    servers = subprocess.check_output(['ps', '-C', 'Xvfb', '-o', 'args='], text=True)
    if not display or not any(display in shlex.split(line)[1:] for line in servers.splitlines()):
        raise RuntimeError('isolated Xvfb required')
    args.evidence.mkdir(mode=0o700)  # Refuse overwrite of earlier evidence.
    output = args.evidence.resolve()
    fixture = args.fixture.resolve()

    def command(*argv):
        return subprocess.check_output(argv, text=True, timeout=5).strip()

    env = dict(os.environ, HF03_OUT=str(output / 'saved.txt'), LANG='C.UTF-8', XMODIFIERS='@im=none')
    child = subprocess.Popen([str(args.runtime.resolve()), 'run', str(fixture), '--allow-all',
                              '--deny-exec', '--deny-net', '--deny-http', '--deny-spawn'],
                             cwd=fixture.parent, env=env, stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE, text=True, start_new_session=True)
    try:
        window = None
        deadline = time.monotonic() + 6
        while time.monotonic() < deadline:
            found = subprocess.run(['xdotool', 'search', '--name', '^HF03 Textarea$'],
                                   capture_output=True, text=True, timeout=2)
            if found.returncode == 0 and found.stdout.strip():
                window = found.stdout.splitlines()[0]
                break
            if child.poll() is not None:
                break
            time.sleep(.03)
        if window is None:
            stdout, stderr = child.communicate(timeout=2)
            (output / 'startup.json').write_text(json.dumps(dict(exit=child.returncode, stdout=stdout, stderr=stderr), indent=2))
            raise RuntimeError('fixture window missing; see startup.json')
        command('setxkbmap', 'gr')
        keymap = command('xmodmap', '-pke')
        if not all(key in keymap.split() for key in ['Greek_alpha', 'Greek_beta', 'Greek_gamma', 'Greek_delta', 'Greek_OMEGA']):
            raise RuntimeError('stable Greek keymap required')
        command('xdotool', 'windowfocus', '--sync', window)
        time.sleep(.15)
        command('xdotool', 'key', '--delay', '1', 'Greek_alpha', 'Greek_beta', 'Return', 'Greek_gamma', 'Greek_delta', 'Left')
        time.sleep(.15)
        command('setxkbmap', 'us')
        command('xdotool', 'key', '--delay', '1', 'X')
        time.sleep(.15)
        subprocess.run(['import', '-window', window, str(output / 'insert.png')], check=True, timeout=5)
        command('setxkbmap', 'gr')
        # Do not request ASCII 'a' in a Greek-only layout: xdotool would create
        # a temporary mapping that the ordered adapter intentionally does not support.
        command('xdotool', 'key', '--delay', '1', 'ctrl+Greek_alpha', 'Greek_OMEGA')
        time.sleep(.15)
        subprocess.run(['import', '-window', window, str(output / 'replacement.png')], check=True, timeout=5)
        stdout, stderr = child.communicate(timeout=8)
        (output / 'receipt.json').write_text(json.dumps(dict(exit=child.returncode, stdout=stdout, stderr=stderr), ensure_ascii=False, indent=2))
        if child.returncode != 0 or stderr:
            raise RuntimeError('fixture failed; see receipt.json')
        result = json.loads(stdout.strip())
        (output / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2))
        if result['input_backend'] != 'x11-ordered-keysyms' or not any(c['value'] == 'αβ\nγXδ' for c in result['changes']):
            raise RuntimeError('backend or intermediate insertion mismatch')
        if not result['value'] == result['reopened'] == (output / 'saved.txt').read_text() == 'Ω':
            raise RuntimeError('Greek selection/replacement/save mismatch')
        print('native_textarea_input_pass')
    finally:
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.communicate(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.communicate()


if __name__ == '__main__':
    main()
