#!/usr/bin/env python3
"""Exercise Markdown in a real Notes window on an isolated X11 display."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
from PIL import ImageGrab, ImageChops

binary = str(Path(sys.argv[1]).resolve())
output = Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
root = Path(tempfile.mkdtemp(prefix='notes-markdown-', dir=output))
env = dict(os.environ, WINIT_X11_SCALE_FACTOR='1', LIBGL_ALWAYS_SOFTWARE='1', ICED_THEME='Light')
children = []
checks = []
wid = None


def x(*args):
    result = subprocess.check_output(['xdotool', *map(str, args)], env=env, text=True, stderr=subprocess.DEVNULL).strip()
    if args[0] in ('key', 'type', 'windowsize'):
        time.sleep(.15)
    return result


def wait(test, label):
    until = time.monotonic() + 15
    while time.monotonic() < until:
        try:
            value = test()
            if value:
                return value
        except (OSError, subprocess.CalledProcessError, IndexError):
            pass
        time.sleep(.05)
    raise AssertionError(label)


def shot(name):
    time.sleep(.35)
    bounds = {k: int(v) for k, v in (line.split('=', 1) for line in x('getwindowgeometry', '--shell', wid).splitlines())}
    image = ImageGrab.grab(xdisplay=env['DISPLAY']).crop((bounds['X'], bounds['Y'], bounds['X'] + bounds['WIDTH'], bounds['Y'] + bounds['HEIGHT']))
    image.save(output / (name + '.png'))
    return image


def start(name, content, dark=False):
    global wid
    case = root / name
    config = case / 'config/StickyNotes'
    config.mkdir(parents=True, exist_ok=True)
    file = case / 'Notes.md'
    file.write_text(content)
    (config / 'iced.json').write_text(json.dumps({'width': 430, 'height': 360, 'x': 30, 'y': 30, 'pinned': False}))
    launch_env = dict(env, STICKY_NOTES_FILE=str(file), XDG_CONFIG_HOME=str(case / 'config'), ICED_THEME='Dark' if dark else 'Light')
    with (output / (name + '.log')).open('w') as log:
        process = subprocess.Popen([binary], env=launch_env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    children.append(process)
    wid = wait(lambda: x('search', '--onlyvisible', '--pid', process.pid).splitlines()[0], 'Notes opens')
    x('windowfocus', wid)
    time.sleep(.7)
    return process, file


def close(process):
    bounds = {k: int(v) for k, v in (line.split('=', 1) for line in x('getwindowgeometry', '--shell', wid).splitlines())}
    x('mousemove', '--window', wid, bounds['WIDTH'] - 25, 21, 'click', 1)
    process.wait(timeout=10)
    assert process.returncode == 0


def saved(file, expected):
    wait(lambda: file.read_text() == expected, 'Exact Markdown must save')


def check(label):
    checks.append(label)
    print('PASS', label, flush=True)


def font_check(image):
    # One H in each face, on the same baseline: normal, bold, italic, both.
    # Ignore the separate asterisk glyphs when comparing weight and slant.
    line = image.convert('L').crop((18, 57, 410, 86))
    occupied = [any(line.getpixel((x, y)) < 160 for y in range(line.height)) for x in range(line.width)]
    groups = []
    for x, ink in enumerate(occupied):
        if ink and (x == 0 or not occupied[x - 1]):
            groups.append([x, x + 1])
        elif ink:
            groups[-1][1] = x + 1
    assert len(groups) >= 15, ('Unexpected glyph layout', groups)
    def glyph(index):
        start, end = groups[index]
        points = [(x - start, y) for x in range(start, end) for y in range(line.height) if line.getpixel((x, y)) < 160]
        top, bottom = min(y for _, y in points), max(y for _, y in points)
        top_x = [x for x, y in points if y <= top + 1]
        bottom_x = [x for x, y in points if y >= bottom - 1]
        return len(points), sum(top_x) / len(top_x) - sum(bottom_x) / len(bottom_x)
    normal, bold, italic, both = [glyph(i) for i in (0, 3, 8, 14)]
    assert bold[0] > normal[0] + 5, ('Live bold must draw heavier glyphs', normal, bold)
    assert italic[1] > normal[1] + .5, ('Live italics must draw slanted glyphs', normal, italic)
    assert both[0] > italic[0] + 5 and both[1] > bold[1] + .5, ('Nested emphasis must be bold and italic', normal, bold, italic, both)


try:
    p, file = start('emphasis', '**Bold** and *italic* and ***both***.\n\nسلام **پررنگ** و *کج*.\n')
    source = file.read_text()
    stamp = file.stat().st_mtime_ns
    editing = shot('01-live-emphasis')
    x('key', 'ctrl+shift+m')
    reading = shot('02-rendered-emphasis')
    # The caret is on the trailing empty line, outside this first-line crop.
    difference = ImageChops.difference(editing.crop((18, 57, 400, 86)), reading.crop((18, 57, 400, 86)))
    assert sum(max(pixel[:3]) > 12 for pixel in difference.getdata()) > 150, 'Reading view must render the first line without Markdown markers'
    assert file.read_text() == source and file.stat().st_mtime_ns == stamp
    x('key', 'ctrl+shift+m')
    x('key', 'ctrl+a', 'ctrl+c')
    assert subprocess.check_output(['xclip', '-selection', 'clipboard', '-out'], env=env).decode() == source
    x('key', 'ctrl+End')
    # Each styled H is followed by a space, so italic overhang cannot touch a marker.
    file.write_text('H **H H** *H H* ***H H***\n')
    time.sleep(2.5)
    x('key', 'Tab')  # Keep the blinking caret outside the measured text.
    font_check(shot('09-live-font-faces'))
    check('Bold, italic, nested and Persian emphasis; reading toggle preserves exact Markdown')
    close(p)

    literals = '# Markdown\n\n- **A bold item**\n- *An italic item*\n\n\\*literal stars\\*, snake_case_name and unmatched *\n\n`**literal code**`\n\n```\n*plain code block*\n```\n'
    p, file = start('literal-code', literals, dark=True)
    shot('03-live-code-dark')
    x('key', 'ctrl+shift+m')
    shot('04-rendered-code-dark')
    x('windowsize', wid, 300, 420)
    shot('05-rendered-narrow')
    assert file.read_text() == literals
    x('key', 'ctrl+shift+m')
    x('key', 'ctrl+a', 'ctrl+c')
    assert subprocess.check_output(['xclip', '-selection', 'clipboard', '-out'], env=env).decode() == literals
    check('Escaped and unmatched markers, lists, inline and fenced code; dark and narrow rendering')
    close(p)

    p, file = start('edit-reload', '**Original** and *emphasis*.\n')
    original = file.read_text()
    x('key', 'ctrl+shift+m')
    x('key', 'ctrl+shift+m')
    x('key', 'ctrl+End')
    x('type', '--clearmodifiers', '--delay', 10, '**Added** and *more*.')
    edited = original + '**Added** and *more*.'
    saved(file, edited)
    shot('06-live-edited')
    x('key', 'ctrl+z')
    saved(file, original)
    x('key', 'ctrl+shift+z')
    saved(file, edited)
    x('key', 'ctrl+shift+m')
    shot('07-rendered-edited')
    external = '# Changed elsewhere\n\n**External bold** and *external italic*.\n'
    file.write_text(external)
    time.sleep(2.5)
    shot('08-external-preview')
    x('key', 'ctrl+shift+m')
    x('key', 'ctrl+a', 'ctrl+c')
    assert subprocess.check_output(['xclip', '-selection', 'clipboard', '-out'], env=env).decode() == external
    close(p)
    check('Edit after reading, autosave, undo/redo and external reload preserve formatted source')
    (output / 'result.json').write_text(json.dumps({'checks': checks, 'binary': binary}, indent=2) + '\n')
finally:
    for process in reversed(children):
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=4)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
