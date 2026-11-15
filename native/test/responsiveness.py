"""Compare real native Pin input latency; run load cases on CI, not the workstation."""
import hashlib
import json
import os
from pathlib import Path
import signal
import statistics
import subprocess
import sys
import time

from PIL import ImageGrab, ImageChops

if os.environ.get('GITHUB_ACTIONS') != 'true':
    raise SystemExit('Run Pin stress measurements on GitHub Actions to keep the workstation quiet.')

binary, output = sys.argv[1:]
root = Path(output).resolve()
root.mkdir(parents=True, exist_ok=True)
env = dict(os.environ, WINIT_X11_SCALE_FACTOR='1', GSETTINGS_BACKEND='keyfile')

def terminate(_signum, _frame):
    raise SystemExit('Interrupted')

signal.signal(signal.SIGTERM, terminate)

def x(*args):
    return subprocess.check_output(['xdotool', *map(str, args)], env=env, text=True, stderr=subprocess.DEVNULL).strip()

def cpu(pid):
    parts = Path(f'/proc/{pid}/stat').read_text().split(') ', 1)[1].split()
    return sum(map(int, parts[11:13])) / os.sysconf('SC_CLK_TCK')

results = []
for count in [20, 200, 1000]:
    case = root / str(count)
    (case / 'config/Pin').mkdir(parents=True, exist_ok=True)
    file = case / 'Pin.md'
    file.write_text(''.join(f'- [ ] Priority {i:04d}: read a paper and explain the result.\n' for i in range(count)))
    (case / 'config/Pin/iced.json').write_text(json.dumps(dict(width=460, height=440, pinned=False)))
    run_env = dict(env, XDG_CONFIG_HOME=str(case / 'config'), PRIORITIES_FILE=str(file))
    with (case / 'log').open('w') as log:
        p = subprocess.Popen([binary], env=run_env, stdout=log, stderr=log, start_new_session=True)
    try:
        deadline = time.monotonic() + 20
        while True:
            try:
                wid = x('search', '--onlyvisible', '--pid', p.pid).splitlines()[0]
                break
            except (subprocess.CalledProcessError, IndexError):
                assert p.poll() is None, (case / 'log').read_text()
                assert time.monotonic() < deadline
                time.sleep(.05)
        x('windowfocus', wid)
        time.sleep(1)
        g = dict(line.split('=', 1) for line in x('getwindowgeometry', '--shell', wid).splitlines())
        gx, gy, gw, gh = [int(g[k]) for k in ['X','Y','WIDTH','HEIGHT']]
        def shot():
            return ImageGrab.grab(xdisplay=env['DISPLAY']).crop((gx, gy, gx+gw, gy+gh))
        shot().save(case / 'window.png')
        x('mousemove', '--window', wid, 130, 73, 'click', 1, 'key', 'End')
        time.sleep(.5)
        latencies=[]
        for _ in range(5):
            before=shot().crop((90, 58, 340, 150))
            began=time.monotonic()
            x('key', 'x')
            while True:
                after=shot().crop((90, 58, 340, 150))
                if ImageChops.difference(before, after).getbbox():
                    break
                assert time.monotonic()-began < 10, 'typing did not paint'
                time.sleep(.005)
            latencies.append((time.monotonic()-began)*1000)
            time.sleep(.15)
        ticks=cpu(p.pid)
        began=time.monotonic()
        # A physical key burst catches queued work, rather than autosave latency.
        x('type', '--delay', 0, 'abcdefghijklmnopqrstuvwxyz'*4)
        x('key', 'ctrl+s')
        deadline=time.monotonic()+20
        while 'abcdefghijklmnopqrstuvwxyz'*4 not in file.read_text():
            assert time.monotonic()<deadline, 'burst did not save'
            time.sleep(.01)
        burst_ms=(time.monotonic()-began)*1000
        burst_cpu=cpu(p.pid)-ticks
        time.sleep(2)
        ticks=cpu(p.pid)
        time.sleep(2)
        idle_cpu=cpu(p.pid)-ticks
        entry=dict(tasks=count, paint_ms=latencies, median_paint_ms=statistics.median(latencies), burst_ms=burst_ms, burst_cpu_s=burst_cpu, idle_cpu_s=idle_cpu)
        results.append(entry)
        print(json.dumps(entry), flush=True)
    finally:
        if p.poll() is None:
            os.killpg(p.pid, signal.SIGTERM)
        p.wait(timeout=5)
(root / 'measurements.json').write_text(json.dumps(dict(binary_sha256=hashlib.sha256(Path(binary).read_bytes()).hexdigest(), cases=results), indent=2))
