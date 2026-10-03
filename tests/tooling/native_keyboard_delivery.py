"""Real X11 input regression; invoke only inside an isolated xvfb-run display."""
import collections
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import time


def x(*args):
    return subprocess.check_output(
        ['xdotool', *map(str, args)], text=True, timeout=6).strip()


def main():
    display = os.environ.get('DISPLAY', '').split('.')[0]
    servers = subprocess.check_output(['ps', '-C', 'Xvfb', '-o', 'args='], text=True)
    if not display or not any(display in shlex.split(line)[1:] for line in servers.splitlines()):
        raise SystemExit('Use xvfb-run: this test must not control a personal desktop.')
    root = Path(__file__).resolve().parents[2]
    results = []
    sequence = ['Tab', 'space', 'Tab', *['period' if c == '.' else c for c in 'native.pbsheet'], 'Tab', 'Return']
    expected = collections.Counter('Enter' if k == 'Return' else 'Space' if k == 'space' else 'Period' if k == 'period' else k.upper() if len(k) == 1 else k for k in sequence)
    for hold in (0.08, 0.12, 0.18):
        env = dict(os.environ)
        for name in ('PBLANG_GRAPHICS_IDLE_PUMP_MS', 'PBLANG_GRAPHICS_MAX_MS', 'PBLANG_GRAPHICS_MAX_FRAMES'):
            env.pop(name, None)
        child = subprocess.Popen(
            [str(root / 'target/release/pblang'), 'run', 'tests/fixtures/native_keyboard_delivery.sapp', '--allow-all'],
            cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, start_new_session=True)
        try:
            window = x('search', '--sync', '--name', '^PBLang Keyboard Delivery$').splitlines()[0]
            x('windowfocus', '--sync', window)
            time.sleep(0.5)
            for key in sequence:
                x('keydown', key)
                time.sleep(hold)
                x('keyup', key)
                time.sleep(hold)
            output, error = child.communicate(timeout=8)
            assert child.returncode == 0, error
            value = json.loads(output.strip().splitlines()[-1])
            assert value['text'] == 'native.pbsheet', value
            assert (value['toggles'], value['buttons'], value['disabled']) == (1, 1, 0), value
            assert value['checked'] and value['focus'] == 'done', value
            for kind in ('keydown', 'keyup'):
                actual = collections.Counter(e['key'] for e in value['events'] if e['type'] == kind)
                assert actual == expected, (kind, actual, expected)
            results.append({'hold_ms': round(hold * 1000), 'text': value['text'],
                            'draws': value['draws'], 'presents': value['presents'],
                            'transitions_once': True, 'controls_pass': True})
        finally:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.communicate(timeout=3)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.communicate()
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()
