"""Measure focused and background idle at both render scales, without generated input."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

if os.environ.get('GITHUB_ACTIONS') != 'true':
    raise SystemExit('Run Pin idle measurements on GitHub Actions.')

def terminate(_signum, _frame):
    raise SystemExit('Interrupted')

signal.signal(signal.SIGTERM, terminate)

binary = str(Path(sys.argv[1]).resolve())
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)

def x(env, *args):
    return subprocess.check_output(['xdotool', *map(str, args)], env=env, text=True, stderr=subprocess.DEVNULL).strip()

def cpu(pid):
    stat = Path(f'/proc/{pid}/stat').read_text().split(') ', 1)[1].split()
    return sum(map(int, stat[11:13])) / os.sysconf('SC_CLK_TCK')

results = []
for scale in [1, 2]:
    case = out / str(scale)
    config = case / 'config/Pin'
    config.mkdir(parents=True, exist_ok=True)
    file = case / 'Pin.md'
    file.write_text(''.join(f'- [ ] ==🟢Priority {i}: read a paper and explain the result.==\n' for i in range(20)))
    (config / 'iced.json').write_text(json.dumps(dict(width=1213, height=884, x=80, y=60, pinned=False)))
    env = dict(os.environ, XDG_CONFIG_HOME=str(case / 'config'), PRIORITIES_FILE=str(file), WINIT_X11_SCALE_FACTOR=str(scale), GSETTINGS_BACKEND='keyfile')
    with (case / 'log').open('w') as log:
        p = subprocess.Popen([binary], env=env, stdout=log, stderr=log, start_new_session=True)
    try:
        deadline = time.monotonic() + 20
        while True:
            try:
                wid = x(env, 'search', '--onlyvisible', '--pid', p.pid).splitlines()[0]
                break
            except (subprocess.CalledProcessError, IndexError):
                assert p.poll() is None, (case / 'log').read_text()
                assert time.monotonic() < deadline
                time.sleep(.05)
        time.sleep(.5)
        x(env, 'windowsize', wid, 1213*scale, 884*scale)
        time.sleep(.5)
        geometry = x(env, 'getwindowgeometry', '--shell', wid)
        dimensions = dict(line.split('=', 1) for line in geometry.splitlines())
        assert int(dimensions['WIDTH']) == 1213*scale and int(dimensions['HEIGHT']) == 884*scale, geometry
        for focused in [True, False]:
            x(env, 'windowfocus', wid if focused else 0)
            time.sleep(2)
            before, began = cpu(p.pid), time.monotonic()
            time.sleep(15)
            elapsed, used = time.monotonic() - began, cpu(p.pid) - before
            entry = dict(scale=scale, focused=focused, cpu_s=used, elapsed_s=elapsed, cpu_percent_one_core=100*used/elapsed, geometry=geometry)
            results.append(entry)
            print(json.dumps(entry), flush=True)
    finally:
        if p.poll() is None:
            os.killpg(p.pid, signal.SIGTERM)
        p.wait(timeout=5)
(out / 'results.json').write_text(json.dumps(results, indent=2))
if '--baseline' not in sys.argv[3:]:
    assert all(r['cpu_percent_one_core'] <= (2 if r['focused'] else 1) for r in results), results
