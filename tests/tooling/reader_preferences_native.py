"""Bounded keyboard preference journey, exclusively on an isolated Xvfb display."""
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import time


def main():
    display = os.environ.get('DISPLAY', '').split('.')[0]
    servers = subprocess.check_output(['ps', '-C', 'Xvfb', '-o', 'args='], text=True)
    if not display or not any(display in shlex.split(line)[1:] for line in servers.splitlines()):
        raise SystemExit('Use an isolated xvfb-run display.')
    root = Path(__file__).resolve().parents[2]
    env = dict(os.environ)
    mode = env.get('PBLANG_FOCUS_PROBE', 'after')
    if mode not in ('before', 'after'):
        raise SystemExit('Unsupported focus probe mode')
    for key in ('PBLANG_GRAPHICS_IDLE_PUMP_MS', 'PBLANG_GRAPHICS_MAX_MS', 'PBLANG_GRAPHICS_MAX_FRAMES'):
        env.pop(key, None)
    state_file = root / f'build/reader-focus-{mode}.json'
    state_file.unlink(missing_ok=True)
    child = subprocess.Popen([str(root / 'target/release/pblang'), 'run', 'tests/fixtures/reader_preferences_native.sapp', '--allow-all'], cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
    observations = []

    def x(*args):
        return subprocess.check_output(['xdotool', *map(str, args)], text=True, timeout=6).strip()

    def settled(**expected):
        deadline = time.monotonic() + 8
        state = None
        while time.monotonic() < deadline:
            try:
                state = json.loads(state_file.read_text())
                if all(state.get(k) == v for k, v in expected.items()):
                    observations.append(state)
                    time.sleep(0.2)
                    return
            except (FileNotFoundError, json.JSONDecodeError):
                pass
            if child.poll() is not None:
                raise AssertionError('Native probe stopped before rendering')
            time.sleep(0.05)
        raise AssertionError(('Native state did not settle', expected, state, observations))

    def press(key, hold=0.15):
        x('keydown', key)
        time.sleep(hold)
        x('keyup', key)
        time.sleep(0.15)

    def screenshot(name):
        subprocess.run(['import', '-window', window, str(root / 'build' / name)], check=True, timeout=5)

    try:
        window = x('search', '--sync', '--name', '^PBLang reader preference focus$').splitlines()[0]
        x('windowfocus', '--sync', window)
        settled(settings=False, focus='appearance')
        press('Return', 0.45)
        if mode == 'before':
            settled(settings=True, focus=None)
            for _ in range(4):
                press('Tab')
            settled(focus='font_family')
            press('Return')
            settled(family='serif', focus=None)
            press('Return')
            settled(family='serif', focus=None)
            screenshot('reader-focus-before.png')
        else:
            settled(settings=True, family='sans', focus='font_family')
            press('Return', 0.45)
            settled(family='serif', focus='font_family')
            screenshot('reader-focus-after.png')
            press('Return')
            settled(family='monospace', focus='font_family')
            press('Tab')
            settled(focus='body_size')
            press('Return', 0.45)
            settled(body_size=18, focus='body_size')
            # Keep a physical modifier held over both Tab presses. Composite
            # xdotool chords can synthesize modifier transitions between keys.
            x('keydown', 'Shift_L')
            time.sleep(0.15)
            press('Tab')
            settled(focus='font_family')
            press('Tab')
            settled(focus='appearance')
            x('keyup', 'Shift_L')
            time.sleep(0.15)
            press('Return', 0.45)
            settled(settings=False, family='monospace', body_size=18, focus='appearance', url='pb://guide')
            screenshot('reader-focus-greek.png')
        output, error = child.communicate(timeout=22)
        assert child.returncode == 0, error
        result = json.loads(output.strip().splitlines()[-1])
        result['observations'] = observations
        print(json.dumps(result, ensure_ascii=False))
    except Exception:
        if child.poll() is not None:
            output, error = child.communicate()
            print(output, error, flush=True)
        raise
    finally:
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.communicate()


if __name__ == '__main__':
    main()
