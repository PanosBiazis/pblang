"""Synthetic native journey; refuses personal displays and cleans its own process."""
import json
import hashlib
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
    statefile = root / 'build/uno-r4-native-state.json'
    statefile.unlink(missing_ok=True)
    env = dict(os.environ)
    for key in ('PBLANG_GRAPHICS_IDLE_PUMP_MS', 'PBLANG_GRAPHICS_MAX_MS', 'PBLANG_GRAPHICS_MAX_FRAMES'):
        env.pop(key, None)
    importfile = root / 'build/uno-r4-import-test.json'
    import_created = False
    if importfile.exists():
        raise RuntimeError('Refuse to overwrite existing import fixture')
    child = subprocess.Popen([str(root / 'target/release/pblang'), 'run', 'tests/fixtures/uno_r4_led_studio_native.sapp', '--allow-all'], cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
    observations = []

    def x(*args):
        return subprocess.check_output(['xdotool', *map(str, args)], text=True, timeout=5).strip()

    def settled(**expected):
        deadline = time.monotonic() + 6
        state = None
        while time.monotonic() < deadline:
            try:
                state = json.loads(statefile.read_text())
                if all(state.get(k) == v for k, v in expected.items()):
                    observations.append(state)
                    return state
            except (FileNotFoundError, json.JSONDecodeError):
                pass
            if child.poll() is not None:
                raise AssertionError('Window closed early')
            time.sleep(0.05)
        raise AssertionError((expected, state))

    def click(px, py):
        x('mousemove', '--window', window, px, py)
        x('mousedown', 1)
        time.sleep(0.15)
        x('mouseup', 1)
        time.sleep(0.15)

    def shot(name):
        subprocess.run(['import', '-window', window, str(root / 'build' / name)], check=True, timeout=5)

    try:
        window = x('search', '--sync', '--name', '^PBLang UNO R4 LED Studio test$').splitlines()[0]
        x('windowfocus', '--sync', window)
        settled(count=1, playing=False, pixel0=0)
        shot('uno-r4-native-heart.png')
        click(35, 150)
        settled(pixel0=1)
        x('keydown', 'Tab'); time.sleep(.15); x('keyup', 'Tab'); time.sleep(.15)
        x('keydown', 'Return'); time.sleep(.15); x('keyup', 'Return'); time.sleep(.15)
        settled(pixel0=1, pixel1=1)
        shot('uno-r4-native-edited.png')
        # 390px layout: matrix ends at 362, text action starts at 402.
        click(320, 416)
        settled(count=25, index=0, playing=False)
        click(232, 504)
        settled(playing=True)
        time.sleep(0.45)
        click(320, 504)
        settled(playing=False)
        shot('uno-r4-native-stopped.png')
        exports = root / 'build/uno-r4-led-studio'
        before = set(exports.glob('frames-*.json'))
        click(240, 548)
        settled(status='Saved new validated export under build/uno-r4-led-studio/.')
        created = set(exports.glob('frames-*.json')) - before
        assert len(created) == 1
        exported_bytes = created.pop().read_bytes()
        exported = json.loads(exported_bytes)
        expected_digest = hashlib.sha256(exported_bytes).hexdigest()
        assert len(exported['document']['frames']) == 25
        assert len(exported['arduino_frames']) == 25
        for frame, words in zip(exported['document']['frames'], exported['arduino_frames']):
            packed = [int(''.join(str(int(v)) for v in frame['pixels'][i:i+32]), 2) for i in (0,32,64)]
            assert packed + [frame['duration_ms']] == words
        # Import the actual exported document via the native button, then reject
        # malformed input without replacing the imported document.
        with importfile.open('x') as stream:
            stream.write(json.dumps(exported))
        import_created = True
        click(320, 100)  # Clear current frame first.
        click(320, 592)
        settled(count=25, index=0, playing=False, pixel0=0, pixel1=0, export_sha256=expected_digest)
        shot('uno-r4-native-imported.png')
        importfile.write_text('{malformed')
        click(320, 592)
        settled(count=25, index=0, playing=False, pixel0=0, export_sha256=expected_digest, status='Cannot apply: check input, file format and limits.')
        importfile.unlink()
        output, error = child.communicate(timeout=28)
        assert child.returncode == 0, error
        assert 'uno_r4_native_pass' in output
        print(json.dumps({'result': 'pass', 'observations': observations}))
    finally:
        if import_created:
            importfile.unlink(missing_ok=True)
        if child.poll() is not None and child.returncode != 0:
            output, error = child.communicate()
            print(error)
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.communicate()


if __name__ == '__main__':
    main()
