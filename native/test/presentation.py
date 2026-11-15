"""Check display-server damage, including the user's doubled render size."""
import ctypes as c
import hashlib
import json
import math
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import time


class Rect(c.Structure):
    _fields_ = [('x', c.c_short), ('y', c.c_short), ('width', c.c_ushort), ('height', c.c_ushort)]


class DamageEvent(c.Structure):
    _fields_ = [('type', c.c_int), ('serial', c.c_ulong), ('send_event', c.c_int),
                ('display', c.c_void_p), ('drawable', c.c_ulong), ('damage', c.c_ulong),
                ('level', c.c_int), ('more', c.c_int), ('timestamp', c.c_ulong),
                ('area', Rect), ('geometry', Rect)]


class Damage:
    def __init__(self, window):
        self.x = c.CDLL('libX11.so.6')
        self.d = c.CDLL('libXdamage.so.1')
        signatures = {
            'XOpenDisplay': ([c.c_char_p], c.c_void_p),
            'XConnectionNumber': ([c.c_void_p], c.c_int),
            'XSync': ([c.c_void_p, c.c_int], c.c_int),
            'XPending': ([c.c_void_p], c.c_int),
            'XNextEvent': ([c.c_void_p, c.c_void_p], c.c_int),
            'XCloseDisplay': ([c.c_void_p], c.c_int),
        }
        for name, (args, result) in signatures.items():
            getattr(self.x, name).argtypes = args
            getattr(self.x, name).restype = result
        self.d.XDamageQueryExtension.argtypes = [c.c_void_p, c.POINTER(c.c_int), c.POINTER(c.c_int)]
        self.d.XDamageCreate.argtypes = [c.c_void_p, c.c_ulong, c.c_int]
        self.d.XDamageCreate.restype = c.c_ulong
        self.d.XDamageDestroy.argtypes = [c.c_void_p, c.c_ulong]
        self.display = self.x.XOpenDisplay(None)
        assert self.display
        self.base, error = c.c_int(), c.c_int()
        assert self.d.XDamageQueryExtension(self.display, c.byref(self.base), c.byref(error))
        self.damage = self.d.XDamageCreate(self.display, int(window), 0)
        self.fd = self.x.XConnectionNumber(self.display)
        self.clear()

    def read(self, seconds):
        events = []
        deadline = time.monotonic() + seconds
        while True:
            while self.x.XPending(self.display):
                raw = (c.c_long * 24)()
                self.x.XNextEvent(self.display, c.byref(raw))
                event = c.cast(c.byref(raw), c.POINTER(DamageEvent)).contents
                if event.type == self.base.value:
                    r = event.area
                    events.append(dict(x=r.x, y=r.y, width=r.width, height=r.height))
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return events
            select.select([self.fd], [], [], remaining)

    def clear(self):
        self.x.XSync(self.display, 0)
        self.read(0)

    def close(self):
        self.d.XDamageDestroy(self.display, self.damage)
        self.x.XCloseDisplay(self.display)


def terminate(_signum, _frame):
    raise SystemExit('Interrupted')


signal.signal(signal.SIGTERM, terminate)

# Read-only observation is safe locally; generated interaction cases stay on CI.
if len(sys.argv) > 1 and sys.argv[1] == '--observe':
    watcher = Damage(sys.argv[2])
    try:
        events = watcher.read(float(sys.argv[3]))
        Path(sys.argv[4]).write_text(json.dumps(events, indent=2))
        print(json.dumps(dict(events=len(events),max_pixels=max((r['width']*r['height'] for r in events), default=0))))
    finally:
        watcher.close()
    raise SystemExit(0)

if os.environ.get('GITHUB_ACTIONS') != 'true':
    raise SystemExit('Run generated Pin presentation checks on GitHub Actions.')

from PIL import ImageGrab, ImageChops, ImageStat

binary = str(Path(sys.argv[1]).resolve())
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
baseline = '--baseline' in sys.argv[3:]


def x(env, *args):
    return subprocess.check_output(['xdotool', *map(str, args)], env=env, text=True, stderr=subprocess.DEVNULL, timeout=10).strip()


def geometry(env, wid):
    return {k: int(v) for k, v in (line.split('=', 1) for line in x(env, 'getwindowgeometry', '--shell', wid).splitlines())}


def cpu(pid):
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(') ', 1)[1].split()
    return sum(map(int, fields[11:13])) / os.sysconf('SC_CLK_TCK')


results = []
for scale in [1, 1.25, 2]:
    case = out / str(scale)
    config = case / 'config/Pin'
    config.mkdir(parents=True, exist_ok=True)
    file = case / 'Pin.md'
    file.write_text(''.join(f'- [ ] ==🟢Priority {i}: read a paper and explain the result.==\n' for i in range(20)))
    (config / 'iced.json').write_text(json.dumps(dict(width=1213, height=869, x=20, y=20, pinned=False)))
    env = dict(os.environ, XDG_CONFIG_HOME=str(case / 'config'), PRIORITIES_FILE=str(file), WINIT_X11_SCALE_FACTOR=str(scale), GSETTINGS_BACKEND='keyfile')
    with (case / 'log').open('w') as log:
        p = subprocess.Popen([binary], env=env, stdout=log, stderr=log, start_new_session=True)
    watcher = None
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
        x(env, 'windowsize', wid, round(1213*scale), round(869*scale), 'windowmove', wid, 20, 20, 'windowfocus', wid)
        time.sleep(.7)
        g = geometry(env, wid)
        assert (g['WIDTH'], g['HEIGHT']) == (round(1213*scale), round(869*scale)), g
        x(env, 'mousemove', '--window', wid, round(150*scale), round(74*scale), 'click', 1, 'key', 'End')
        time.sleep(2)
        assert int(x(env, 'getwindowfocus', '-f')) == int(wid)
        watcher = Damage(wid)
        entry = dict(scale=scale, width=g['WIDTH'], height=g['HEIGHT'], cases={})

        def record(name, action=None, seconds=1.2):
            watcher.clear()
            display_pids = [int(pid) for pid in os.environ.get('PIN_DISPLAY_PIDS', '').split()]
            before_cpu, before_display = cpu(p.pid), sum(cpu(pid) for pid in display_pids)
            began = time.monotonic()
            if action:
                action()
            events = watcher.read(seconds)
            elapsed = time.monotonic() - began
            assert events, (name, scale, 'No display update observed')
            pixels = [r['width']*r['height'] for r in events]
            entry['cases'][name] = dict(events=events, total_pixels=sum(pixels), max_pixels=max(pixels), max_window_fraction=max(pixels)/(g['WIDTH']*g['HEIGHT']), elapsed_s=elapsed, app_cpu_percent=100*(cpu(p.pid)-before_cpu)/elapsed, display_cpu_percent=100*(sum(cpu(pid) for pid in display_pids)-before_display)/elapsed)
            print(json.dumps(dict(scale=scale,case=name,events=len(events),max_window_fraction=entry['cases'][name]['max_window_fraction'])), flush=True)

        record('caret')
        record('typing', lambda: x(env, 'type', '--delay', 80, 'abcde'))
        record('hover', lambda: x(env, 'mousemove', '--window', wid, g['WIDTH']-round(100*scale), round(74*scale)))

        def motion(kind):
            gx, gy = g['X'], g['Y']
            start = (gx+round(45*scale), gy+round(20*scale)) if kind == 'window' else (gx+g['WIDTH']-round(58*scale), gy+round(74*scale))
            x(env, 'mousemove', *start, 'mousedown', 1)
            began = time.monotonic()
            try:
                for step in range(60):
                    point = (start[0]+round(20*scale*math.sin(step/10)), start[1]+round(10*scale*math.cos(step/10))) if kind == 'window' else (start[0], gy+round((74+(step%40)*3)*scale))
                    x(env, 'mousemove', *point)
                    time.sleep(max(0, began+(step+1)/60-time.monotonic()))
            finally:
                x(env, 'mousemove', 0, 0, 'mouseup', 1)
                if kind == 'window':
                    x(env, 'windowmove', wid, gx, gy)

        record('window', lambda: motion('window'))
        record('task', lambda: motion('task'))
        x(env, 'key', 'ctrl+s')
        deadline = time.monotonic() + 3
        while 'abcde' not in file.read_text():
            assert time.monotonic() < deadline, 'Typed text was not saved'
            time.sleep(.05)

        def shot():
            return ImageGrab.grab(xdisplay=env['DISPLAY']).crop((g['X'], g['Y'], g['X']+g['WIDTH'], g['Y']+g['HEIGHT']))

        x(env, 'mousemove', 0, 0)
        time.sleep(.5)
        before = shot()
        before.save(case / 'before-minimize.png')
        x(env, 'windowminimize', wid)
        time.sleep(.3)
        x(env, 'windowactivate', '--sync', wid)
        time.sleep(.7)
        after = shot()
        after.save(case / 'after-restore.png')
        # Caret phase may differ; missing task cards or stale pixels cannot.
        difference = sum(ImageStat.Stat(ImageChops.difference(before, after)).mean) / (3*255)
        assert difference < .01, ('Restore changed window contents', scale, difference)
        entry['restore_mean_difference'] = difference
        watcher.clear()
        x(env, 'windowsize', wid, round(700*scale), round(600*scale))
        resize = watcher.read(.7)
        assert resize and any(r['width']*r['height'] >= round(700*scale)*round(600*scale)*.9 for r in resize), ('Resize did not redraw the new buffer', scale, resize)
        g = geometry(env, wid)
        shot().save(case / 'resized.png')
        results.append(entry)
        (out / 'results.json').write_text(json.dumps(dict(binary_sha256=hashlib.sha256(Path(binary).read_bytes()).hexdigest(), cases=results), indent=2))
        if not baseline:
            assert all(entry['cases'][name]['max_window_fraction'] < .25 for name in ['caret', 'typing', 'hover']), entry
    finally:
        if watcher:
            watcher.close()
        if p.poll() is None:
            os.killpg(p.pid, signal.SIGTERM)
        try:
            p.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.wait()
