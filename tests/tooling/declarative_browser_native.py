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
    child = subprocess.Popen([str(root / 'target/release/pblang'), 'run', 'tests/fixtures/declarative_browser_native.sapp', '--allow-all'], cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
    def x(*args):
        return subprocess.check_output(['xdotool', *map(str, args)], text=True, timeout=6).strip()
    try:
        window = x('search', '--sync', '--name', '^PBLang document browser probe$').splitlines()[0]
        x('windowfocus', '--sync', window)
        time.sleep(0.4)
        for key in ['Tab', 'Tab', 'Tab', 'Return', 'Tab', 'Return', 'Tab', 'Return']:
            x('keydown', key)
            time.sleep(0.15)
            x('keyup', key)
            time.sleep(0.15)
        # Optional screenshot utility; assertions below are independent of it.
        image = root / 'build/declarative-browser-native.png'
        if subprocess.run(['which', 'import'], stdout=subprocess.DEVNULL).returncode == 0:
            subprocess.run(['import', '-window', window, str(image)], check=True, timeout=5)
        output, error = child.communicate(timeout=8)
        assert child.returncode == 0, error
        result = json.loads(output.strip().splitlines()[-1])
        assert result['url'] == 'pb://guide' and result['navigations'] == 3, result
        print(json.dumps(result, ensure_ascii=False))
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
