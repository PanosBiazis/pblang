"""Real loopback import/export and playback transport checks; synthetic data only."""
from pathlib import Path
from html.parser import HTMLParser
import copy
import json
import os
import signal
import socket
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request


class Form(HTMLParser):
    def __init__(self, page):
        super().__init__()
        self.inputs, self.buttons, self.active = {}, {}, None
        self.feed(page)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'input' and 'name' in attrs:
            self.inputs[attrs['name']] = attrs.get('value', '')
        if tag == 'button' and 'value' in attrs:
            self.active = [attrs['value'], '']

    def handle_data(self, data):
        if self.active is not None:
            self.active[1] += data

    def handle_endtag(self, tag):
        if tag == 'button' and self.active is not None:
            self.buttons[self.active[1].strip()] = self.active[0]
            self.active = None


def main():
    root = Path(__file__).resolve().parents[2]
    origin = 'http://127.0.0.1:18768'
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 18768))
    child = subprocess.Popen([str(root / 'target/release/pblang'), 'run', 'tests/fixtures/uno_r4_led_studio_web.sapp', '--allow-all'], cwd=root, start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    def get(path='/'):
        with opener.open(origin + path, timeout=15) as response:
            return response.read().decode()

    def post(page, label, payload='', origin_header=origin):
        form = Form(page)
        values = dict(form.inputs, action=form.buttons[label], animation=payload)
        request = urllib.request.Request(origin + '/action', urllib.parse.urlencode(values).encode(), {'Origin': origin_header, 'Content-Type': 'application/x-www-form-urlencoded'})
        try:
            with opener.open(request, timeout=30) as response:
                return response.status, response.read().decode()
        except urllib.error.HTTPError as response:
            return response.code, response.read().decode()

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
        original = json.loads(get('/frames.json'))
        assert '<set ' not in page
        document = copy.deepcopy(original)
        frame = document['document']['frames'][0]
        frame['duration_ms'] = 50
        document['document']['frames'] = [copy.deepcopy(frame) for _ in range(64)]
        document['document']['total_ms'] = 3200
        row = document['arduino_frames'][0]
        row[3] = 50
        document['arduino_frames'] = [list(row) for _ in range(64)]
        pretty = json.dumps(document, indent=2)
        assert 65536 < len(pretty.encode()) <= 131072
        code, page = post(page, 'Import JSON', pretty)
        assert code == 200 and 'Frame 1 / 64' in page and '<set ' not in page
        assert json.loads(get('/frames.json')) == document
        snapshot = get('/frames.json')
        for payload in ('{bad', pretty + 'x', 'x' * 131073):
            code, candidate = post(page, 'Import JSON', payload)
            assert code in (200, 400)
            assert get('/frames.json') == snapshot
            if code == 200:
                page = candidate
        assert post(page, 'Import JSON', json.dumps(original), 'https://invalid.example')[0] == 403
        assert get('/frames.json') == snapshot
        code, page = post(page, 'Play once')
        assert code == 200 and 'id="led-playback"' in page
        code, page = post(page, 'Stop playback')
        assert code == 200 and '<set ' not in page
        code, page = post(page, 'Play once')
        code, page = post(page, 'Clear')
        assert code == 200 and '<set ' not in page
        code, page = post(page, 'Import JSON', snapshot)
        assert code == 200 and '<set ' not in page and get('/frames.json') == snapshot
        for frame, words in zip(document['document']['frames'], document['arduino_frames']):
            assert [int(''.join(str(int(v)) for v in frame['pixels'][i:i+32]), 2) for i in (0, 32, 64)] + [frame['duration_ms']] == words
        print(json.dumps({'result': 'pass', 'real_http': True, 'pretty_import_bytes': len(pretty.encode()), 'frames': 64, 'atomic_invalid_imports': True, 'origin_guard': True, 'play_stop_edit_remove_animation': True, 'all_words_independently_verified': True}))
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
