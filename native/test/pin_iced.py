#!/usr/bin/env python3
"""Drive native Pin on an isolated X11 display using fixtures, never the real list."""
import ctypes as c
from datetime import date, timedelta
import json
import os
from pathlib import Path
import signal
import runpy
from collections import Counter
import statistics
import subprocess
import sys
import tempfile
import time
from PIL import ImageGrab

baseline, variant = (str(Path(p).resolve()) for p in sys.argv[1:3])
out = Path(sys.argv[3]).resolve()
out.mkdir(parents=True, exist_ok=True)
root = Path(tempfile.mkdtemp(prefix='pin-', dir=out))
scale = int(os.environ.get('ICED_TEST_SCALE', '1'))
env = dict(os.environ, GDK_BACKEND='x11', GDK_SCALE=str(scale), WINIT_X11_SCALE_FACTOR=str(scale), WEBKIT_DISABLE_COMPOSITING_MODE='1', LIBGL_ALWAYS_SOFTWARE='1', GSETTINGS_BACKEND='keyfile')
children, checks = [], []
resize_edges=runpy.run_path(str(Path(__file__).with_name("window_checks.py")))["resize"]
row_checks=runpy.run_path(str(Path(__file__).with_name("row_checks.py")))
today = date.today().isoformat()
initial = 'Intro stays here.\n\n- [ ] ==🟢First priority==\n  - [ ] ==🔵Child task==\n- [ ] ==🟣Second priority==\n- [x] ==🟠Previously done==\n'
long_text = 'Read one long research paper and take careful notes about the training process, model behavior and the evaluation harness.'
wrapped = '- [ ] ==🟢' + long_text + '==\n- [ ] ==🔵Another priority==\n  - [ ] ==🟣Nested priority with a longer explanation that must wrap cleanly too==\n- [ ] ==🟡سلام دنیا و یادداشت های روزانه==\n'

xlib = c.CDLL('libX11.so.6')
xtest = c.CDLL('libXtst.so.6')
xlib.XOpenDisplay.argtypes = [c.c_char_p]
xlib.XOpenDisplay.restype = c.c_void_p
xlib.XkbLockGroup.argtypes = [c.c_void_p, c.c_uint, c.c_uint]
xlib.XSync.argtypes = [c.c_void_p, c.c_int]
xtest.XTestFakeKeyEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
display = xlib.XOpenDisplay(env['DISPLAY'].encode())
assert display

def x(*args):
    return subprocess.check_output(['xdotool', *map(str, args)], env=env, text=True, stderr=subprocess.DEVNULL).strip()

def wait(predicate, label, seconds=12):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        try:
            value = predicate()
            if value:
                return value
        except (OSError, subprocess.CalledProcessError, IndexError):
            pass
        time.sleep(.05)
    raise AssertionError(label)

def check(label):
    checks.append(label)
    print('PASS', label, flush=True)

def geometry(wid):
    return {k: int(v) for k, v in (s.split('=', 1) for s in x('getwindowgeometry', '--shell', wid).splitlines())}

def start(label, contents=initial, kind='iced', reuse=False, theme=None):
    global current_kind
    current_kind = kind
    case = root / label
    config = case / 'config/Pin'
    config.mkdir(parents=True, exist_ok=True)
    file = case / 'Pin.md'
    if not reuse:
        file.write_text(contents)
        # Native preferences use physical positions; Iced migrates them using the actual scale.
        (config / 'native.json').write_text(json.dumps({'width': 460, 'height': 440, 'x': 40 * scale, 'y': 30 * scale, 'pinned': True, 'folded': []}))
    run_env = dict(env, XDG_CONFIG_HOME=str(case / 'config'), PRIORITIES_FILE=str(file))
    if theme:
        run_env['ICED_THEME'] = theme
    log = open(out / (label + '.log'), 'a')
    began = time.monotonic()
    p = subprocess.Popen([variant if kind == 'iced' else baseline], env=run_env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    log.close()
    children.append(p)
    def mapped():
        if p.poll() is not None:
            raise RuntimeError((out / (label + '.log')).read_text())
        return x('search', '--onlyvisible', '--pid', p.pid).splitlines()[0]
    wid = wait(mapped, 'Pin opens', 25)
    mapped_ms = (time.monotonic() - began) * 1000
    x('windowfocus', wid)
    time.sleep(1.2)
    return p, wid, file, mapped_ms

def click(px, py, button=1):
    x('mousemove', '--window', wid, px * scale, py * scale, 'click', button)
    time.sleep(.2)

def physical(code, shift=False):
    events = [(37, 1)] + ([(50, 1)] if shift else []) + [(code, 1), (code, 0)] + ([(50, 0)] if shift else []) + [(37, 0)]
    for key, on in events:
        xtest.XTestFakeKeyEvent(display, key, on, 0)
    xlib.XSync(display, 0)

def group(n):
    xlib.XkbLockGroup(display, 256, n)
    xlib.XSync(display, 0)

def saved(expected):
    wait(lambda: file.read_text() == expected, 'Unexpected saved list: ' + file.read_text())

def contains(value):
    wait(lambda: value in file.read_text(), 'Missing ' + value)

def shot(name):
    image = ImageGrab.grab(xdisplay=env['DISPLAY'])
    try:
        g = geometry(wid)
        image = image.crop((g['X'], g['Y'], g['X'] + g['WIDTH'], g['Y'] + g['HEIGHT']))
    except (NameError, OSError, subprocess.CalledProcessError):
        pass
    image.save(out / (name + '.png'))
    return image

def close():
    if current_kind == 'webview':
        os.killpg(p.pid, signal.SIGTERM)
        p.wait(timeout=8)
    else:
        click(geometry(wid)['WIDTH'] // scale - 25, 21)
        p.wait(timeout=8)
        assert p.returncode == 0

def clipboard(value):
    q = subprocess.Popen(['xclip', '-quiet', '-selection', 'clipboard', '-in'], env=env, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    children.append(q)
    q.stdin.write(value.encode())
    q.stdin.close()
    time.sleep(.15)

def tree(pid):
    yield pid
    try:
        for child in Path(f'/proc/{pid}/task/{pid}/children').read_text().split():
            yield from tree(int(child))
    except FileNotFoundError:
        pass

def usage(pid):
    pids = list(tree(pid))
    total = 0
    for pid in pids:
        try:
            total += int(next(line.split()[1] for line in Path(f'/proc/{pid}/smaps_rollup').read_text().splitlines() if line.startswith('Pss:')))
        except FileNotFoundError:
            pass
    return {'pss_kib': total, 'processes': len(pids)}

try:
    subprocess.run(['setxkbmap', '-layout', 'us,ir'], env=env, check=True)
    p, wid, file, _ = start('editing')
    click(115, 73)
    x('key', 'End')
    x('type', '--clearmodifiers', '--delay', 12, ' updated')
    typed = initial.replace('First priority', 'First priority updated')
    saved(typed)
    for layout in [0, 1]:
        group(layout)
        physical(52)
        saved(initial)
        physical(52, shift=True)
        saved(typed)
        physical(52)
        saved(initial)
        physical(29)
        saved(typed)
        before = file.stat().st_mtime_ns
        physical(39)
        wait(lambda: file.stat().st_mtime_ns > before, 'Physical Ctrl+S writes')
    group(0)
    check('English/Persian physical Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y and Ctrl+S')
    click(293, 21)
    saved(initial)
    click(321, 21)
    saved(typed)
    before = file.stat().st_mtime_ns
    click(349, 21)
    wait(lambda: file.stat().st_mtime_ns > before, 'Save button writes')
    check('Toolbar Undo, Redo and Save use the same actions')
    click(115, 73)
    x('key', 'End', 'ctrl+Return')
    completed = 'Intro stays here.\n\n- [ ] ==🟣Second priority==\n- [x] ==🟠Previously done==\n- [x] ==🟢First priority updated==\n  - [ ] ==🔵Child task==\n'
    saved(completed)
    physical(52)
    saved(typed)
    check('Completion moves the whole branch last; undo restores its place and status')
    x('key', 'ctrl+t')
    contains('  - [ ] ==🟢==\n  - [ ] ==🔵Child task==')
    x('type', '--clearmodifiers', 'New first child')
    contains('  - [ ] ==🟢New first child==\n  - [ ] ==🔵Child task==')
    x('key', 'shift+Tab')
    contains('\n- [ ] ==🟢New first child==')
    x('key', 'ctrl+shift+BackSpace')
    saved(typed)
    physical(52)
    contains('New first child')
    physical(29)
    saved(typed)
    check('New subtasks go first, outdent and deletion are undoable')
    click(115, 73)
    x('key', 'End', 'ctrl+d')
    time.sleep(.3)
    shot('pin-options')
    # The task list stays visible while a dropdown opens a month calendar.
    x('key', 'Return')
    time.sleep(.2)
    target = date(2028, 2, 29)
    months = (target.year - date.today().year) * 12 + target.month - date.today().month
    if months:
        x('key', *(['Next' if months > 0 else 'Prior'] * abs(months)))
    days = target.day - date.today().day
    if days:
        x('key', *(['Right' if days > 0 else 'Left'] * abs(days)))
    shot('pin-calendar')
    x('key', 'Return')
    contains('📅 2028-02-29')
    # Hour and minute are dropdowns too; Enter chooses the focused option.
    x('key', 'Tab', 'Return', 'Return', 'Return')
    contains('📅 2028-02-29 09:00')
    x('key', 'Tab', 'Return', *(['Tab'] * 30), 'Return')
    contains('📅 2028-02-29 09:30')
    x('key', 'Escape', 'Escape')
    physical(52)
    contains('📅 2028-02-29 09:00')
    physical(29)
    contains('📅 2028-02-29 09:30')
    x('key', 'Escape', 'Escape')
    check('Calendar and time dropdowns work by keyboard and share undo/redo')
    # Open a fresh options panel, then reach daily repeat with Shift+Tab.
    click(115, 73)
    x('key', 'ctrl+d', 'shift+Tab', 'Return')
    contains('🔁 daily')
    x('key', 'Escape')
    click(115, 73)
    x('key', 'ctrl+Return')
    contains('🔁 daily ✅ ' + today)
    physical(52)
    wait(lambda: '✅ ' not in file.read_text(), 'Undo daily completion')
    check('Daily recurrence is reachable by keyboard and completion is undoable')
    close()
    # Restart with a yesterday-completed daily task; ordinary tasks stay completed.
    yesterday = (date.today() - timedelta(days=1)).isoformat()
    recurrence = f'- [x] Daily 📅 {yesterday} 09:30 🔁 daily ✅ {yesterday}\n- [x] One-off\n'
    p, wid, file, _ = start('daily', recurrence)
    saved(f'- [ ] Daily 📅 {today} 09:30 🔁 daily\n- [x] One-off\n')
    close()
    check('Daily tasks reopen after an overnight restart and preserve their time')
    p, wid, file, _ = start('colors')
    click(115, 73)
    x('key', 'ctrl+d', 'Tab', 'Tab', 'Return', *(['Tab'] * 6), 'Return')
    contains('<mark style="background: #ec4899;">First priority</mark>')
    physical(52)
    saved(initial)
    physical(29)
    contains('#ec4899')
    for _ in range(7):
        x('key', 'Tab')
    x('key', 'ctrl+a')
    x('type', '--clearmodifiers', '#123456')
    shot('pin-custom-color')
    x('key', 'Return')
    contains('<mark style="background: #123456;">First priority</mark>')
    physical(52)
    contains('#ec4899')
    x('key', 'Escape', 'Escape')
    close()
    check('Palette and custom colors work through the keyboard and share undo/redo')
    p, wid, file, _ = start('unicode', '- [ ] First priority\n- [ ] Second priority\n')
    click(115, 73)
    x('key', 'End')
    clipboard(' سلام دنیا\nnext line')
    physical(55)
    saved('- [ ] First priority سلام دنیا next line\n- [ ] Second priority\n')
    physical(52)
    saved('- [ ] First priority\n- [ ] Second priority\n')
    x('key', 'ctrl+a', 'shift+F10')
    time.sleep(.2)
    # Redo is the first enabled entry; Copy follows it.
    x('key', 'Down', 'Return')
    copied = subprocess.check_output(['xclip', '-selection', 'clipboard', '-out'], env=env, text=True)
    assert copied == 'First priority', copied
    check('Plain-text Persian paste and the keyboard context menu preserve selection and undo')
    close()
    p, wid, file, _ = start('windows')
    above = lambda: '_NET_WM_STATE_ABOVE' in subprocess.check_output(['xprop', '-id', wid, '_NET_WM_STATE'], env=env, text=True)
    wait(above, 'Initial pin state')
    click(379, 21)
    wait(lambda: not above(), 'Unpin changes the window manager state')
    click(115, 73)
    x('key', 'ctrl+period')
    time.sleep(.2)
    x('windowsize', wid, 350 * scale, 300 * scale)
    x('windowmove', wid, 80, 90)
    time.sleep(.3)
    bounds = geometry(wid)
    close()
    p, wid, file, _ = start('windows', reuse=True)
    assert all(geometry(wid)[key] == bounds[key] for key in ['WIDTH', 'HEIGHT', 'X', 'Y'])
    wait(lambda: not above(), 'Unpinned state survives restart')
    prefs = json.loads((root / 'windows/config/Pin/iced.json').read_text())
    assert prefs['folded'] == ['First priority'], prefs
    saved(initial)
    shot('pin-remembered-window')
    resize_edges(env, wid, scale, out, 'pin-edge-light')
    close()
    check('Pin/unpin, scaled bounds, folded branches and actual continuous edge resizing work')
    p, wid, file, _ = start('conflicts', '- [ ] First priority\n')
    click(115, 73)
    x('key', 'End')
    x('type', '--clearmodifiers', '--delay', 1, ' local')
    file.write_text('- [ ] Agent update\n')
    time.sleep(.9)
    assert file.read_text() == '- [ ] Agent update\n'
    shot('pin-conflict')
    click(128, 92)
    saved('- [ ] First priority local\n')
    click(115, 73)
    x('key', 'End')
    x('type', '--clearmodifiers', '--delay', 1, ' unsaved')
    file.write_text('- [ ] Disk version\n')
    time.sleep(.9)
    click(45, 92)
    saved('- [ ] Disk version\n')
    file.write_text('- [ ] New external version\n')
    time.sleep(3)
    click(115, 73)
    x('key', 'End')
    x('type', '--clearmodifiers', ' appended')
    saved('- [ ] New external version appended\n')
    close()
    check('Both conflict choices protect local typing and idle reload uses the external list')
    drag_list = '- [ ] ==🟢Alpha==\n- [ ] ==🔵Beta==\n- [ ] ==🟣Gamma==\n'
    p, wid, file, _ = start('drag', drag_list)
    x('mousemove', '--window', wid, 402 * scale, 75 * scale, 'mousedown', '1')
    time.sleep(.15)
    x('mousemove', '--window', wid, 150 * scale, 210 * scale)
    time.sleep(.3)
    shot('pin-drop-indicator')
    x('mouseup', '1')
    saved('- [ ] ==🔵Beta==\n- [ ] ==🟣Gamma==\n- [ ] ==🟢Alpha==\n')
    physical(52)
    saved(drag_list)
    x('mousemove', '--window', wid, 402 * scale, 75 * scale, 'mousedown', '1')
    time.sleep(.15)
    x('mousemove', 1, 1)
    x('mouseup', '1')
    time.sleep(.4)
    saved(drag_list)
    x('mousemove', '--window', wid, 80 * scale, 75 * scale, 'mousedown', '1')
    time.sleep(.15)
    x('mousemove', '--window', wid, 150 * scale, 75 * scale, 'mouseup', '1')
    physical(53)  # Cut the dragged text selection, not a task branch.
    wait(lambda: file.read_text() != drag_list, 'Dragging text makes a selection')
    physical(52)
    saved(drag_list)
    close()
    check('Only the grip reorders; drop indication, outside cancellation and text selection work')
    p, wid, file, _ = start('wrapping', wrapped)
    heights = []
    for width in [460, 350, 280]:
        x('windowsize', wid, width * scale, 440 * scale)
        time.sleep(.5)
        im = shot('pin-wrap-' + str(width))
        ys = [y for y in range(45 * scale, im.height) if im.getpixel((12 * scale, y))[1] - im.getpixel((12 * scale, y))[0] > 8]
        assert ys, 'The first colored row is missing'
        top = ys[0]
        end = next((y for y in range(top, im.height) if y not in set(ys)), im.height)
        heights.append((end - top) / scale)
        assert heights[-1] > 65, ('Long text did not grow its row', width, heights)
    assert heights[0] < heights[1] < heights[2], heights
    saved(wrapped)
    check('Long task rows grow at 460, 350 and 280 logical pixels without changing text')
    close()
    p, wid, file, _ = start('dark', wrapped, theme='Dark')
    im = shot('pin-dark')
    pixel = im.getpixel((3 * scale, im.height - 12 * scale))
    assert max(pixel[:3]) < 65, pixel
    close()
    check('Native Pin renders its dark theme')
    # Solid checkbox faces and a 3:1 outline contrast are checked from the real pixels.
    def luminance(rgb):
        v=[c/255 for c in rgb[:3]]
        v=[c/12.92 if c<=.04045 else ((c+.055)/1.055)**2.4 for c in v]
        return sum(c*w for c,w in zip(v,[.2126,.7152,.0722]))
    def contrast(a,b):
        low,high=sorted([luminance(a),luminance(b)])
        return (high+.05)/(low+.05)
    palette='- [ ] ==🟢Green priority==\n- [ ] ==🟡Yellow priority==\n- [ ] <mark style="background: #ec4899;">Pink priority</mark>\n- [ ] ==🔵Blue priority==\n- [ ] Plain priority\n'
    contrasts=[]
    for theme in ['Light','Dark']:
        p,wid,file,_=start('contrast-'+theme,palette,theme=theme)
        picture=shot('pin-checkboxes-'+theme).convert('RGB')
        face=(25,25,28) if theme=='Dark' else (252,252,252)
        boxes=row_checks['checkboxes'](picture,scale,theme=='Dark')
        assert len(boxes)==5,boxes
        layout=row_checks['alignment'](picture,scale,boxes,theme=='Dark')
        (out/('row-layout-'+theme+'.json')).write_text(json.dumps(layout,indent=2)+'\n')
        for i,box in enumerate(boxes):
            left,top,right,bottom=box
            crop=picture.crop(box)
            colors=Counter(crop.getdata())
            row_color=picture.getpixel((right+2*scale,(top+bottom)//2))
            outline=max((rgb for rgb,count in colors.most_common(8)),key=lambda rgb:contrast(rgb,row_color))
            ratio=contrast(outline,row_color)
            record={'theme':theme,'row':i,'outline':outline,'background':row_color,'contrast':round(ratio,2)}
            contrasts.append(record)
            assert ratio>=3,record
            assert colors[face]>=crop.width*crop.height*.4,('Checkbox face is not solid',record,colors.most_common(3))
        if theme=='Dark':
            x('windowsize',wid,350*scale,300*scale)
            x('windowmove',wid,80,60)
            time.sleep(.3)
            resize_edges(env,wid,scale,out,'pin-edge-dark',dark=True)
        close()
    (out/'checkbox-contrast.json').write_text(json.dumps(contrasts,indent=2)+'\n')
    check('Checkboxes stay solid with at least 3:1 contrast on five colors in both themes; dark edge resizing retains paint')
    records = []
    for repeat in range(4):
        for kind in ['webview', 'iced']:
            p, wid, file, mapped = start(f'{kind}-{repeat}', wrapped, kind=kind)
            record = dict(usage(p.pid), kind=kind, repeat=repeat, warmup=repeat == 0, mapped_ms=round(mapped, 2), load=list(os.getloadavg()))
            records.append(record)
            print(json.dumps(record), flush=True)
            if repeat == 1:
                shot(kind + '-comparison')
            close()
    median = {kind: {key: statistics.median(r[key] for r in records if r['kind'] == kind and not r['warmup']) for key in ['pss_kib', 'mapped_ms']} for kind in ['webview', 'iced']}
    gate = median['iced']['pss_kib'] < 50 * 1024 and median['iced']['pss_kib'] < median['webview']['pss_kib'] / 2 and median['iced']['mapped_ms'] <= median['webview']['mapped_ms'] + 250
    (out / 'result.json').write_text(json.dumps({'checks': checks, 'runs': records, 'median': median, 'footprint_gate': gate}, indent=2) + '\n')
    assert gate, median
finally:
    if sys.exc_info()[0]:
        shot('failure')
    group(0)
    for p in reversed(children):
        if p.poll() is None:
            os.killpg(p.pid, signal.SIGTERM)
            p.wait(timeout=5)
