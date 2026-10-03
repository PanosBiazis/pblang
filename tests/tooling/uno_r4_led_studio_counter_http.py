"""Real loopback generator workflow; explicit local clock source, no device I/O."""
from pathlib import Path
import hashlib
import json
import os
import signal
import socket
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
from uno_r4_led_studio_http import Form


def main():
    root = Path(__file__).resolve().parents[2]
    origin = 'http://127.0.0.1:18773'
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 18773))
    child = subprocess.Popen([str(root / 'target/release/pblang'), 'run', 'tests/fixtures/uno_r4_led_studio_counter_web.sapp', '--allow-all'], cwd=root, start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    def get(path='/'):
        with opener.open(origin + path, timeout=15) as response:
            return response.read().decode()

    def post(page, label, **updates):
        form = Form(page)
        values = dict(form.inputs, action=form.buttons[label], animation='', workspace='dynamic')
        values.update(updates)
        request = urllib.request.Request(origin + '/action', urllib.parse.urlencode(values).encode(), {'Origin': origin, 'Content-Type': 'application/x-www-form-urlencoded'})
        with opener.open(request, timeout=15) as response:
            return response.read().decode()

    try:
        deadline = time.monotonic() + 10
        while True:
            try:
                page = get()
                break
            except OSError:
                if time.monotonic() >= deadline or child.poll() is not None:
                    raise
                time.sleep(.1)
        page = post(page, 'Duplicate frame')
        page = post(page, 'Invert')
        authored = get('/frames.json')
        page = post(page, 'Start counter', counter_start='7', counter_end='9', counter_seconds='1')
        assert 'Counter 07' in page and 'http-equiv="refresh"' in page and '<set ' not in page
        project = get('/project.json')
        saved = json.loads(project)
        assert saved['generator'] == {'kind': 'counter', 'start': 7, 'end': 9, 'interval_ms': 1000}
        token = Form(page).inputs['csrf']
        time.sleep(1.15)
        page = get()
        assert 'Counter 08' in page and Form(page).inputs['csrf'] == token
        assert get('/frames.json') == authored and get('/project.json') == project
        page = post(page, 'Stop counter')
        assert 'http-equiv="refresh"' not in page and get('/frames.json') == authored
        time.sleep(1.1)
        page = get()
        assert 'http-equiv="refresh"' not in page and 'last value 08' in page
        page = post(page, 'Capture last value → selected frame')
        captured = json.loads(get('/frames.json'))
        original = json.loads(authored)
        assert len(captured['document']['frames']) == 2
        assert captured['document']['frames'][0] == original['document']['frames'][0]
        assert captured['document']['frames'][1]['duration_ms'] == original['document']['frames'][1]['duration_ms']
        assert captured['document']['frames'][1]['pixels'] != original['document']['frames'][1]['pixels']
        page = post(page, 'Import JSON', animation=project)
        assert 'http-equiv="refresh"' not in page and get('/project.json') == project and get('/frames.json') == authored
        page = post(page, 'Save counter settings', counter_start='7', counter_end='9', counter_seconds='0')
        assert 'Counter: values 0–99' in page and get('/project.json') == project
        page = post(page, 'Start counter', counter_start='9', counter_end='9', counter_seconds='1')
        assert 'Counter 09' in page
        time.sleep(1.15)
        page = get()
        assert 'Counter complete at 09' in page and 'http-equiv="refresh"' not in page
        page = post(page, 'Import JSON', animation=authored)
        assert json.loads(get('/project.json'))['generator'] is None
        assert get('/frames.json') == authored
        print(json.dumps({'result': 'pass', 'live_server_samples': ['07', '08'], 'stable_stop_token': True, 'stop_freezes_source': True, 'authored_frames_preserved': True, 'project_roundtrip': True, 'legacy_roundtrip': True, 'explicit_capture_only_current_frame': True, 'finite_end': True, 'authored_sha256': hashlib.sha256(authored.encode()).hexdigest()}))
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
