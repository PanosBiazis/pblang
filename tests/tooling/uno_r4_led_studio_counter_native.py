"""Synthetic counter journey on an isolated Xvfb, never the user's desktop."""
import hashlib
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
    assert display and any(display in shlex.split(line)[1:] for line in servers.splitlines()), 'Use isolated xvfb-run'
    root = Path(__file__).resolve().parents[2]
    statefile = root / 'build/uno-counter-native-state.json'
    importfile = root / 'build/uno-counter-import.json'
    assert not importfile.exists(), 'Existing import file must not be overwritten'
    statefile.unlink(missing_ok=True)
    env = dict(os.environ)
    for key in ('PBLANG_GRAPHICS_IDLE_PUMP_MS', 'PBLANG_GRAPHICS_MAX_MS', 'PBLANG_GRAPHICS_MAX_FRAMES'):
        env.pop(key, None)
    child = subprocess.Popen([str(root / 'target/release/pblang'), 'run', 'tests/fixtures/uno_r4_led_studio_counter_native.sapp', '--allow-all'], cwd=root, env=env, start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    observations = []
    created_import = False

    def x(*args):
        return subprocess.check_output(['xdotool', *map(str, args)], text=True, timeout=5).strip()

    def state(**expected):
        deadline = time.monotonic() + 6
        current = None
        while time.monotonic() < deadline:
            try:
                current = json.loads(statefile.read_text())
                if all(current.get(k) == v for k, v in expected.items()):
                    observations.append(current)
                    return current
            except (FileNotFoundError, json.JSONDecodeError):
                pass
            if child.poll() is not None:
                output, error = child.communicate()
                raise AssertionError(('Native fixture ended early', child.returncode, current, output, error))
            time.sleep(.05)
        raise AssertionError((expected, current))

    def click(px, py):
        x('mousemove', '--window', window, px, py); x('mousedown', 1); time.sleep(.15); x('mouseup', 1); time.sleep(.15)

    def key(name):
        x('keydown', name); time.sleep(.15); x('keyup', name); time.sleep(.15)

    def replace(value):
        x('keydown', 'Control_L'); time.sleep(.15); key('a'); x('keyup', 'Control_L'); time.sleep(.15)
        for char in value:
            key(char)

    try:
        window = x('search', '--sync', '--name', '^PBLang UNO counter test$').splitlines()[0]
        x('windowfocus', '--sync', window)
        state(count=2, playing=False, index=1)
        click(35, 150)
        authored = state()['frames_sha']
        click(200, 688); replace('3'); state(counter_end='3')
        click(60, 724)
        saved = state(playing=False, value=0)['project_sha']
        click(150, 724)
        state(playing=True, source='counter', value=1, frames_sha=authored, pixel_enabled=False)
        click(35, 150)
        assert state()['frames_sha'] == authored
        click(320, 504)
        stopped = state(playing=False, source='frames', frames_sha=authored, pixel_enabled=True)
        assert stopped['value'] in (1, 2, 3)
        click(235, 724)
        captured = state(playing=False, count=2, index=1)
        assert captured['frames_sha'] != authored
        exports = root / 'build/uno-r4-led-studio'
        before = set(exports.glob('project-*.json'))
        click(320, 548)
        state(status='Saved new validated export under build/uno-r4-led-studio/.')
        created = set(exports.glob('project-*.json')) - before
        assert len(created) == 1
        raw = created.pop().read_bytes()
        project = json.loads(raw)
        assert project['format'] == 'pblang.uno-r4-led-project.v2'
        assert project['generator'] == {'kind': 'counter', 'start': 0, 'end': 3, 'interval_ms': 1000}
        with importfile.open('xb') as stream:
            stream.write(raw)
        created_import = True
        click(320, 724); state(generator=None, playing=False)
        click(320, 592)
        state(project_sha=hashlib.sha256(raw).hexdigest(), counter_end='3', value=0, playing=False)
        snapshot = state()['project_sha']
        click(300, 688); replace('0'); state(counter_seconds='0')
        click(60, 724)
        state(project_sha=snapshot, playing=False, counter_seconds='0')
        subprocess.run(['import', '-window', window, str(root / 'build/uno-counter-native.png')], check=True, timeout=5)
        output, error = child.communicate(timeout=30)
        assert child.returncode == 0 and 'uno_r4_led_studio_counter_native_pass' in output, (output, error)
        print(json.dumps({'result': 'pass', 'static_authored_frames_preserved_during_source': True, 'pixel_edit_disabled_during_source': True, 'capture_explicit': True, 'project_file_roundtrip': True, 'invalid_settings_atomic': True, 'observations': len(observations)}))
    finally:
        if created_import:
            importfile.unlink(missing_ok=True)
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL); child.communicate()


if __name__ == '__main__':
    main()
