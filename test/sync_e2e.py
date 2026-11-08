"""Exercise real file conflicts through Pin's window on a private display."""

import base64
import json
import sys
import time
import urllib.request
from pathlib import Path

import websocket

file = Path(sys.argv[1])
port = int(sys.argv[2])
shots = Path(sys.argv[3])
fixture = "- [ ] ==🟢Original==\n- [ ] ==🔵External==\n"
file.write_text(fixture)
tabs = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json"))
ws = websocket.create_connection(
    next(tab for tab in tabs if tab["type"] == "page")["webSocketDebuggerUrl"],
    suppress_origin=True,
)
seq = 0


def call(method, params=None):
    global seq
    seq += 1
    ws.send(json.dumps({"id": seq, "method": method, "params": params or {}}))
    while True:
        result = json.loads(ws.recv())
        if result.get("id") == seq:
            if "error" in result:
                raise RuntimeError(result["error"])
            return result["result"]


def ev(expression):
    result = call("Runtime.evaluate", {"expression": expression, "returnByValue": True})
    if "exceptionDetails" in result:
        raise RuntimeError(result["exceptionDetails"])
    return result["result"].get("value")


def replace(text):
    temp = file.with_name(file.name + ".external")
    temp.write_text(text)
    temp.replace(file)


def check(name, condition):
    print(("PASS " if condition else "FAIL ") + name, flush=True)
    if not condition:
        raise AssertionError(name)


call("Page.reload")
time.sleep(2)
ev("""(() => {
  const title = document.querySelector('.title');
  title.focus();
  const range = document.createRange();
  range.selectNodeContents(title);
  range.collapse(false);
  getSelection().removeAllRanges();
  getSelection().addRange(range);
})()""")
call("Input.insertText", {"text": " local"})
disk = fixture.replace("External", "Disk change")
replace(disk)
time.sleep(1.5)
check("a pending edit does not overwrite an external atomic replacement", file.read_text() == disk)
check("the local edit remains visible", "Original local" in ev("document.body.innerText"))
check("Pin asks how to resolve the conflict", ev("!!document.querySelector('#sync-notice:not([hidden])')"))


def wait_for(predicate, timeout=4):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if predicate():
            return True
        time.sleep(0.1)
    return bool(predicate())


ev("document.getElementById('sync-disk').click()")
check("choosing disk loads the external version", wait_for(
    lambda: "Disk change" in ev("document.body.innerText") and
    ev("document.getElementById('sync-notice').hidden")
))
ev("document.activeElement.blur()")
idle = disk.replace("Disk change", "Idle replacement")
replace(idle)
check("an idle window reloads an atomic replacement", wait_for(
    lambda: "Idle replacement" in ev("document.body.innerText")
))
ev("""(() => { const t = document.querySelector('.title'); t.focus();
  const r = document.createRange(); r.selectNodeContents(t); r.collapse(false);
  getSelection().removeAllRanges(); getSelection().addRange(r); })()""")
call("Input.insertText", {"text": " mine"})
replace(idle.replace("Idle replacement", "Conflicting disk edit"))
check("a second conflict pauses saving", wait_for(
    lambda: not ev("document.getElementById('sync-notice').hidden")
))
ev("document.getElementById('sync-mine').click()")
check("choosing mine saves the local version explicitly", wait_for(
    lambda: "Original mine" in file.read_text() and "Idle replacement" in file.read_text()
))

# A focused but unchanged title is preserved until the user makes a choice.
ev("document.querySelector('.title').focus()")
latest = file.read_text().replace("Idle replacement", "New disk value")
replace(latest)
check("a disk edit while a title has focus asks before changing the view", wait_for(
    lambda: not ev("document.getElementById('sync-notice').hidden")
))
replace(latest.replace("New disk value", "Newest disk value"))
ev("document.getElementById('sync-disk').click()")
check("using disk reads its latest version", wait_for(
    lambda: "Newest disk value" in ev("document.body.innerText")
))

# An unsaved date in the popover is editing too.
ev("document.querySelector('.more').click()")
ev("[...document.querySelectorAll('#menu button')].find(b => b.textContent.startsWith('Add date')).click()")
ev("document.querySelector('#menu input[type=date]').value = '2033-02-03'")
before = file.read_text()
replace(before.replace("Newest disk value", "From sync"))
check("a disk edit preserves an open date field", wait_for(
    lambda: not ev("document.getElementById('sync-notice').hidden")
) and ev("document.querySelector('#menu input[type=date]').value") == '2033-02-03')
ev("document.getElementById('sync-disk').click()")
check("the selected disk version closes the date field", wait_for(
    lambda: ev("document.getElementById('menu').hidden") and "From sync" in ev("document.body.innerText")
))

# A read/write error must be visible and keep the pending edit for retry.
before = file.read_text()
ev("""(() => { const t = document.querySelector('.title'); t.focus();
  const r = document.createRange(); r.selectNodeContents(t); r.collapse(false);
  getSelection().removeAllRanges(); getSelection().addRange(r); })()""")
call("Input.insertText", {"text": " pending"})
file.unlink()
file.mkdir()
check("a storage error leaves the edit visible", wait_for(
    lambda: not ev("document.getElementById('sync-retry').hidden")
) and "pending" in ev("document.body.innerText"))
file.rmdir()
file.write_text(before)
ev("document.getElementById('sync-retry').click()")
check("retry saves the retained edit", wait_for(lambda: "pending" in file.read_text()))

named = ["🟢", "🔵", "🟣", "🟠", "🟡", "🔴"]
extra = ["#ec4899", "#14b8a6", "#06b6d4", "#84cc16", "#a16207", "#64748b"]
colors = "".join(f"- [ ] =={color}Task {i}==\n" for i, color in enumerate(named, 1))
colors += "".join(f'- [ ] <mark style="background: {color};">Task {i}</mark>\n'
                  for i, color in enumerate(extra, 7))
ev("document.activeElement.blur()")
replace(colors)
check("all twelve colors load into the window", wait_for(
    lambda: ev("document.querySelectorAll('.row').length") == 12
))

# Calculate contrast from Chromium's rendered colors, including done and date labels.
contrast = """(() => {
  const canvas = document.createElement('canvas'); canvas.width = canvas.height = 1;
  const ctx = canvas.getContext('2d');
  const rgb = value => { ctx.clearRect(0, 0, 1, 1); ctx.fillStyle = value; ctx.fillRect(0, 0, 1, 1); return [...ctx.getImageData(0, 0, 1, 1).data].slice(0, 3); };
  const luminance = color => color.map(v => { v /= 255; return v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4; }).reduce((a, v, i) => a + v * [.2126, .7152, .0722][i], 0);
  const ratio = (a, b) => { const [lo, hi] = [luminance(a), luminance(b)].sort((x, y) => x - y); return (hi + .05) / (lo + .05); };
  return [...document.querySelectorAll('.row')].map(row => {
    const bg = rgb(getComputedStyle(row).backgroundColor), title = row.querySelector('.title'), when = row.querySelector('.when');
    const text = ratio(rgb(getComputedStyle(title).color), bg);
    row.classList.add('done'); const done = ratio(rgb(getComputedStyle(title).color), bg); row.classList.remove('done');
    when.classList.add('today'); const today = ratio(rgb(getComputedStyle(when).color), bg); when.classList.remove('today');
    when.classList.add('late'); const late = ratio(rgb(getComputedStyle(when).color), bg); when.classList.remove('late');
    return {text, done, today, late};
  });
})()"""
backgrounds = []
for theme in ("light", "dark"):
    call("Emulation.setEmulatedMedia", {"features": [{"name": "prefers-color-scheme", "value": theme}]})
    check(f"{theme} preference is active", ev(f"matchMedia('(prefers-color-scheme: {theme})').matches"))
    backgrounds.append(ev("getComputedStyle(document.body).backgroundColor"))
    ratios = ev(contrast)
    check(f"{theme} task colors, done text and dates have readable contrast",
          len(ratios) == 12 and min(value for row in ratios for value in row.values()) >= 4.5)
    ev("document.querySelector('.dot').click()")
    check(f"{theme} color menu opens", ev("!document.getElementById('menu').hidden"))
    image = call("Page.captureScreenshot", {"format": "png"})["data"]
    (shots / f"{theme}.png").write_bytes(base64.b64decode(image))
    ev("document.querySelector('.dot').click()")
    ev("document.querySelector('.more').click()")
    ev("[...document.querySelectorAll('#menu button')].find(b => b.textContent.startsWith('Add date')).click()")
    check(f"{theme} date controls open", ev("!!document.querySelector('#menu input[type=date]')"))
    ev("document.getElementById('menu').hidden = true; document.activeElement.blur()")
check("the same window changes its palette live", backgrounds[0] != backgrounds[1])
ws.close()
