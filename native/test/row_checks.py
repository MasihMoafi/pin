"""Measure checkbox spacing and control alignment in the rendered Pin window."""


def checkboxes(picture, scale, dark=False):
    outline = (161, 161, 170) if dark else (116, 116, 124)
    pixels = picture.convert('RGB').load()
    candidates = {(x, y) for y in range(50 * scale, picture.height)
                  for x in range(30 * scale, min(100 * scale, picture.width))
                  if max(abs(a - b) for a, b in zip(pixels[x, y], outline)) < 40}
    boxes = []
    while candidates:
        pending = [candidates.pop()]
        component = []
        while pending:
            x, y = pending.pop()
            component.append((x, y))
            for dx, dy in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]:
                point = (x + dx, y + dy)
                if point in candidates:
                    candidates.remove(point)
                    pending.append(point)
        xs, ys = zip(*component)
        left, top, right, bottom = min(xs), min(ys), max(xs) + 1, max(ys) + 1
        if 15 * scale <= right - left <= 19 * scale and 15 * scale <= bottom - top <= 19 * scale:
            boxes.append((left, top, right, bottom))
    return sorted(boxes, key=lambda b: b[1])


def alignment(picture, scale, boxes, dark=False):
    pixels = picture.convert('RGB').load()
    outline = (161, 161, 170) if dark else (116, 116, 124)
    records = []
    for left, top, right, bottom in boxes:
        center = (top + bottom - 1) / 2
        grip = [(x, y) for y in range(top - 3 * scale, bottom + 3 * scale)
                for x in range(picture.width - 80 * scale, picture.width - 20 * scale)
                if max(abs(a - b) for a, b in zip(pixels[x, y], outline)) < 20]
        assert grip, 'Drag handle must remain visible'
        grip_center = (min(y for _, y in grip) + max(y for _, y in grip)) / 2
        text_x = [x for y in range(top, bottom) for x in range(right, right + 30 * scale)
                  if (min(pixels[x, y]) > 210 if dark else max(pixels[x, y]) < 80)]
        assert text_x, 'Task text must remain visible next to the checkbox'
        gap = (min(text_x) - right) / scale
        offset = abs(grip_center - center) / scale
        record = {'checkbox': [left, top, right, bottom], 'text_gap': gap, 'control_offset': offset}
        assert gap >= 9, ('Checkbox is crowded against its text', record)
        assert offset <= 1.5, ('Row controls do not share the first-line center', record)
        records.append(record)
    assert all((b[1] - a[1]) / scale >= 50 for a, b in zip(boxes, boxes[1:])), ('Task rows are cramped', boxes)
    return records
