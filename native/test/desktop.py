#!/usr/bin/env python3
"""Drive the installed WebView app on an isolated X11 display and D-Bus."""
import ctypes as c
import datetime
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi
from PIL import ImageGrab

binary = str(Path(sys.argv[1]).resolve())
output = Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
root = Path(tempfile.mkdtemp(prefix='case-', dir=output))
scale = int(os.environ.get('GDK_SCALE', '1'))
notes = root / 'Notes.txt'
board = root / 'Pin.md'
initial = 'A first note.\n\nWrite thoughts here.\n'
notes.write_text(initial)
today = datetime.date.today()
long_task = 'A long priority that must wrap across several lines and remain fully visible at the smallest supported window width.'
board.write_text(f'- [ ] ==🟢{long_task}==\n  - [ ] Child task\n- [ ] Second task\n- [x] Old finished\n- [x] Yesterday 📅 {today - datetime.timedelta(days=1)} 09:30 🔁 daily ✅ {today - datetime.timedelta(days=1)}\n')
env = dict(os.environ, XDG_CONFIG_HOME=str(root / 'config'), PRIORITIES_FILE=str(board), STICKY_NOTES_FILE=str(notes), GDK_BACKEND='x11', WEBKIT_DISABLE_COMPOSITING_MODE='1', GTK_IM_MODULE='simple', GSETTINGS_BACKEND='keyfile')
children = []
checks = []
proc = None

xlib = c.CDLL('libX11.so.6')
xtest = c.CDLL('libXtst.so.6')
xlib.XOpenDisplay.argtypes = [c.c_char_p]
xlib.XOpenDisplay.restype = c.c_void_p
xlib.XkbLockGroup.argtypes = [c.c_void_p, c.c_uint, c.c_uint]
xlib.XSync.argtypes = [c.c_void_p, c.c_int]
xtest.XTestFakeKeyEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
display = xlib.XOpenDisplay(env['DISPLAY'].encode())
assert display


def wait(check, label, seconds=12):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            value = check()
            if value:
                return value
        except Exception:
            pass
        time.sleep(.08)
    raise AssertionError(label)


def x(*args):
    return subprocess.check_output(['xdotool', *map(str, args)], env=env, text=True).strip()


def walk(node):
    yield node
    for i in range(node.get_child_count()):
        yield from walk(node.get_child_at_index(i))


def nodes():
    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        if app.get_process_id() == proc.pid:
            yield from walk(app)


def find(name):
    return wait(lambda: next((n for n in nodes() if n.get_name() == name), None), 'Find ' + name)


def click(node):
    rect = node.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
    x('mousemove', rect.x + rect.width // 2, rect.y + rect.height // 2, 'click', '1')
    time.sleep(.15)


def text(node):
    return Atspi.Text.get_text(node, 0, -1)


def chord(code, shift=False):
    # Physical key codes: do not use keysym-based xdotool Ctrl+z.
    events = [(37, 1)] + ([(50, 1)] if shift else []) + [(code, 1), (code, 0)] + ([(50, 0)] if shift else []) + [(37, 0)]
    for key, pressed in events:
        xtest.XTestFakeKeyEvent(display, key, pressed, 0)
    xlib.XSync(display, 0)


def group(value):
    xlib.XkbLockGroup(display, 256, value)
    xlib.XSync(display, 0)


def saved(path, expected):
    wait(lambda: path.read_text() == expected, 'Saved ' + path.name)


def check(label):
    checks.append(label)
    print('PASS', label, flush=True)


def shot(name):
    bounds = geometry()
    im = ImageGrab.grab(xdisplay=env['DISPLAY'])
    im.crop((bounds['X'], bounds['Y'], bounds['X'] + bounds['WIDTH'], bounds['Y'] + bounds['HEIGHT'])).save(output / (name + '.png'))


def geometry():
    return {k: int(v) for k, v in (line.split('=', 1) for line in x('getwindowgeometry', '--shell', wid).splitlines())}


def start(note_mode):
    global proc, wid
    log = open(output / ('notes.log' if note_mode else 'pin.log'), 'a')
    proc = subprocess.Popen([binary] + (['--notes'] if note_mode else []), env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    log.close()
    children.append(proc)
    wid = wait(lambda: x('search', '--onlyvisible', '--pid', proc.pid).splitlines()[0], 'Window opens', 25)
    x('windowfocus', wid)
    find('Notes' if note_mode else 'Task text')
    time.sleep(.3)


def close():
    click(find('Close'))
    proc.wait(timeout=8)
    assert proc.returncode == 0


def toolbar(label):
    return find(label)


try:
    subprocess.run(['setxkbmap', '-layout', 'us,ir'], env=env, check=True)
    for note_mode in [False, True]:
        start(note_mode)
        path = notes if note_mode else board
        label = 'Notes' if note_mode else 'Task text'
        if not note_mode:
            wait(lambda: f'- [ ] Yesterday 📅 {today} 09:30 🔁 daily' in board.read_text(), 'Daily rollover')
            check('Daily completion reopens on the next local date and keeps its time')
        baseline = path.read_text()
        field = find(label)
        click(field)
        x('key', 'ctrl+End' if note_mode else 'End')
        x('type', '--clearmodifiers', '--delay', '20', ' extra words')
        expected = baseline + ' extra words' if note_mode else baseline.replace(long_task, long_task + ' extra words')
        saved(path, expected)
        for layout in [0, 1]:
            group(layout)
            chord(52)
            saved(path, baseline)
            chord(52, True)
            saved(path, expected)
            chord(52)
            saved(path, baseline)
            chord(29)
            saved(path, expected)
            stamp = path.stat().st_mtime_ns
            chord(39)
            wait(lambda: path.stat().st_mtime_ns > stamp, 'Ctrl+S writes the file')
        group(0)
        click(toolbar('Undo (Ctrl+Z)'))
        saved(path, baseline)
        click(toolbar('Redo (Ctrl+Shift+Z)'))
        saved(path, expected)
        time.sleep(1.6)
        stamp = path.stat().st_mtime_ns
        click(toolbar('Save (Ctrl+S)'))
        wait(lambda: path.stat().st_mtime_ns > stamp, 'Save button writes the file')
        click(toolbar('Undo (Ctrl+Z)'))
        saved(path, baseline)
        check(('Notes' if note_mode else 'Pin') + ': physical English/Persian Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y, Ctrl+S and toolbar Undo/Redo/Save')
        if not note_mode:
            click(find('Task text'))
            x('key', 'ctrl+Return')
            wait(lambda: board.read_text().index('Old finished') < board.read_text().index(long_task), 'Completion moves the branch')
            chord(52)
            saved(board, baseline)
            options = [n for n in nodes() if n.get_name() == 'Task options'][0]
            click(options)
            click(find('Repeat daily'))
            wait(lambda: long_task + '== 🔁 daily' in board.read_text(), 'Daily option saves')
            x('key', 'Escape')
            chord(52)
            saved(board, baseline)
            check('Pin completion restores order on undo; daily option saves and undoes')
            for width in [280, 350, 460]:
                x('windowsize', wid, width * scale, 720 * scale)
                time.sleep(.5)
                for field in [n for n in nodes() if n.get_name() == 'Task text']:
                    value = text(field)
                    if not value:
                        continue
                    rect = field.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
                    last = Atspi.Text.get_character_extents(field, len(value) - 1, Atspi.CoordType.SCREEN)
                    assert last.y + last.height <= rect.y + rect.height + 2, (width, value, last.y, rect.height)
                shot('pin-' + str(width))
            check('All task lines remain within their text field at 280, 350 and 460 logical pixels')
        else:
            click(find('Notes'))
            notes.write_text('Changed elsewhere\n')
            click(find('Use disk'))
            saved(notes, 'Changed elsewhere\n')
            click(find('Notes'))
            x('key', 'ctrl+End')
            x('type', '--clearmodifiers', 'Saved after reload')
            saved(notes, 'Changed elsewhere\nSaved after reload')
            notes.write_text('Another disk edit\n')
            click(find('Keep mine'))
            saved(notes, 'Changed elsewhere\nSaved after reload')
            check('External file changes keep local text and both conflict choices work')
            shot('notes-light')
            subprocess.run(['gsettings', 'set', 'org.gnome.desktop.interface', 'color-scheme', 'prefer-dark'], env=env, check=True)
            time.sleep(.7)
            shot('notes-dark')
            subprocess.run(['gsettings', 'set', 'org.gnome.desktop.interface', 'color-scheme', 'default'], env=env, check=True)
        above = lambda: '_NET_WM_STATE_ABOVE' in subprocess.check_output(['xprop', '-id', wid, '_NET_WM_STATE'], env=env, text=True)
        assert above()
        click(find('Unpin from top'))
        wait(lambda: not above(), 'Unpin')
        click(find('Keep on top'))
        wait(above, 'Pin')
        x('windowsize', wid, 400 * scale, 400 * scale)
        x('windowmove', wid, 40, 60)
        time.sleep(.4)
        before = geometry()
        close()
        start(note_mode)
        after = geometry()
        assert all(before[k] == after[k] for k in ['WIDTH', 'HEIGHT', 'X', 'Y']), (before, after)
        check(('Notes' if note_mode else 'Pin') + ': real pin/unpin and restored window size/position')
        close()
    (output / 'result.json').write_text(json.dumps({'checks': checks, 'scale': scale, 'binary_bytes': Path(binary).stat().st_size}, indent=2) + '\n')
except BaseException:
    try:
        shot('failure')
    except Exception:
        pass
    raise
finally:
    group(0)
    for child in reversed(children):
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=4)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
