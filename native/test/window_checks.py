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
    for axis in ['width', 'height']:
        before = geometry()
        px = before['WIDTH'] - 2 if axis == 'width' else before['WIDTH'] // 2
        py = before['HEIGHT'] // 2 if axis == 'width' else before['HEIGHT'] - 2
        x('mousemove', '--window', wid, px, py, 'mousedown', '1')
        time.sleep(.15)
        try:
            for i, delta in enumerate([20, 40, 60, 80]):
                x('mousemove', '--window', wid, px + (delta * scale if axis == 'width' else 0), py + (delta * scale if axis == 'height' else 0))
                time.sleep(.015)
                g = geometry()
                image = ImageGrab.grab(xdisplay=env['DISPLAY']).crop((g['X'], g['Y'], g['X'] + g['WIDTH'], g['Y'] + g['HEIGHT'])).convert('RGB')
                image.save(Path(output) / f'{prefix}-{axis}-{i}.png')
                width, height = image.size
                pixels = image.load()
                black_rows = sum(sum(max(pixels[xx, yy]) < 12 for xx in range(3, width - 3)) > (width - 6) * .8 for yy in range(max(3, height - 20 * scale), height - 3))
                black_columns = sum(sum(max(pixels[xx, yy]) < 12 for yy in range(3, height - 3)) > (height - 6) * .8 for xx in range(max(3, width - 20 * scale), width - 3))
                region = image.crop((12 * scale, 55 * scale, min(width - 12 * scale, 230 * scale), min(height, 115 * scale)))
                ink = sum(max(rgb) > 200 if dark else max(rgb) < 100 for rgb in region.getdata())
                record = {'axis': axis, 'frame': i, 'geometry': g, 'black_rows': black_rows, 'black_columns': black_columns, 'visible_ink': ink}
                records.append(record)
                assert max(black_rows, black_columns) <= 2 * scale, record
                assert ink > 10 * scale * scale, ('Resizing erased the painted content', record)
        finally:
            x('mouseup', '1')
        time.sleep(.2)
        after = geometry()
        assert after['WIDTH' if axis == 'width' else 'HEIGHT'] > before['WIDTH' if axis == 'width' else 'HEIGHT'] + 20 * scale, ('The actual edge did not resize', axis, before, after)
    (Path(output) / f'{prefix}-result.json').write_text(json.dumps(records, indent=2) + '\n')
    return records
