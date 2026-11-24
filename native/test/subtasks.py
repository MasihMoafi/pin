"""Exercise discoverable subtask creation and history in real isolated Pin windows."""
import ctypes as c
import json
import os
from pathlib import Path
import runpy
import signal
import subprocess
import sys
import time

from PIL import ImageGrab

binary = str(Path(sys.argv[1]).resolve())
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
scale = int(os.environ.get('ICED_TEST_SCALE', '1'))
env = dict(os.environ, WINIT_X11_SCALE_FACTOR=str(scale), GSETTINGS_BACKEND='keyfile')
boxes = runpy.run_path(str(Path(__file__).with_name('row_checks.py')))['checkboxes']
initial = '- [ ] Other parent\n- [ ] ==🟢Target parent==\n  - [ ] ==🔵Older child==\n- [ ] Last parent\n'
checks = []

def x(*args):
    return subprocess.check_output(['xdotool', *map(str, args)], env=env, text=True, stderr=subprocess.DEVNULL).strip()

def wait(predicate, label):
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.03)
    raise AssertionError(label)

def terminate(_signum, _frame):
    raise SystemExit('Interrupted')

signal.signal(signal.SIGTERM, terminate)
lib = c.CDLL('libX11.so.6')
xtest = c.CDLL('libXtst.so.6')
lib.XOpenDisplay.argtypes = [c.c_char_p]
lib.XOpenDisplay.restype = c.c_void_p
lib.XkbLockGroup.argtypes = [c.c_void_p, c.c_uint, c.c_uint]
lib.XSync.argtypes = [c.c_void_p, c.c_int]
xtest.XTestFakeKeyEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
display = lib.XOpenDisplay(env['DISPLAY'].encode())
assert display
subprocess.run(['setxkbmap', '-layout', 'us,ir'], env=env, check=True)

def physical_key(code, shift=False):
    events = [(37, 1)] + ([(50, 1)] if shift else []) + [(code, 1), (code, 0)] + ([(50, 0)] if shift else []) + [(37, 0)]
    for key, on in events:
        xtest.XTestFakeKeyEvent(display, key, on, 0)
    lib.XSync(display, 0)

def physical_n(shift=False):
    physical_key(57, shift)

for label in ['mouse', 'keyboard', 'nested', 'drag', 'delete', 'history']:
    case = out / label
    config = case / 'config/Pin'
    config.mkdir(parents=True, exist_ok=True)
    file = case / 'Pin.md'
    width = 280 if label == 'history' else 460
    fixture = initial + '- [x] Finished project\n' if label == 'history' else initial
    file.write_text(fixture)
    (config / 'iced.json').write_text(json.dumps(dict(width=width, height=560, pinned=False)))
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
                assert time.monotonic() < deadline, 'Pin opens'
                time.sleep(.05)
        x('windowfocus', wid)
        time.sleep(.6)
        def shot(name):
            g = dict(line.split('=', 1) for line in x('getwindowgeometry', '--shell', wid).splitlines())
            gx, gy, gw, gh = [int(g[k]) for k in ['X', 'Y', 'WIDTH', 'HEIGHT']]
            image = ImageGrab.grab(xdisplay=env['DISPLAY']).crop((gx, gy, gx+gw, gy+gh))
            image.save(case / (name + '.png'))
            return image
        def click_row(index, px):
            picture = shot('before')
            rows = boxes(picture, scale)
            assert index < len(rows), ('Missing task row', rows)
            _, top, _, bottom = rows[index]
            x('mousemove', '--window', wid, px * scale, (top+bottom)//2, 'click', 1)
            time.sleep(.15)
        def saved(expected):
            wait(lambda: file.read_text() == expected, 'Saved list mismatch: ' + file.read_text())
        if label == 'history':
            assert len(boxes(shot('active'), scale)) == 4
            x('mousemove', '--window', wid, 56*scale, 21*scale, 'click', 1)
            time.sleep(.2)
            picture = shot('history').convert('RGB')
            assert any(picture.getpixel((px*scale, py*scale)) == (45, 151, 148)
                       for px in range(47, 62) for py in range(68, 82)), 'History shows a checked row'
            x('click', 1)
            time.sleep(.2)
            assert len(boxes(shot('active-again'), scale)) == 4, 'The same button returns to Active'
            x('key', 'ctrl+Tab', 'Return')
            assert not boxes(shot('history-keyboard'), scale), 'Keyboard activation opens History'
            x('key', 'ctrl+h')
            assert len(boxes(shot('active-keyboard'), scale)) == 4, 'The history shortcut returns to Active'
            saved(fixture)
            x('mousemove', '--window', wid, (width-25)*scale, 21*scale, 'click', 1)
            p.wait(timeout=5)
            assert p.returncode == 0, 'Close remains clickable at minimum width'
            checks.append('The compact History icon switches both ways and leaves Close available')
            print('PASS', checks[-1], flush=True)
            continue
        elif label == 'mouse':
            # Focus one parent, then click another parent's plus.
            click_row(0, 120)
            click_row(1, 400)
            x('type', '--clearmodifiers', 'From plus')
            expected = initial.replace('  - [ ] ==🔵Older child==', '  - [ ] ==🟢From plus==\n  - [ ] ==🔵Older child==')
            saved(expected)
            shot('child-focused')
            x('key', 'ctrl+z', 'ctrl+z')
            saved(initial)
            x('key', 'ctrl+y', 'ctrl+y')
            saved(expected)
            checks.append('Mouse plus targets its own parent, focuses the child, inherits color, inserts first, and supports exact undo/redo')
        elif label == 'keyboard':
            for layout in [0, 1]:
                click_row(1, 120)
                lib.XkbLockGroup(display, 256, layout)
                lib.XSync(display, 0)
                physical_n(True)
                blank = initial.replace('  - [ ] ==🔵Older child==', '  - [ ] ==🟢==\n  - [ ] ==🔵Older child==')
                saved(blank)
                # Undo on the same physical key under either keyboard layout.
                for key, on in [(37,1), (52,1), (52,0), (37,0)]:
                    xtest.XTestFakeKeyEvent(display, key, on, 0)
                lib.XSync(display, 0)
                saved(initial)
                physical_n()
                saved('- [ ] \n' + initial)
                for key, on in [(37,1), (52,1), (52,0), (37,0)]:
                    xtest.XTestFakeKeyEvent(display, key, on, 0)
                lib.XSync(display, 0)
                saved(initial)
            lib.XkbLockGroup(display, 256, 0)
            lib.XSync(display, 0)
            click_row(1, 120)
            x('key', 'ctrl+t')
            saved(blank)
            shot('keyboard-child')
            checks.append('Physical Ctrl+N and Ctrl+Shift+N work in English/Persian; legacy Ctrl+T still adds a subtask')
        elif label == 'delete':
            branch_removed = '- [ ] Other parent\n- [ ] Last parent\n'
            leaf_removed = initial.replace('  - [ ] ==🔵Older child==\n', '')
            last = '- [ ] Last parent\n'
            for layout in [0, 1]:
                lib.XkbLockGroup(display, 256, layout)
                lib.XSync(display, 0)
                # Delete another row's whole branch, then click elsewhere.
                click_row(0, 120)
                click_row(1, 369)
                saved(branch_removed)
                click_row(0, 120)
                physical_key(52)
                saved(initial)
                physical_key(29)
                saved(branch_removed)
                physical_key(52)
                saved(initial)
                physical_key(52, True)
                saved(branch_removed)
                physical_key(52)
                saved(initial)
                # A leaf removes only itself; both redo keys preserve siblings.
                click_row(2, 369)
                saved(leaf_removed)
                physical_key(52)
                saved(initial)
                physical_key(29)
                saved(leaf_removed)
                physical_key(52)
                saved(initial)
                physical_key(52, True)
                saved(leaf_removed)
                physical_key(52)
                saved(initial)
                # The final row can be removed and restored from the empty state.
                click_row(1, 369)
                saved(branch_removed)
                click_row(0, 369)
                saved(last)
                click_row(0, 369)
                wait(lambda: not file.read_text().strip(), 'Final task deletion')
                empty = file.read_text()
                physical_key(52)
                saved(last)
                physical_key(29)
                saved(empty)
                physical_key(52)
                saved(last)
                physical_key(52, True)
                saved(empty)
                physical_key(52)
                saved(last)
                physical_key(52)
                saved(branch_removed)
                physical_key(52)
                saved(initial)
            lib.XkbLockGroup(display, 256, 0)
            lib.XSync(display, 0)
            shot('row-delete')
            checks.append('Row trash deletes its own leaf, branch or final task; Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z preserve exact contents in English/Persian, including after clicking elsewhere')
        elif label == 'drag':
            picture = shot('initial')
            rows = boxes(picture, scale)
            _, top, _, bottom = rows[-1]
            x('mousemove', '--window', wid, 23*scale, (rows[0][1]+rows[0][3])//2, 'mousedown', 1)
            time.sleep(.15)
            x('mousemove', '--window', wid, 150*scale, (top+bottom)//2)
            time.sleep(.15)
            shot('before-final-task')
            x('mouseup', 1)
            moved = '- [ ] ==🟢Target parent==\n  - [ ] ==🔵Older child==\n- [ ] Other parent\n- [ ] Last parent\n'
            saved(moved)
            x('key', 'ctrl+z')
            saved(initial)
            x('mousemove', '--window', wid, 23*scale, (rows[0][1]+rows[0][3])//2, 'mousedown', 1)
            time.sleep(.15)
            x('mousemove', '--window', wid, 150*scale, bottom+30*scale)
            time.sleep(.15)
            shot('after-final-task')
            x('mouseup', 1)
            saved('- [ ] ==🟢Target parent==\n  - [ ] ==🔵Older child==\n- [ ] Last parent\n- [ ] Other parent\n')
            x('key', 'ctrl+z')
            saved(initial)
            checks.append('Drops before the final task and after the list stay distinct, preserve branches, and undo exactly')
        else:
            rows = boxes(shot('expanded'), scale)
            assert len(rows) == 4 and rows[0][0] == rows[1][0], rows
            assert rows[0][0] < 50 * scale, ('Empty gutter before the grip', rows[0])
            click_row(1, rows[1][2] // scale + 18)
            assert len(boxes(shot('folded'), scale)) == 3, 'Clicking beside the parent title folds its child'
            saved(initial)
            click_row(1, 400)
            x('type', '--clearmodifiers', 'Unfolded child')
            expected = initial.replace('  - [ ] ==🔵Older child==', '  - [ ] ==🟢Unfolded child==\n  - [ ] ==🔵Older child==')
            saved(expected)
            # The newly focused subtask can itself own a child.
            x('key', 'ctrl+shift+n')
            x('type', '--clearmodifiers', 'Grandchild')
            nested = expected.replace('  - [ ] ==🟢Unfolded child==', '  - [ ] ==🟢Unfolded child==\n    - [ ] ==🟢Grandchild==')
            saved(nested)
            x('windowsize', wid, 280*scale, 560*scale)
            time.sleep(.3)
            picture = shot('nested-narrow')
            rows = boxes(picture, scale)
            assert len(rows) == 6, rows
            assert rows[4][1] - rows[3][1] >= 65 * scale, 'A nested word must wrap rather than clip at minimum width'
            x('key', 'ctrl+shift+BackSpace', 'ctrl+z')
            saved(nested)
            x('key', 'ctrl+s')
            checks.append('A folded parent opens for its new child; nesting and narrow layout preserve text, and deleting the grandchild is undoable')
        print('PASS', checks[-1], flush=True)
        x('mousemove', '--window', wid, 250*scale, 20*scale)
        x('key', 'ctrl+s')
        time.sleep(.3)
        # Verify the saved hierarchy survives a fresh process.
        durable = file.read_text()
        os.killpg(p.pid, signal.SIGTERM)
        p.wait(timeout=5)
        with (case / 'log').open('a') as log:
            p = subprocess.Popen([binary], env=run_env, stdout=log, stderr=log, start_new_session=True)
        time.sleep(.8)
        assert p.poll() is None
        assert file.read_text() == durable
    finally:
        if p.poll() is None:
            os.killpg(p.pid, signal.SIGTERM)
        p.wait(timeout=5)
(out / 'results.json').write_text(json.dumps(dict(scale=scale, checks=checks), indent=2))
