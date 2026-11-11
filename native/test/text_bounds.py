#!/usr/bin/env python3
"""Keep note text inside its viewport through scrolling, editing and resizing."""
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

from PIL import ImageGrab

binary = str(Path(sys.argv[1]).resolve())
output = Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
root = Path(tempfile.mkdtemp(prefix='text-bounds-', dir=output))
assert os.environ['DISPLAY'] != ':0', 'Use a hidden display'
children = []
records = []
failures = []
sentence = 'I read the questions carefully and explain each answer in my own words. '
cases = {
    'short': 'A short note that fits without scrolling.\n',
    'wrapped': sentence * 35 + 'The final sentence stays readable.',
    'formatted': '\n'.join(f'{i}: **Earlier bold text** and *italic words* with `code`. ' + sentence * 2 for i in range(55)) + '\nThe final sentence stays readable.',
}


def wait(test, label, seconds=15):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            value = test()
            if value:
                return value
        except (OSError, subprocess.CalledProcessError, IndexError):
            pass
        time.sleep(.05)
    raise AssertionError(label)


def x(*args):
    return subprocess.check_output(['xdotool', *map(str, args)], env=env, text=True, stderr=subprocess.DEVNULL).strip()


def geometry():
    return {k: int(v) for k, v in (line.split('=', 1) for line in x('getwindowgeometry', '--shell', wid).splitlines())}


def capture(action):
    time.sleep(.3)
    bounds = geometry()
    image = ImageGrab.grab(xdisplay=env['DISPLAY']).crop((bounds['X'], bounds['Y'], bounds['X'] + bounds['WIDTH'], bounds['Y'] + bounds['HEIGHT'])).convert('RGB')
    label = f'{name}-{scale:g}-{action}'
    image.save(output / (label + '.png'))
    width, height = image.size
    background = image.getpixel((width - round(10 * scale), height - round(8 * scale)))
    # Text ends 14 logical pixels above the bottom. Ignore the outer border.
    def ink_in(top, bottom):
        strip = image.crop((math.ceil(20 * scale), top, width - math.ceil(20 * scale), bottom))
        pixels = strip.load()
        return sum(max(abs(a - b) for a, b in zip(pixels[x, y], background)) > 40 for y in range(strip.height) for x in range(strip.width))
    ink = ink_in(height - math.floor(12 * scale), height - math.ceil(2 * scale))
    top_ink = ink_in(math.ceil(45 * scale), math.floor(55 * scale))
    record = {'case': name, 'scale': scale, 'action': action, 'bottom_text_pixels': ink, 'top_text_pixels': top_ink, 'size': [width, height]}
    records.append(record)
    if ink or top_ink:
        failures.append(record)


try:
    for scale in map(float, os.environ.get('NOTES_TEST_SCALES', '1,1.25,2').split(',')):
        for name, initial in cases.items():
            case = root / f'{name}-{scale:g}'
            config = case / 'config/StickyNotes'
            config.mkdir(parents=True)
            file = case / 'Notes.md'
            file.write_text(initial)
            (config / 'iced.json').write_text(json.dumps({'width': 430, 'height': 272, 'x': 30, 'y': 30, 'pinned': False}))
            env = dict(os.environ, STICKY_NOTES_FILE=str(file), XDG_CONFIG_HOME=str(case / 'config'), WINIT_X11_SCALE_FACTOR=str(scale), LIBGL_ALWAYS_SOFTWARE='1', ICED_THEME='Dark' if name == 'formatted' else 'Light')
            with (output / f'{name}-{scale:g}.log').open('w') as log:
                process = subprocess.Popen([binary], env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            children.append(process)
            wid = wait(lambda: x('search', '--onlyvisible', '--pid', process.pid).splitlines()[0], 'Notes opens')
            # Header tooltips are intentional overlays, outside the text being tested.
            screen_width, screen_height = map(int, x('getdisplaygeometry').split())
            x('mousemove', screen_width - 2, screen_height - 2)
            x('windowfocus', wid)
            time.sleep(.5)
            capture('opened')
            x('key', 'ctrl+Home')
            capture('scroll-top')
            x('key', 'ctrl+End')
            capture('scroll-bottom')
            x('type', '--clearmodifiers', '--delay', 30, ' Appended words.')
            wait(lambda: file.read_text() == initial + ' Appended words.', 'Typing saves exact text')
            capture('typed')
            x('key', 'ctrl+z')
            wait(lambda: file.read_text() == initial, 'Undo restores exact note')
            capture('undo')
            x('windowsize', wid, round(315 * scale), round(247 * scale))
            capture('narrow')
            x('key', 'ctrl+Home')
            capture('narrow-top')
            x('key', 'ctrl+End')
            capture('narrow-bottom')
            bounds = geometry()
            x('mousemove', '--window', wid, bounds['WIDTH'] - round(25 * scale), round(21 * scale), 'click', 1)
            process.wait(timeout=10)
            assert process.returncode == 0
            print('CHECKED', name, 'scale', scale, flush=True)
    (output / 'result.json').write_text(json.dumps({'records': records, 'failures': failures}, indent=2) + '\n')
    assert not failures, f'Text escaped the editor viewport: {failures[:5]} ({len(failures)} frames)'
    print('PASS short notes, wrapped scrolling and formatted editing/resizing retain a clear bottom margin', flush=True)
finally:
    for process in reversed(children):
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=4)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
