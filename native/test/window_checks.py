"""Shared real-edge resize checks for the native windows on hidden X11 displays."""
import json
from pathlib import Path
import subprocess
import time
from PIL import ImageGrab


def resize(env, wid, scale, output, prefix, dark=False):
    def x(*args):
        return subprocess.check_output(['xdotool', *map(str, args)], env=env, text=True).strip()

    def geometry():
        return {k: int(v) for k, v in (s.split('=', 1) for s in x('getwindowgeometry', '--shell', wid).splitlines())}

    records = []
    original = geometry()
    for edge, dx, dy in [('left', -1, 0), ('right', 1, 0), ('top', 0, -1), ('bottom', 0, 1),
                         ('top-left', -1, -1), ('top-right', 1, -1), ('bottom-left', -1, 1), ('bottom-right', 1, 1)]:
        # Leave room above and left for the compositor to follow every direction.
        x('windowsize', wid, original['WIDTH'], original['HEIGHT'])
        x('windowmove', wid, 100 * scale, 100 * scale)
        time.sleep(.15)
        before = geometry()
        inset = (12 if dx and dy else 6) * scale
        px = inset if dx < 0 else before['WIDTH'] - inset if dx > 0 else before['WIDTH'] // 2
        py = inset if dy < 0 else before['HEIGHT'] - inset if dy > 0 else before['HEIGHT'] // 2
        start_x, start_y = before['X'] + px, before['Y'] + py
        x('mousemove', start_x, start_y, 'mousedown', '1')
        time.sleep(.15)
        try:
            for i, delta in enumerate([20, 40, 60, 80]):
                x('mousemove', start_x + dx * delta * scale, start_y + dy * delta * scale)
                time.sleep(.06)
                g = geometry()
                expected = {'X': before['X'] + min(dx, 0) * delta * scale,
                            'Y': before['Y'] + min(dy, 0) * delta * scale,
                            'WIDTH': before['WIDTH'] + abs(dx) * delta * scale,
                            'HEIGHT': before['HEIGHT'] + abs(dy) * delta * scale}
                assert all(abs(g[k] - v) <= 2 * scale for k, v in expected.items()), ('Resize must start inside a usable border and follow the pointer', edge, g, expected)
                image = ImageGrab.grab(xdisplay=env['DISPLAY']).crop((g['X'], g['Y'], g['X'] + g['WIDTH'], g['Y'] + g['HEIGHT'])).convert('RGB')
                image.save(Path(output) / f'{prefix}-{edge}-{i}.png')
                width, height = image.size
                pixels = image.load()
                black_rows = sum(sum(max(pixels[xx, yy]) < 12 for xx in range(3, width - 3)) > (width - 6) * .8 for yy in range(max(3, height - 20 * scale), height - 3))
                black_columns = sum(sum(max(pixels[xx, yy]) < 12 for yy in range(3, height - 3)) > (height - 6) * .8 for xx in range(max(3, width - 20 * scale), width - 3))
                region = image.crop((12 * scale, 55 * scale, min(width - 12 * scale, 230 * scale), min(height, 115 * scale)))
                ink = sum(max(rgb) > 200 if dark else max(rgb) < 100 for rgb in region.getdata())
                record = {'edge': edge, 'frame': i, 'geometry': g, 'black_rows': black_rows, 'black_columns': black_columns, 'visible_ink': ink}
                records.append(record)
                assert max(black_rows, black_columns) <= 2 * scale, record
                assert ink > 10 * scale * scale, ('Resizing erased the painted content', record)
        finally:
            x('mouseup', '1')
        time.sleep(.2)
    x('windowsize', wid, original['WIDTH'], original['HEIGHT'])
    x('windowmove', wid, original['X'], original['Y'])
    (Path(output) / f'{prefix}-result.json').write_text(json.dumps(records, indent=2) + '\n')
    return records
