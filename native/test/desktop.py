#!/usr/bin/env python3
"""Run on an isolated X display and D-Bus: desktop.py BINARY OUTPUT_DIRECTORY."""
import datetime
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

import gi
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi
from PIL import ImageGrab

binary = str(Path(sys.argv[1]).resolve())
output = Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
scratch = Path(tempfile.mkdtemp(prefix="case-", dir=output))
config = scratch / "config"
note_file = scratch / "Notes.txt"
list_file = scratch / "Pin.md"
initial_note = "A first note.\n\nWrite thoughts here.\n"
note_file.write_text(initial_note)
day = datetime.date.today()
yesterday = day - datetime.timedelta(days=1)
list_file.write_text(f"- [ ] ==🟢Daily task== 📅 {day} 09:30 🔁 daily\n"
                     "  - [ ] Child task\n- [ ] Second task\n- [x] Old finished\n"
                     f"- [x] Yesterday 🔁 daily ✅ {yesterday}\n")
env = dict(os.environ, XDG_CONFIG_HOME=str(config), PRIORITIES_FILE=str(list_file),
           STICKY_NOTES_FILE=str(note_file), GDK_BACKEND="x11", GSK_RENDERER="cairo",
           GTK_A11Y="atspi", GSETTINGS_BACKEND="keyfile")
children = []
checks = []


def wait_for(check, description, seconds=8):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            value = check()
            if value:
                return value
        except Exception:
            pass
        time.sleep(0.1)
    raise AssertionError(description)


def x(*args):
    return subprocess.check_output(["xdotool", *map(str, args)], env=env, text=True).strip()


def walk(node=None):
    node = node or Atspi.get_desktop(0)
    yield node
    try:
        for i in range(node.get_child_count()):
            yield from walk(node.get_child_at_index(i))
    except Exception:
        pass


def activate(name):
    def attempt():
        for node in walk():
            try:
                if node.get_name() == name:
                    action = node.get_action_iface()
                    if action and action.get_n_actions() and action.do_action(0):
                        return True
            except Exception:
                pass
        return False
    wait_for(attempt, f"Activate {name}")
    time.sleep(0.2)


def seen(name):
    return any(node.get_name() == name for node in walk())


def saved(path, expected):
    wait_for(lambda: path.read_text() == expected, f"Saved text mismatch in {path.name}")


def shot(name):
    ImageGrab.grab(xdisplay=env["DISPLAY"]).save(output / f"{name}.png")


def start(notes):
    log = open(output / ("notes.log" if notes else "pin.log"), "a")
    proc = subprocess.Popen([binary] + (["--notes"] if notes else []), env=env,
                            stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    log.close()
    children.append(proc)
    title = "^Sticky Notes$" if notes else "^Pin$"
    wid = wait_for(lambda: x("search", "--onlyvisible", "--name", title).splitlines()[0],
                   "Window did not open", 20)
    time.sleep(0.8)
    x("windowfocus", wid)
    return proc, wid


def click(wid, px, py):
    x("mousemove", "--window", wid, px, py, "click", 1)
    time.sleep(0.1)


def key(chord):
    x("key", "--clearmodifiers", chord)


def type_text(text):
    x("type", "--clearmodifiers", "--delay", 1, text)


def geometry(wid):
    return dict(line.split("=", 1) for line in x("getwindowgeometry", "--shell", wid).splitlines())


def close(proc):
    activate("Close")
    proc.wait(timeout=8)
    assert proc.returncode == 0


def paste(text):
    clip = subprocess.Popen(["xclip", "-quiet", "-in", "-selection", "clipboard"], env=env,
                            stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                            start_new_session=True)
    children.append(clip)
    clip.stdin.write(text.encode())
    clip.stdin.close()
    time.sleep(0.15)
    key("ctrl+v")
    time.sleep(0.25)


def check(label):
    checks.append(label)
    print("PASS", label, flush=True)


try:
    proc, wid = start(False)
    wait_for(lambda: "- [ ] Yesterday 🔁 daily" in list_file.read_text(), "Daily rollover")
    baseline = list_file.read_text()
    check("Daily task reopens after an overnight restart")
    click(wid, 130, 75)
    key("End")
    type_text(" extra words")
    expected = baseline.replace("Daily task==", "Daily task extra words==")
    saved(list_file, expected)
    key("ctrl+z")
    saved(list_file, baseline)
    key("ctrl+shift+z")
    saved(list_file, expected)
    key("ctrl+z")
    saved(list_file, baseline)
    check("Pin groups typing by pause and supports undo and redo")
    key("ctrl+Return")
    wait_for(lambda: f"🔁 daily ✅ {day}" in list_file.read_text(), "Daily completion date")
    text = list_file.read_text()
    assert text.index("Old finished") < text.index("Daily task") < text.index("Child task")
    key("ctrl+z")
    saved(list_file, baseline)
    check("Completion moves the whole branch and undo restores order and status")
    key("ctrl+t")
    type_text("New child")
    wait_for(lambda: "New child" in list_file.read_text(), "New subtask")
    assert list_file.read_text().index("New child") < list_file.read_text().index("Child task")
    key("ctrl+z")
    key("ctrl+z")
    saved(list_file, baseline)
    check("New subtasks go first and both text and structural changes undo")
    key("ctrl+n")
    key("BackSpace")
    saved(list_file, baseline)
    check("Backspace removes an empty task without a crash")
    click(wid, 130, 75)
    key("ctrl+d")
    activate("Repeat daily")
    wait_for(lambda: "Daily task== 📅" in list_file.read_text() and list_file.read_text().count("🔁 daily") == 1,
             "Daily option did not change")
    key("ctrl+z")
    saved(list_file, baseline)
    check("Daily recurrence is selectable in the task options and undoable")
    click(wid, 130, 75)
    key("End")
    paste("\nPlain pasted text")
    wait_for(lambda: "Daily task Plain pasted text" in list_file.read_text(), "Plain text task paste")
    key("ctrl+z")
    saved(list_file, baseline)
    check("Task paste preserves a single Markdown line")
    x("windowsize", wid, 350, 420)
    time.sleep(0.4)
    shot("pin-narrow")
    before = geometry(wid)
    close(proc)
    proc, wid = start(False)
    after = geometry(wid)
    assert before["WIDTH"] == after["WIDTH"] and before["HEIGHT"] == after["HEIGHT"], (before, after)
    check("Pin restores its resized window exactly")
    close(proc)

    proc, wid = start(True)
    click(wid, 140, 100)
    key("ctrl+End")
    type_text("A scratch thought.")
    saved(note_file, initial_note + "A scratch thought.")
    key("ctrl+z")
    saved(note_file, initial_note)
    key("ctrl+y")
    saved(note_file, initial_note + "A scratch thought.")
    key("ctrl+z")
    saved(note_file, initial_note)
    check("Notes groups a typed phrase into one undo step and supports redo")
    key("ctrl+End")
    paste("سلام\nSecond line")
    saved(note_file, initial_note + "سلام\nSecond line")
    key("ctrl+z")
    saved(note_file, initial_note)
    check("Notes pastes Unicode and multiple lines as plain text")
    time.sleep(0.7)
    key("ctrl+a")
    key("BackSpace")
    saved(note_file, "")
    key("ctrl+z")
    saved(note_file, initial_note)
    check("Deleting all notes is undoable")
    key("ctrl+End")
    type_text("Unsaved local text")
    note_file.write_text("Changed externally\n")
    wait_for(lambda: seen("Use disk"), "Conflict prompt missing")
    activate("Keep mine")
    saved(note_file, initial_note + "Unsaved local text")
    note_file.write_text("Disk version\n")
    wait_for(lambda: seen("Use disk"), "Second conflict prompt missing")
    activate("Use disk")
    key("ctrl+End")
    type_text("Loaded")
    saved(note_file, "Disk version\nLoaded")
    check("External edits preserve local text and both conflict choices work")
    x("windowsize", wid, 420, 300)
    time.sleep(0.5)
    shot("notes-light")
    subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", "color-scheme", "prefer-dark"], env=env, check=True)
    time.sleep(0.6)
    shot("notes-dark")
    light = ImageGrab.grab(xdisplay=env["DISPLAY"])
    subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", "color-scheme", "default"], env=env, check=True)
    time.sleep(0.6)
    assert ImageGrab.grab(xdisplay=env["DISPLAY"]).tobytes() != light.tobytes(), "Live theme did not change"
    check("Live desktop theme changes reach the actual window")
    before = geometry(wid)
    close(proc)
    proc, wid = start(True)
    after = geometry(wid)
    assert before["WIDTH"] == after["WIDTH"] and before["HEIGHT"] == after["HEIGHT"], (before, after)
    saved(note_file, "Disk version\nLoaded")
    check("Notes retains text and window size after closing and reopening")
    usage = {}
    for line in Path(f"/proc/{proc.pid}/smaps_rollup").read_text().splitlines():
        if line.startswith(("Rss:", "Pss:")):
            k, v = line.split(":", 1)
            usage[k] = v.strip()
    close(proc)
    result = {"checks": checks, "memory": usage, "binary_bytes": Path(binary).stat().st_size}
    (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)
except BaseException:
    shot("failure")
    raise
finally:
    for child in reversed(children):
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=4)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
