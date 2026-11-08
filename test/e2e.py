"""Drive the Priorities sticky note through DevTools on a hidden display; check the file after each step."""

import json
import sys
import time
import urllib.request
from pathlib import Path

import websocket

FILE = Path(sys.argv[1])
FIXTURE = """Top line matters most.

- [ ] ==🟢Swarm hackathon · today's work== 📅 2026-11-07
- [ ] ==🔵Elpis · Workflow test: halve desktop CI time==
- [ ] ==🔵Elpis · Agent readiness==
- [ ] ==🟡RAG-MCP · Nothing requested==
"""
FILE.write_text(FIXTURE)
tabs = json.load(urllib.request.urlopen("http://127.0.0.1:9377/json"))
ws = websocket.create_connection(
    next(t for t in tabs if t["type"] == "page")["webSocketDebuggerUrl"],
    suppress_origin=True,
)
n = [0]


def call(m, p=None):
    n[0] += 1
    ws.send(json.dumps({"id": n[0], "method": m, "params": p or {}}))
    while True:
        r = json.loads(ws.recv())
        if r.get("id") == n[0]:
            return r


def ev(js):
    return call("Runtime.evaluate", {"expression": js, "returnByValue": True})[
        "result"
    ]["result"].get("value")


VK = {
    "t": 84,
    "d": 68,
    "Enter": 13,
    "Tab": 9,
    "Backspace": 8,
    "z": 90,
    "y": 89,
    "ArrowUp": 38,
    "ArrowDown": 40,
}


def key(k, mods=0):
    for t in ("keyDown", "keyUp"):
        call(
            "Input.dispatchKeyEvent",
            {
                "type": t,
                "key": k,
                "code": "Key" + k.upper() if len(k) == 1 else k,
                "windowsVirtualKeyCode": VK[k],
                "modifiers": mods,
            },
        )
    time.sleep(0.35)


CTRL, SHIFT, ALT = 2, 8, 1


def file():
    time.sleep(0.5)
    return FILE.read_text()


def lines():
    return [l for l in file().splitlines() if l.startswith(("-", " "))]


def focus_end(i):
    ev(
        f"(()=>{{const el=document.querySelectorAll('.title')[{i}];el.focus();const r=document.createRange();r.selectNodeContents(el);r.collapse(false);getSelection().removeAllRanges();getSelection().addRange(r);}})()"
    )


ok = True


def check(name, cond):
    global ok
    ok &= bool(cond)
    print(("PASS " if cond else "FAIL ") + name)


call("Page.reload")
time.sleep(2)
start = file()
focus_end(0)
key("Enter")
call("Input.insertText", {"text": "Write the demo"})
time.sleep(0.8)
check("Enter adds a task below and typing saves", "Write the demo" in lines()[1])
key("Tab")
check("Tab makes it a subtask", lines()[1].startswith("  - [ ] ==🟢Write the demo"))
key("Tab", SHIFT)
check("Shift+Tab lifts it", lines()[1].startswith("- [ ] ==🟢Write the demo"))
key("z", CTRL)
check("Ctrl+Z undoes the lift", lines()[1].startswith("  - "))
key("z", CTRL)
key("z", CTRL)
key("z", CTRL)
check("Ctrl+Z again undoes the subtask, typing and the new task", file() == start)
key("y", CTRL)
check("Ctrl+Y redoes", file() != start)
key("z", CTRL)
check("back to start", file() == start)
# split
ev(
    "(()=>{const el=document.querySelectorAll('.title')[1];el.focus();const r=document.createRange();r.setStart(el.firstChild,5);r.collapse(true);getSelection().removeAllRanges();getSelection().addRange(r);})()"
)
key("Enter")
L = lines()
check(
    "Enter mid-text splits the task",
    L[1].endswith("==🔵Elpis==") and "· Workflow" in L[2],
)
key("z", CTRL)
check("undo the split", file() == start)
# mouse drag selects text; Backspace deletes the selection
box = ev(
    "(()=>{const b=document.querySelectorAll('.title')[1].getBoundingClientRect();return [b.left,b.top,b.width,b.height]})()"
)
x0, y0 = box[0] + 2, box[1] + 10
call(
    "Input.dispatchMouseEvent",
    {"type": "mousePressed", "x": x0, "y": y0, "button": "left", "clickCount": 1},
)
for dx in range(10, 60, 10):
    call(
        "Input.dispatchMouseEvent",
        {"type": "mouseMoved", "x": x0 + dx, "y": y0, "button": "left", "buttons": 1},
    )
call(
    "Input.dispatchMouseEvent",
    {"type": "mouseReleased", "x": x0 + 55, "y": y0, "button": "left", "clickCount": 1},
)
sel = ev("getSelection().toString()")
check(f"mouse drag selects text ({sel!r})", len(sel) >= 3)
key("Backspace")
check("Backspace deletes the selection", sel not in lines()[1])
key("z", CTRL)
check("undo restores it", file() == start)
# Alt+Down moves; Ctrl+Enter marks done
focus_end(1)
key("ArrowDown", ALT)
check("Alt+Down moves the task down", "Workflow test" in lines()[2])
key("z", CTRL)
focus_end(1)
key("Enter", CTRL)
check("Ctrl+Enter completes the task and moves it to the bottom", lines()[-1].startswith("- [x]") and "Workflow test" in lines()[-1])
key("z", CTRL)
check("undo restores completion and priority together", file() == start)
key("y", CTRL)
check("redo restores completion and its position", lines()[-1].startswith("- [x]") and "Workflow test" in lines()[-1])
key("z", CTRL)
# arrow navigation
focus_end(0)
key("ArrowDown")
check(
    "Down arrow goes to the next task",
    ev("document.activeElement===document.querySelectorAll('.title')[1]"),
)
# menu: color and date
ev("document.querySelectorAll('.more')[1].click()")
time.sleep(0.3)
ev(
    "[...document.querySelectorAll('.swatches button')].find(b=>b.title==='Red').click()"
)
check("menu sets a color", "==🔴Elpis · Workflow" in lines()[1])
ev("document.querySelectorAll('.more')[1].click()")
time.sleep(0.3)
ev(
    "[...document.querySelectorAll('#menu button')].find(b=>b.textContent.startsWith('Add date')).click()"
)
time.sleep(0.3)
ev(
    "(()=>{const i=document.querySelectorAll('#menu input');i[0].value='2026-11-09';i[1].value='14:30';[...document.querySelectorAll('#menu button')].find(b=>b.textContent==='Save').click()})()"
)
check("date menu saves date and time", lines()[1].endswith("📅 2026-11-09 14:30"))
check(
    "date shows like Automations",
    ev("document.querySelectorAll('.when')[1].textContent")
    == ev(
        "new Date('2026-11-09T14:30').toLocaleString(undefined,{weekday:'short',day:'numeric',month:'short',hour:'2-digit',minute:'2-digit'})"
    ),
)
print("when text:", ev("document.querySelectorAll('.when')[1].textContent"))
key("z", CTRL)
key("z", CTRL)
check("undo date and color", file() == start)
# external edit appears
ev("document.activeElement.blur()")
FILE.write_text(start.replace("RAG-MCP", "RAG MCP"))
time.sleep(2.5)
check(
    "an edit made elsewhere appears",
    ev(
        "[...document.querySelectorAll('.title')].some(t=>t.textContent.startsWith('RAG MCP'))"
    ),
)
# one-click subtask, Ctrl+T, and the color dot
ev("document.querySelectorAll('.addsub')[0].click()")
time.sleep(0.6)
check("the + button adds a subtask", lines()[1].startswith("  - [ ] ==🟢=="))
key("z", CTRL)
focus_end(1)
key("t", CTRL)
check("Ctrl+T adds a subtask", lines()[2].startswith("  - [ ] ==🔵=="))
key("z", CTRL)
ev("document.querySelectorAll('.dot')[1].click()")
time.sleep(0.3)
ev(
    "[...document.querySelectorAll('.swatches button')].find(b=>b.title==='Purple').click()"
)
time.sleep(0.6)
check("clicking the dot recolors the row", "==🟣Elpis · Workflow" in lines()[1])
ev("document.querySelectorAll('.dot')[2].click()")
time.sleep(0.3)
ev(
    "[...document.querySelectorAll('.swatches button')].find(b=>b.title==='Pink').click()"
)
time.sleep(0.6)
check(
    "pink from the palette saves as a portable highlight",
    '<mark style="background: #ec4899;">Elpis · Agent readiness</mark>' in lines()[2],
)
left = ev(
    "[...document.querySelectorAll('.check')].map(c=>Math.round(c.getBoundingClientRect().left))"
)
check(f"checkboxes line up at one left edge per level ({left})", len(set(left)) <= 2)
right = ev(
    "[...document.querySelectorAll('#list > li > .row')].map(r=>Math.round(r.getBoundingClientRect().right))"
)
check(f"rows end at one right edge ({set(right)})", len(set(right)) == 1)
# one-click delete on each row, its hotkey, and the date hotkey
before = file()
ev("document.querySelectorAll('.del')[3].click()")
time.sleep(0.6)
check("the trash button deletes its task", "RAG" not in file())
key("z", CTRL)
check("undo brings the deleted task back", file() == before)
focus_end(3)
key("Backspace", CTRL | SHIFT)
check("Ctrl+Shift+Backspace deletes the task", "RAG" not in file())
key("z", CTRL)
focus_end(1)
key("d", CTRL)
check(
    "Ctrl+D opens the date menu",
    ev(
        "!document.getElementById('menu').hidden && !!document.querySelector('#menu input[type=date]')"
    ),
)
# subtasks: a new one goes on top; a task folds its subtasks away and stays folded (8 Nov)
ev("document.getElementById('menu').hidden = true; document.activeElement.blur()")
FILE.write_text("- [ ] ==🟢Parent==\n  - [ ] ==🟢Old child==\n- [ ] ==🔵Next==\n")
time.sleep(2.5)
ev("document.querySelectorAll('.addsub')[0].click()")
time.sleep(0.6)
check(
    "a new subtask goes on top",
    lines()[1].startswith("  - [ ] ==🟢==") and "Old child" in lines()[2],
)
key("z", CTRL)
titles = "[...document.querySelectorAll('.title')].map(t=>t.textContent)"
ev("document.querySelector('.fold').click()")
time.sleep(0.4)
check(
    "the fold arrow hides the subtasks",
    "Old child" not in ev(titles) and "Next" in ev(titles),
)
check("folding leaves the file alone", "  - [ ] ==🟢Old child==" in file())
call("Page.reload")
time.sleep(2.5)
check("a folded task stays folded after a restart", "Old child" not in ev(titles))
ev("document.querySelector('.fold').click()")
time.sleep(0.4)
check("the fold arrow shows them again", "Old child" in ev(titles))
# Completion keeps each subtree together and keeps unfinished siblings in priority order.
ev("document.activeElement.blur()")
completion_start = (
    "- [ ] ==🟢Parent==\n"
    "  - [ ] First child\n"
    "    - [ ] Grandchild\n"
    "  - [ ] Second child\n"
    "- [ ] Next\n"
    "- [x] Previously finished\n"
)
FILE.write_text(completion_start)
time.sleep(1.5)
ev("document.querySelectorAll('.check')[1].click()")
check(
    "a completed subtask moves below its siblings with its children",
    lines()[:4] == ["- [ ] ==🟢Parent==", "  - [ ] Second child", "  - [x] First child", "    - [ ] Grandchild"],
)
key("z", CTRL)
check("undo restores the subtask and its entire branch", file() == completion_start)
ev("document.querySelector('.check').click()")
check(
    "a completed parent moves to the very bottom with its subtasks",
    lines() == ["- [ ] Next", "- [x] Previously finished", "- [x] ==🟢Parent==", "  - [ ] First child", "    - [ ] Grandchild", "  - [ ] Second child"],
)
call("Page.reload")
time.sleep(1)
check("the completed order survives a reload", ev(titles) == ["Next", "Previously finished", "Parent", "First child", "Grandchild", "Second child"])
ev("document.querySelectorAll('.check')[2].click()")
check("reopening a task puts it above completed tasks", lines()[:2] == ["- [ ] Next", "- [ ] ==🟢Parent=="])
ev("document.activeElement.blur()")
FILE.write_text("- [x] Existing done\n- [ ] Active parent\n  - [x] Done subtask\n  - [ ] Active subtask\n- [ ] Next active\n")
time.sleep(1.5)
check(
    "existing completed tasks are also moved below active siblings",
    lines() == ["- [ ] Active parent", "  - [ ] Active subtask", "  - [x] Done subtask", "- [ ] Next active", "- [x] Existing done"],
)
print("ALL PASS" if ok else "SOME FAILED")
if not ok:
    sys.exit(1)
