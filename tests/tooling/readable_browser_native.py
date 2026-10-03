"""Exercise native local-link/back/forward via XTEST, only inside xvfb-run."""
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
    for key in ('PBLANG_GRAPHICS_IDLE_PUMP_MS', 'PBLANG_GRAPHICS_MAX_MS', 'PBLANG_GRAPHICS_MAX_FRAMES'):
        env.pop(key, None)
    child = subprocess.Popen([str(root / 'target/release/pblang'), 'run', 'tests/fixtures/readable_browser_native.sapp', '--allow-all'], cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
    def x(*args):
        return subprocess.check_output(['xdotool', *map(str, args)], text=True, timeout=6).strip()
    width = env.get('PBLANG_READER_WIDTH', '800')
    state_file = root / ('build/reader-probe-' + width + '.json')
    state_file.unlink(missing_ok=True)
    observations = []
    def settled(**expected):
        deadline = time.monotonic() + 12
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
        raise AssertionError(('Native state did not settle', expected))
    def press(*keys):
        for key in keys:
            x('keydown', key)
            time.sleep(0.15)
            x('keyup', key)
            time.sleep(0.15)
    try:
        window = x('search', '--sync', '--name', '^PBLang readable browser probe$').splitlines()[0]
        x('windowfocus', '--sync', window)
        settled(navigations=0, settings=False)
        press('Tab', 'Tab', 'Tab', 'Return')
        settled(navigations=1, url='pb://guide')
        press('Tab', 'Return')
        settled(navigations=2, url='pb://home')
        press('Tab', 'Return')
        settled(navigations=3, url='pb://guide')
        subprocess.run(['import', '-window', window, str(root / ('build/reader-after-' + width + '.png'))], check=True, timeout=5)
        press('Tab', 'Tab', 'Tab', 'Return')
        settled(settings=True, family='sans')
        subprocess.run(['import', '-window', window, str(root / ('build/reader-settings-' + width + '.png'))], check=True, timeout=5)
        press('Tab', 'Tab', 'Tab', 'Tab', 'Return')
        settled(settings=True, family='serif')
        press('Tab', 'Tab', 'Tab', 'Return')
        settled(settings=False, family='serif')
        subprocess.run(['import', '-window', window, str(root / ('build/reader-serif-' + width + '.png'))], check=True, timeout=5)
        output, error = child.communicate(timeout=26)
        assert child.returncode == 0, error
        result = json.loads(output.strip().splitlines()[-1])
        assert result['url'] == 'pb://guide' and result['navigations'] == 3, result
        assert result['typography']['requested_family'] == 'serif' and result['natural_width'] < result['fixed_width'], result
        result["observations"] = observations
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
