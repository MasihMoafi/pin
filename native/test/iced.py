#!/usr/bin/env python3
"""Compare real WebView and Iced Notes windows on one isolated X11 display."""
import ctypes as c
import json
import os
from pathlib import Path
import signal
import statistics
import subprocess
import sys
import tempfile
import time

from PIL import ImageGrab

baseline = str(Path(sys.argv[1]).resolve())
variant = str(Path(sys.argv[2]).resolve())
output = Path(sys.argv[3]).resolve()
output.mkdir(parents=True, exist_ok=True)
root = Path(tempfile.mkdtemp(prefix='iced-', dir=output))
initial = 'Responsibilities\n\nEach agent is responsible for something.\n'
children = []
checks = []
base_env = dict(os.environ, GDK_BACKEND='x11', GDK_SCALE='1', WINIT_X11_SCALE_FACTOR='1', WEBKIT_DISABLE_COMPOSITING_MODE='1', LIBGL_ALWAYS_SOFTWARE='1', GTK_IM_MODULE='simple', GSETTINGS_BACKEND='keyfile')

xlib = c.CDLL('libX11.so.6')
xtest = c.CDLL('libXtst.so.6')
xlib.XOpenDisplay.argtypes = [c.c_char_p]
xlib.XOpenDisplay.restype = c.c_void_p
xlib.XkbLockGroup.argtypes = [c.c_void_p, c.c_uint, c.c_uint]
xlib.XSync.argtypes = [c.c_void_p, c.c_int]
xtest.XTestFakeKeyEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
display = xlib.XOpenDisplay(base_env['DISPLAY'].encode())
assert display


def x(*args):
    return subprocess.check_output(['xdotool', *map(str, args)], env=base_env, text=True, stderr=subprocess.DEVNULL).strip()


def wait(check, label, seconds=12):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            value = check()
            if value:
                return value
        except (OSError, subprocess.CalledProcessError, IndexError):
            pass
        time.sleep(.05)
    raise AssertionError(label)


def check(label):
    checks.append(label)
    print('PASS', label, flush=True)


def start(kind, label):
    case = root / label
    config = case / 'config/StickyNotes'
    config.mkdir(parents=True)
    file = case / 'Notes.txt'
    file.write_text(initial)
    for prefs in ['native.json', 'iced-prototype.json']:
        (config / prefs).write_text(json.dumps({'width': 430, 'height': 320, 'x': 40, 'y': 60, 'pinned': True}))
    env = dict(base_env, XDG_CONFIG_HOME=str(case / 'config'), STICKY_NOTES_FILE=str(file))
    log = open(output / (label + '.log'), 'w')
    launch = time.monotonic()
    p = subprocess.Popen([baseline, '--notes'] if kind == 'webview' else [variant], env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    log.close()
    children.append(p)
    wid = wait(lambda: x('search', '--onlyvisible', '--pid', p.pid).splitlines()[0], kind + ' window opens', 25)
    elapsed = (time.monotonic() - launch) * 1000
    x('windowfocus', wid)
    time.sleep(1.5)
    return p, wid, file, elapsed


def close(p, wid):
    click(wid, 405, 21)
    p.wait(timeout=8)
    assert p.returncode == 0


def click(wid, px, py, button=1):
    x('mousemove', '--window', wid, px, py, 'click', button)
    time.sleep(.15)


def physical(code, shift=False):
    events = [(37, 1)] + ([(50, 1)] if shift else []) + [(code, 1), (code, 0)] + ([(50, 0)] if shift else []) + [(37, 0)]
    for key, on in events:
        xtest.XTestFakeKeyEvent(display, key, on, 0)
    xlib.XSync(display, 0)


def group(value):
    xlib.XkbLockGroup(display, 256, value)
    xlib.XSync(display, 0)


def saved(file, expected):
    wait(lambda: file.read_text() == expected, 'Saved content mismatch: ' + file.name)


def shot(name):
    ImageGrab.grab(xdisplay=base_env['DISPLAY']).save(output / (name + '.png'))


def tree(pid):
    yield pid
    try:
        for child in Path(f'/proc/{pid}/task/{pid}/children').read_text().split():
            yield from tree(int(child))
    except FileNotFoundError:
        pass


def usage(pid):
    pss = cpu = 0
    pids = list(tree(pid))
    for child in pids:
        try:
            for line in Path(f'/proc/{child}/smaps_rollup').read_text().splitlines():
                if line.startswith('Pss:'):
                    pss += int(line.split()[1])
            stat = Path(f'/proc/{child}/stat').read_text().split(') ', 1)[1].split()
            cpu += int(stat[11]) + int(stat[12])
        except FileNotFoundError:
            pass
    return {'pss_kib': pss, 'cpu_ticks': cpu, 'processes': len(pids)}


def clipboard(text):
    p = subprocess.Popen(['xclip', '-quiet', '-selection', 'clipboard', '-in'], env=base_env, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    children.append(p)
    p.stdin.write(text.encode())
    p.stdin.close()
    time.sleep(.2)


try:
    subprocess.run(['setxkbmap', '-layout', 'us,ir'], env=base_env, check=True)
    p, wid, file, _ = start('iced', 'editing')
    click(wid, 130, 130)
    x('key', 'ctrl+End')
    x('type', '--clearmodifiers', '--delay', 15, 'A new thought.')
    typed = initial + 'A new thought.'
    saved(file, typed)
    for layout in [0, 1]:
        group(layout)
        physical(52)
        saved(file, initial)
        physical(52, True)
        saved(file, typed)
        physical(52)
        saved(file, initial)
        physical(29)
        saved(file, typed)
        stamp = file.stat().st_mtime_ns
        physical(39)
        wait(lambda: file.stat().st_mtime_ns > stamp, 'Physical Ctrl+S saves')
    group(0)
    check('English and Persian physical Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y and Ctrl+S')
    click(wid, 262, 21)
    saved(file, initial)
    click(wid, 290, 21)
    saved(file, typed)
    stamp = file.stat().st_mtime_ns
    click(wid, 318, 21)
    wait(lambda: file.stat().st_mtime_ns > stamp, 'Toolbar Save')
    check('Toolbar Undo, Redo and Save share keyboard actions')
    click(wid, 130, 130)
    x('key', 'ctrl+a', 'BackSpace')
    saved(file, '')
    physical(52)
    saved(file, typed)
    check('Selecting and deleting all text is undoable')
    x('key', 'ctrl+End')
    clipboard('\nسلام دنیا\nA second line.')
    physical(55)  # physical Ctrl+V
    saved(file, typed + '\nسلام دنیا\nA second line.')
    physical(52)
    saved(file, typed)
    check('Plain-text multiline Persian clipboard paste and undo')
    click(wid, 130, 130)
    x('key', 'ctrl+End')
    group(1)
    for code in [39, 42, 43, 46]:
        xtest.XTestFakeKeyEvent(display, code, 1, 0)
        xtest.XTestFakeKeyEvent(display, code, 0, 0)
        xlib.XSync(display, 0)
        time.sleep(.03)
    group(0)
    time.sleep(.8)
    assert file.read_text() == typed + 'سلام', 'Persian layout did not insert the expected text'
    physical(52)
    saved(file, typed)
    check('Persian keyboard typing and grouped undo')
    x('key', 'ctrl+End', 'ctrl+BackSpace')
    saved(file, typed.rsplit(' ', 1)[0] + ' ')
    physical(52)
    saved(file, typed)
    check('Ctrl+Backspace removes a word and undo restores it')
    x('key', 'Tab', 'Return')
    saved(file, initial)
    x('key', 'Tab', 'Return')
    saved(file, typed)
    check('Tab and Enter operate toolbar history actions')
    shot('iced-notes-light')
    # The footprint comparison uses fresh windows with unchanged fixture files.
    close(p, wid)
    results = []
    for repeat in range(4):
        for kind in ['webview', 'iced']:
            p, wid, file, mapped = start(kind, f'{kind}-{repeat}')
            before = usage(p.pid)
            began = time.monotonic()
            time.sleep(4)
            after = usage(p.pid)
            elapsed = time.monotonic() - began
            record = {'kind': kind, 'repeat': repeat, 'warmup': repeat == 0, 'mapped_ms': round(mapped, 2), 'pss_kib': after['pss_kib'], 'processes': after['processes'], 'idle_cpu_percent': round((after['cpu_ticks'] - before['cpu_ticks']) / os.sysconf('SC_CLK_TCK') / elapsed * 100, 2), 'load': list(os.getloadavg())}
            results.append(record)
            print(json.dumps(record), flush=True)
            if repeat == 1:
                shot(kind + '-comparison')
            close(p, wid)
    median = {kind: {metric: statistics.median(r[metric] for r in results if r['kind'] == kind and not r['warmup']) for metric in ['mapped_ms', 'pss_kib', 'idle_cpu_percent']} for kind in ['webview', 'iced']}
    gate = median['iced']['pss_kib'] <= 50 * 1024 and median['iced']['pss_kib'] <= median['webview']['pss_kib'] / 2 and median['iced']['mapped_ms'] <= median['webview']['mapped_ms'] + 250
    summary = {'checks': checks, 'runs': results, 'median': median, 'footprint_gate': gate, 'kernel': os.uname().release, 'binary_bytes': {'webview': Path(baseline).stat().st_size, 'iced': Path(variant).stat().st_size}}
    (output / 'result.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary), flush=True)
    assert gate, 'Iced footprint/startup gate failed; do not replace installed apps'
except BaseException:
    shot('failure')
    raise
finally:
    group(0)
    for p in reversed(children):
        if p.poll() is None:
            os.killpg(p.pid, signal.SIGTERM)
            try:
                p.wait(timeout=4)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL)
                p.wait()
