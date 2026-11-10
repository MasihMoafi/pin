#!/usr/bin/env python3
"""Capture continuous edge drags on an isolated GNOME Wayland/Xwayland desktop.

Run with DISPLAY, XAUTHORITY and DBUS_SESSION_BUS_ADDRESS for the hidden desktop.
Arguments: PIN_BINARY NOTES_BINARY OUTPUT. RESIZE_TEST_SCALE defaults to 1.
For native Pin, run with ICED_THEME=Light and ICED_THEME=Dark.
The compositor must not be the user's active desktop.
"""
import json
from collections import Counter
import math
import os
from pathlib import Path
import runpy
import signal
import subprocess
import sys
import tempfile
import threading
import time

import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi, Gio, GLib
from PIL import Image, ImageChops

pin, notes = (str(Path(p).resolve()) for p in sys.argv[1:3])
output = Path(sys.argv[3]).resolve()
output.mkdir(parents=True, exist_ok=True)
root = Path(tempfile.mkdtemp(prefix='resize-', dir=output))
scale = float(os.environ.get('RESIZE_TEST_SCALE', '1'))
assert os.environ['DISPLAY'] != ':0', 'Use an isolated desktop'
env = dict(os.environ, GDK_BACKEND='x11', WEBKIT_DISABLE_COMPOSITING_MODE='1', GSETTINGS_BACKEND='keyfile', GDK_SCALE=str(scale), WINIT_X11_SCALE_FACTOR=str(scale), XDG_CONFIG_HOME=str(root / 'config'), STICKY_NOTES_FILE=str(root / 'Notes.txt'), PRIORITIES_FILE=str(root / 'Pin.md'))
(root / 'Notes.txt').write_text('Responsibilities\n\nEach agent is responsible for something.\n')
(root / 'Pin.md').write_text('- [ ] ==🟢Green priority==\n- [ ] ==🟡Yellow priority==\n- [ ] <mark style="background: #ec4899;">Pink priority</mark>\n- [ ] ==🔵Blue priority==\n- [ ] Plain priority\n')
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
# The isolated shell grants screenshots to the media-keys service.
name = bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'RequestName', GLib.Variant('(su)', ('org.gnome.SettingsDaemon.MediaKeys', 0)), None, 0, 3000, None).unpack()[0]
assert name == 1, 'The hidden desktop must not have an active media-keys service'
shell_pid = bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', GLib.Variant('(s)', ('org.gnome.Shell',)), None, 0, 3000, None).unpack()[0]
shell_proc = Path(f'/proc/{shell_pid}')
assert b'--headless' in (shell_proc / 'cmdline').read_bytes(), 'Use the isolated headless shell'
shell_env = dict(part.split('=', 1) for part in (shell_proc / 'environ').read_bytes().decode().split('\0') if '=' in part)
assert shell_env.get('GSETTINGS_BACKEND') == 'keyfile', 'Use isolated desktop settings'
for key, value in [('picture-uri', ''), ('picture-uri-dark', ''), ('primary-color', '#808080'), ('color-shading-type', 'solid')]:
    subprocess.run(['gsettings', 'set', 'org.gnome.desktop.background', key, value], env=shell_env, check=True)
session = bus.call_sync('org.gnome.Mutter.RemoteDesktop', '/org/gnome/Mutter/RemoteDesktop', 'org.gnome.Mutter.RemoteDesktop', 'CreateSession', None, None, 0, 3000, None).unpack()[0]


def input_event(method, signature=None, args=None):
    return bus.call_sync('org.gnome.Mutter.RemoteDesktop', session, 'org.gnome.Mutter.RemoteDesktop.Session', method, GLib.Variant(signature, args) if signature else None, None, 0, 3000, None)


def x(*args):
    return subprocess.check_output(['xdotool', *(str(round(arg)) if isinstance(arg, float) else str(arg) for arg in args)], env=env, text=True, stderr=subprocess.DEVNULL).strip()


def geometry(wid):
    return {k: int(v) for k, v in (line.split('=') for line in x('getwindowgeometry', '--shell', wid).splitlines())}


position = [0., 0.]


def move(px, py):
    input_event('NotifyPointerMotionRelative', '(dd)', (px - position[0], py - position[1]))
    position[:] = [px, py]


def screenshot(name, window=False):
    path = output / (name + '.png')
    method = 'ScreenshotWindow' if window else 'Screenshot'
    args = GLib.Variant('(bbbs)', (False, False, False, str(path))) if window else GLib.Variant('(bbs)', (False, False, str(path)))
    success, _ = bus.call_sync('org.gnome.Shell.Screenshot', '/org/gnome/Shell/Screenshot', 'org.gnome.Shell.Screenshot', method, args, None, 0, 5000, None).unpack()
    assert success, 'Compositor screenshot failed'
    return path


def walk(node):
    yield node
    for i in range(node.get_child_count()):
        yield from walk(node.get_child_at_index(i))


def luminance(pixel):
    channels = [c / 255 for c in pixel]
    channels = [c / 12.92 if c <= .04045 else ((c + .055) / 1.055) ** 2.4 for c in channels]
    return sum(c * weight for c, weight in zip(channels, [.2126, .7152, .0722]))


def contrast(first, second):
    values = sorted([luminance(first), luminance(second)])
    return (values[1] + .05) / (values[0] + .05)


def checkboxes(pid):
    desktop = Atspi.get_desktop(0)
    app = next(n for n in walk(desktop) if n.get_process_id() == pid)
    checks = [n for n in walk(app) if n.get_name().startswith('Complete ')]
    assert len(checks) == 5, ('Task checkboxes', len(checks))
    evidence = []
    for theme in ['default', 'prefer-dark']:
        subprocess.run(['gsettings', 'set', 'org.gnome.desktop.interface', 'color-scheme', theme], env=env, check=True)
        time.sleep(.8)
        picture = Image.open(screenshot('checkboxes-' + theme)).convert('RGB')
        for node in checks:
            r = node.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
            left, top, width, height = [int(v * scale) for v in [r.x, r.y, r.width, r.height]]
            crop = picture.crop((left, top, left + width, top + height))
            colors = [rgb for rgb, count in Counter(crop.getdata()).most_common(2)]
            assert len(colors) == 2, (node.get_name(), colors)
            row = picture.getpixel((left + width + 3 * scale, top + height // 2))
            outline = max(colors, key=lambda c: contrast(c, row))
            ratio = contrast(outline, row)
            evidence.append({'theme': theme, 'checkbox': node.get_name(), 'outline': outline, 'row': row, 'contrast': round(ratio, 2)})
            assert ratio >= 3, evidence[-1]
    subprocess.run(['gsettings', 'set', 'org.gnome.desktop.interface', 'color-scheme', 'default'], env=env, check=True)
    (output / 'checkboxes.json').write_text(json.dumps(evidence, indent=2) + '\n')
    baseline = (root / 'Pin.md').read_text()
    r = checks[0].get_component_iface().get_extents(Atspi.CoordType.SCREEN)
    move((r.x + r.width / 2) * scale, (r.y + r.height / 2) * scale)
    input_event('NotifyPointerButton', '(ib)', (272, True))
    input_event('NotifyPointerButton', '(ib)', (272, False))
    time.sleep(.6)
    assert '- [x] ==🟢Green priority==' in (root / 'Pin.md').read_text(), 'Checkbox click must save completion'
    undo = next(n for n in walk(app) if n.get_name() == 'Undo (Ctrl+Z)')
    r = undo.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
    move((r.x + r.width / 2) * scale, (r.y + r.height / 2) * scale)
    input_event('NotifyPointerButton', '(ib)', (272, True))
    input_event('NotifyPointerButton', '(ib)', (272, False))
    time.sleep(.6)
    assert (root / 'Pin.md').read_text() == baseline, 'Undo restores checkbox state and position'
    print('PASS: checkbox contrast on five row colors in both themes, click, save and undo', flush=True)


def native_checkboxes(wid):
    g = geometry(wid)
    theme = os.environ.get('ICED_THEME', 'Light')
    picture = Image.open(screenshot('native-checkboxes-' + theme)).convert('RGB').crop(
        (g['X'], g['Y'], g['X'] + g['WIDTH'], g['Y'] + g['HEIGHT']))
    row_checks = runpy.run_path(str(Path(__file__).with_name('row_checks.py')))
    boxes = row_checks['checkboxes'](picture, scale, theme == 'Dark')
    assert len(boxes) == 5, boxes
    layout = row_checks['alignment'](picture, scale, boxes, theme == 'Dark')
    (output / 'row-layout.json').write_text(json.dumps(layout, indent=2) + '\n')
    evidence = []
    for i, box in enumerate(boxes):
        left, top, right, bottom = box
        crop = picture.crop(box)
        row = picture.getpixel((right + 2 * scale, (top + bottom) // 2))
        colors = Counter(crop.getdata())
        outline = max((rgb for rgb, count in colors.most_common(8)), key=lambda rgb: contrast(rgb, row))
        ratio = contrast(outline, row)
        face = (25, 25, 28) if theme == 'Dark' else (252, 252, 252)
        evidence.append({'row': i, 'theme': theme, 'contrast': round(ratio, 2), 'face_pixels': colors[face]})
        assert ratio >= 3, evidence[-1]
        assert colors[face] >= crop.width * crop.height * .4, evidence[-1]
    (output / 'checkboxes.json').write_text(json.dumps(evidence, indent=2) + '\n')
    baseline = (root / 'Pin.md').read_text()
    left, top, right, bottom = boxes[0]
    move(g['X'] + (left + right) / 2, g['Y'] + (top + bottom) / 2)
    input_event('NotifyPointerButton', '(ib)', (272, True))
    input_event('NotifyPointerButton', '(ib)', (272, False))
    time.sleep(.6)
    assert '- [x] ==🟢Green priority==' in (root / 'Pin.md').read_text(), 'Native checkbox saves completion'
    move(g['X'] + g['WIDTH'] - 167 * scale, g['Y'] + 21 * scale)
    input_event('NotifyPointerButton', '(ib)', (272, True))
    input_event('NotifyPointerButton', '(ib)', (272, False))
    time.sleep(.6)
    assert (root / 'Pin.md').read_text() == baseline, 'Native undo restores completion and order'
    print('PASS: native Pin checkbox contrast, completion, save and undo', flush=True)


def black_strip(path, before, after):
    # Check the composited desktop: raw window captures can expose transparent
    # pixels that appear black when converted to RGB. The isolated grey desktop
    # distinguishes visible black artifacts from space exposed during a drag.
    left, top = max(before['X'], after['X']), max(before['Y'], after['Y'])
    width = min(before['X'] + before['WIDTH'], after['X'] + after['WIDTH']) - left
    height = min(before['Y'] + before['HEIGHT'], after['Y'] + after['HEIGHT']) - top
    image = Image.open(path).convert('RGB').crop((left + 3, top + 3, left + width - 3, top + height - 3))
    pixels = image.load()
    min_width = width - 6
    min_height = height - 6
    body = image.crop((0, round(50 * scale), min_width, min_height))
    dark = os.environ.get('ICED_THEME') == 'Dark'
    visible = sum((min(pixel) >= 15 and max(pixel) < 80) if dark else min(pixel) > 210 for pixel in body.getdata())
    assert visible > body.width * body.height / 2, 'The painted app must remain visible'
    rows = sum(sum(max(pixels[x, y]) < 12 for x in range(image.width)) > min_width * .8 for y in range(image.height))
    columns = sum(sum(max(pixels[x, y]) < 12 for y in range(image.height)) > min_height * .8 for x in range(image.width))
    return max(rows, columns)


records = []
children = []
input_event('Start')
input_event('NotifyPointerMotionRelative', '(dd)', (-10000., -10000.))
time.sleep(.1)
try:
    for kind, binary in [('pin', pin), ('notes', notes)]:
        with (output / (kind + '.log')).open('w') as log:
            process = subprocess.Popen([binary], env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        children.append(process)
        wid = None
        for _ in range(250):
            if process.poll() is not None:
                raise AssertionError(kind + ' exited during startup')
            try:
                wid = x('search', '--onlyvisible', '--pid', process.pid).splitlines()[0]
                break
            except (subprocess.CalledProcessError, IndexError):
                time.sleep(.1)
        assert wid, kind + ' did not open'
        time.sleep(1)
        x('windowmove', wid, 80, 60)
        x('windowsize', wid, 430 * scale, 360 * scale)
        x('windowactivate', '--sync', wid)
        time.sleep(.5)
        if kind == 'pin':
            if Path(pin).name == 'pin-iced':
                native_checkboxes(wid)
            else:
                checkboxes(process.pid)
        for axis in ['width', 'height', 'top-left']:
            g = geometry(wid)
            if axis == 'top-left':
                x('windowmove', wid, 550 * scale, 450 * scale)
                x('windowsize', wid, 430 * scale, 360 * scale)
                time.sleep(.3)
                g = geometry(wid)
            sx = g['X'] + (10 * scale if axis == 'top-left' else g['WIDTH'] - 2)
            sy = g['Y'] + (10 * scale if axis == 'top-left' else g['HEIGHT'] // 2 if axis == 'width' else g['HEIGHT'] - 2)
            move(sx, sy)
            time.sleep(.1)
            input_event('NotifyPointerButton', '(ib)', (272, True))
            time.sleep(.05)
            stop = threading.Event()
            failures = []

            def motion():
                began = time.monotonic()
                try:
                    while not stop.is_set():
                        delta = (280 if axis == 'top-left' else 100) * scale * math.sin((time.monotonic() - began) * 6)
                        move(sx + delta if axis in ['width', 'top-left'] else sx, sy if axis == 'width' else sy + delta)
                        time.sleep(.016)
                except BaseException as error:
                    failures.append(error)

            thread = threading.Thread(target=motion)
            thread.start()
            try:
                for frame in range(8):
                    before = geometry(wid)
                    path = screenshot(f'{kind}-{axis}-{frame:02}')
                    after = geometry(wid)
                    strip = black_strip(path, before, after)
                    record = {'app': kind, 'axis': axis, 'frame': frame, 'black_strip_pixels': strip, 'before': before, 'after': after}
                    if axis == 'top-left':
                        picture = Image.open(path).convert('RGB')
                        channels = [c.point(lambda value: 255 if value > 210 else 0) for c in picture.split()]
                        mask = ImageChops.multiply(ImageChops.multiply(channels[0], channels[1]), channels[2])
                        bounds = mask.crop((0, round(40 * scale), picture.width, picture.height)).getbbox()
                        assert bounds, 'The window disappeared during a corner drag'
                        record['painted_bounds'] = [bounds[0], bounds[1] + 40 * scale, bounds[2], bounds[3] + 40 * scale]
                        record['anchor_error'] = max(abs(bounds[2] - (g['X'] + g['WIDTH'])), abs(bounds[3] + 40 * scale - (g['Y'] + g['HEIGHT'])))
                    records.append(record)
            finally:
                stop.set()
                thread.join(timeout=4)
                input_event('NotifyPointerButton', '(ib)', (272, False))
            assert not failures, failures
            print('CAPTURED', kind, axis, flush=True)
            time.sleep(.3)
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=5)
    (output / 'result.json').write_text(json.dumps({'scale': scale, 'frames': records}, indent=2) + '\n')
    bad = [r for r in records if r['black_strip_pixels'] > 2 * scale]
    assert not bad, f'{len(bad)} frames expose black strips: {bad[:3]}'
    bad = [r for r in records if r.get('anchor_error', 0) > 2 * scale]
    assert not bad, f'{len(bad)} frames move the fixed opposite corner: {bad[:3]}'
    print('PASS: continuous horizontal and vertical drags retain both app pictures', flush=True)
finally:
    input_event('NotifyPointerButton', '(ib)', (272, False))
    input_event('Stop')
    for child in reversed(children):
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=4)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
