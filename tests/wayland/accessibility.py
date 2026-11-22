"""Disposable GTK control and AT-SPI inspector, on the test session's buses."""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

import gi
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib

if sys.argv[1] == "--control":
    gi.require_version("Gtk", "3.0")
    from gi.repository import Gtk
    GLib.set_prgname("mundane-probe-control")
    app = Gtk.Window(title="AT-SPI control")
    app.set_accept_focus(False)
    app.add(Gtk.Button(label="Probe button"))
    app.show_all()
    Gtk.main()
    sys.exit()

Atspi.init()
desktop = Atspi.get_desktop(0)
ctx = GLib.MainContext.default()


def drain(seconds=0.1):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        while ctx.pending():
            ctx.iteration(False)
        time.sleep(0.005)


def wait(fn, description):
    for _ in range(80):
        result = fn()
        if result:
            return result
        drain()
    raise AssertionError(description)


def children(node):
    return [x for i in range(node.get_child_count()) if (x := node.get_child_at_index(i))]


def walk(node):
    yield node
    for child in children(node):
        yield from walk(child)


def named(root, name):
    return next((n for n in walk(root) if n.get_name() == name), None)


def gtk():
    return next((n for n in children(desktop) if n.get_name() == "mundane-probe-control"), None)


if sys.argv[1] == "--observe":
    # Keep an AT client registered across the shorter inspector and Orca commands.
    stream = Path(os.environ["EVENT_LOG"]).open("w", buffering=1)
    def observed(event):
        try:
            print(json.dumps([time.monotonic(), event.type, event.source.get_name(),
                              event.detail1, event.detail2, str(event.any_data)], ensure_ascii=False), file=stream)
        except GLib.Error:
            pass
    listener = Atspi.EventListener.new(observed)
    for signal in ("object:state-changed:focused", "object:text-changed", "object:text-caret-moved",
                   "object:text-selection-changed", "object:children-changed", "window"):
        assert listener.register(signal)
    assert named(wait(gtk, "GTK registration"), "Probe button")
    Path(os.environ["XDG_RUNTIME_DIR"], "observer-ready").touch()
    Atspi.event_main()
    sys.exit()

pid, suite = int(sys.argv[1]), sys.argv[2]
assert named(wait(gtk, "GTK control"), "Probe button")
def app_root():
    return next((n for n in children(desktop) if n.get_process_id() == pid), None)


def status(enabled):
    subprocess.run(["gdbus", "call", "--session", "--dest", "org.a11y.Bus", "--object-path", "/org/a11y/bus", "--method",
                    "org.freedesktop.DBus.Properties.Set", "org.a11y.Status", "IsEnabled",
                    "<true>" if enabled else "<false>"], check=True)


if suite == "Fields":
    assert app_root() is None, "adapter active before enable"
    status(True)
    def windows():
        root = app_root()
        if root and {n.get_name() for n in children(root)} == {"Document fixture", "Password fixture"}:
            return root
        return None
    try:
        root = wait(windows, "late activation of both windows")
    except AssertionError:
        if (root := app_root()):
            print([(n.get_role_name(), n.get_name()) for n in walk(root)], flush=True)
        raise
    assert {n.get_name() for n in children(root)} == {"Document fixture", "Password fixture"}
    def snapshot(root):
        return [(n.get_name(), n.get_role_name(), n.get_interfaces()) for n in walk(root)]
    first = snapshot(root)
    status(False)
    wait(lambda: app_root() is None, "deactivation")
    status(True)
    root = wait(windows, "re-registration")
    assert sorted(snapshot(root)) == sorted(first)
    field = named(root, "Passphrase")
    assert Atspi.Text.get_text(field, 0, -1) == "" and field.get_caret_offset() == 0
    assert field.get_role() == Atspi.Role.PASSWORD_TEXT
    secret = "së😀"
    subprocess.run(["swaymsg", "-q", '[title="Password fixture"] focus'], check=True)
    subprocess.run(["wtype", "-s", "150", secret, "-s", "100"], check=True)
    wait(lambda: Atspi.Text.get_text(field, 0, -1) == "•••", "masked field")
    assert field.get_caret_offset() == 3
    subprocess.run(["wtype", "-k", "BackSpace", "-s", "150"], check=True)
    wait(lambda: Atspi.Text.get_text(field, 0, -1) == "••", "masked deletion")
    result = Path(os.environ["XDG_RUNTIME_DIR"]) / "accepted"
    disabled = named(named(root, "Password fixture"), "Close fixture")
    disabled.clear_cache()
    reported_sensitive = disabled.get_state_set().contains(Atspi.StateType.SENSITIVE)
    try:
        disabled.do_action(0)
    except GLib.Error:
        pass
    drain()
    assert not result.exists()
    doc = named(root, "Notes")
    assert Atspi.Text.get_text(doc, 0, -1) == "e\u0301😀\r\nline two"
    assert doc.get_character_count() == 13
    subprocess.run(["swaymsg", "-q", '[title="Document fixture"] focus'], check=True)
    assert doc.set_caret_offset(3)
    wait(lambda: doc.get_caret_offset() == 3, "non-BMP caret")
    subprocess.run(["wtype", "-M", "shift", "-k", "Left", "-m", "shift"], check=True)
    wait(lambda: doc.get_caret_offset() == 2, "reversed selection")
    selection = Atspi.Text.get_selection(doc, 0)
    assert (selection.start_offset, selection.end_offset) == (2, 3)
    frame = named(root, "Document fixture")
    close = named(frame, "Close fixture")
    assert close.do_action(0)
    try:
        close.do_action(0)  # race a second request against window teardown
    except GLib.Error:
        pass
    wait(lambda: not named(root, "Document fixture"), "frame removal")
    assert result.read_text() == "close\n" and named(root, "Password fixture")
    events = [json.loads(line) for line in Path(os.environ["EVENT_LOG"]).read_text().splitlines()]
    mask_events = [e for e in events if e[2] == "Passphrase" and e[1].startswith("object:text-changed")]
    assert mask_events and all(set(e[5]) == {"•"} for e in mask_events), mask_events
    assert any(e[1].endswith(":delete") and e[3:] == [2, 1, "•"] for e in mask_events), mask_events
    assert not reported_sensitive, "AccessKit reported the disabled push button as SENSITIVE"
    print("Passed two-window lifecycle, password, Unicode multiline selection and action teardown")
    sys.exit()
root = wait(lambda: next((n for n in children(desktop) if n.get_process_id() == pid), None), "GPUI registration")
if suite == "MissingVideoDecoder":
    error = wait(lambda: next((n for n in walk(root) if "Missing decoder vp8dec" in n.get_name()), None),
                 "missing VP8 decoder reported to user")
    assert error.get_role() == Atspi.Role.STATUS_BAR and "video/x-vp8" in error.get_name()
    sys.exit()
if suite == "Processes":
    listbox = named(root, "Processes")
    row = named(listbox, "file-indexer (PID 1978)")
    assert row.get_state_set().contains(Atspi.StateType.SELECTED)
    assert row.get_description() and "128" in row.get_description()
    before = [n.get_name() for n in children(listbox)]
    assert row.get_attributes()["posinset"] == "2" and row.get_attributes()["setsize"] == "8"
    assert named(root, "Process").do_action(0)
    def reordered():
        listbox.clear_cache()
        return [n.get_name() for n in children(listbox)] != before
    wait(reordered, "row reorder")
    assert named(listbox, row.get_name()) == row
    subprocess.run(["swaymsg", "-q", "output HEADLESS-1 mode 720x240"], check=True)
    subprocess.run(["wtype", "-s", "150", "-k", "End", "-s", "100"], check=True)
    def selected_last():
        listbox.clear_cache()
        for item in children(listbox):
            item.clear_cache()
            if item.get_attributes().get("posinset") == "8" and item.get_state_set().contains(Atspi.StateType.SELECTED):
                return item
        return None
    last = wait(selected_last, "selection and scroll across viewport boundary")
    rect = Atspi.Component.get_extents(listbox, Atspi.CoordType.WINDOW)
    cell = Atspi.Component.get_extents(last, Atspi.CoordType.WINDOW)
    assert rect.y <= cell.y < rect.y + rect.height and cell.y+cell.height <= rect.y+rect.height+1
    subprocess.run(["swaymsg", "-q", "output HEADLESS-1 mode 720x440"], check=True)
    subprocess.run(["wtype", "-s", "150", "-k", "Home", "-s", "100"], check=True)
    wait(lambda: named(listbox, "file-indexer (PID 1978)"), "rematerialized first row")
    row = named(listbox, "file-indexer (PID 1978)")
    assert row.do_action(0)
    wait(lambda: row.get_state_set().contains(Atspi.StateType.SELECTED), "restore selected process")
    sys.exit()
tree = [(n.get_role_name(), n.get_name(), n.get_interfaces()) for n in walk(root)]
print(json.dumps(tree, ensure_ascii=False), flush=True)
expected = {"Terminal": ["Find in Terminal", "Find match", "Find text"],
            "Packages": ["Review package changes", "Install chartroom", "Cancel", "Apply changes"],
            "ArchiveManager": ["Archive entries", "Extract selected…", "Extract all…", "2 marked"],
            "DiskUsage": ["Folder contents", "Private records", "Show allocated space", "Rescan"],
            "Fonts": ["Font families", "Edit sample", "Next face", "Previous face", "Preview in DejaVu Sans · Bold", "All sample characters covered."],
            "Characters": ["Copy character", "Name or code point", "Matching characters", "BLACK STAR", "Copied U+2605"],
            "Clocks": ["Saved cities", "UTC", "New York", "Add city", "Timer"],
            "Disks": ["Drives and volumes", "Survey", "Unmount", "Refresh"],
            "Music": ["Harbour Lights", "Play", "Playback position", "Add tracks…", "Next track"],
            "Video": ["Coastal passage.webm", "Play", "Playback position", "Playback status"],
            "DocumentViewer": ["Page 2 of 2", "Page number", "Find text", "Document status"],
            "Calculator": ["Expression", "Result", "History", "Use result", "Clear history"],
            "ImageViewer": ["North Point.jpg", "Open", "Zoom out", "Zoom in", "Fit", "Image status"],
            "Files": ["Open folder", "Cancel", "Open"],
            "TextEditor": ["Document status", "Find", "Save"],
            "SystemMonitor": ["End process?", "Cancel", "End process"]}
assert all(named(root, name) for name in expected[suite]), tree
if suite in ("Files", "Terminal", "SystemMonitor", "Packages"):
    dialog = next(n for n in walk(root) if n.get_role() == Atspi.Role.DIALOG)
    assert dialog.get_state_set().contains(Atspi.StateType.MODAL)
    assert dialog.get_description()
    assert all(n.get_role() != Atspi.Role.TITLE_BAR for n in walk(root))
if suite == "Packages":
    assert named(root, "Cancel").get_state_set().contains(Atspi.StateType.FOCUSED)
    assert named(root, "Apply changes").get_role() == Atspi.Role.PUSH_BUTTON
    assert named(root, "Install libtide · 1.4 · Dependency")
    transition = "Upgrade coast-data · 3.0 → 3.1 · Dependency"
    row = named(root, transition)
    assert row and Atspi.Text.get_text(row, 0, -1) == transition
    assert "Cached repository index" in dialog.get_description()
if suite == "ArchiveManager":
    row = named(root, "tides.csv")
    assert row.get_state_set().contains(Atspi.StateType.SELECTED)
    assert "Marked" in row.get_description()
if suite == "DiskUsage":
    row = named(root, "Private records")
    assert row.get_state_set().contains(Atspi.StateType.SELECTED)
    assert "permission denied" in row.get_description()
    assert "512.0 KiB" in named(root, "Charts").get_description()
    assert any("Incomplete scan" in n.get_name() for n in walk(root))
if suite == "Fonts":
    row = named(named(root, "Font families"), "DejaVu Sans")
    assert row.get_state_set().contains(Atspi.StateType.SELECTED)
    assert named(root, "Preview in DejaVu Sans · Bold").get_description() == "Coast · Αθήνα · שלום"
    sample = next(n for n in walk(root) if n.get_name() == "Sample text" and n.get_role() == Atspi.Role.ENTRY)
    assert Atspi.Text.get_text(sample, 0, -1) == "Coast · Αθήνα · שלום"
    subprocess.run(["wtype", "-M", "ctrl", "-k", "e", "-m", "ctrl", "-s", "100", "字", "-k", "Return"], check=True)
    wait(lambda: named(root, "Missing glyphs · U+5B57"), "missing CJK glyph in DejaVu")
    subprocess.run(["wtype", "-M", "ctrl", "-k", "f", "-m", "ctrl", "-s", "100", "Noto Sans CJK SC", "-k", "Return"], check=True)
    wait(lambda: named(root, "Preview in Noto Sans CJK SC · Regular"), "CJK collection face preview")
    assert named(root, "All sample characters covered.")
if suite == "Characters":
    row = named(named(root, "Matching characters"), "BLACK STAR")
    assert row.get_state_set().contains(Atspi.StateType.SELECTED)
    assert "U+2605" in row.get_description()
    assert named(root, "Copy character").get_action_name(0) == "click"
if suite == "Clocks":
    assert "03:00" in named(root, "New York").get_description()
    assert "07:00" in named(root, "UTC").get_description()
if suite == "Disks":
    row = named(root, "Survey")
    assert row.get_state_set().contains(Atspi.StateType.SELECTED)
    assert "Mounted at /run/media/joe/Survey" in row.get_description()
    assert "Action" in named(root, "Unmount").get_interfaces()
    assert named(root, "Coast Field Recorder") and named(root, "Transfer")
if suite == "Music":
    queue = next(n for n in walk(root) if n.get_role() == Atspi.Role.LIST_BOX)
    rows = children(queue)
    order = ["Ebb Tide", "Harbour Lights", "North Channel", "Field recording.wav"]
    for row in rows:
        pos = int(row.get_attributes()["posinset"])
        assert row.get_attributes()["setsize"] == "4" and row.get_name() == order[pos-1]
    assert rows[1].get_state_set().contains(Atspi.StateType.SELECTED)
    assert "Current track" in rows[1].get_description() and "Soundings" in rows[1].get_description()
    assert "Paused" in named(root, "Playback status").get_description()
    slider = named(root, "Playback position")
    assert abs(slider.get_current_value() - 5) < 0.01
    assert named(root, "Move down").do_action(0)
    def reordered():
        queue.clear_cache()
        return any(n.get_name() == "Harbour Lights" and n.get_attributes()["posinset"] == "3"
                   for n in children(queue))
    wait(reordered, "accessible reorder")
    assert named(root, "Move up").do_action(0)
    wait(lambda: not reordered(), "restored queue order")
if suite == "Video":
    screen = named(root, "Coastal passage.webm")
    assert screen.get_state_set().contains(Atspi.StateType.FOCUSED)
    slider = named(root, "Playback position")
    assert slider.get_role() == Atspi.Role.SLIDER and "Value" in slider.get_interfaces()
    assert abs(slider.get_current_value() - 5) < 0.01
    assert slider.get_minimum_value() == 0 and 11.9 <= slider.get_maximum_value() < 12.1
    assert slider.set_current_value(7)
    wait(lambda: abs(slider.get_current_value() - 7) < 0.01, "seek to seven seconds")
    assert named(root, "Back 5 seconds").do_action(0)
    wait(lambda: abs(slider.get_current_value() - 2) < 0.01, "accessible backward step")
    assert named(root, "Play").do_action(0)
    wait(lambda: named(root, "Pause"), "resume by AT-SPI")
    assert named(root, "Pause").do_action(0)
    wait(lambda: named(root, "Play"), "pause by AT-SPI")
if suite == "DocumentViewer":
    field = named(root, "Find text")
    assert field.get_state_set().contains(Atspi.StateType.FOCUSED)
    assert Atspi.Text.get_text(field, 0, -1) == "high water"
    assert Atspi.Text.get_text(named(root, "Page number"), 0, -1) == "2"
    def page_text():
        page = next(n for n in walk(root) if n.get_role() == Atspi.Role.DOCUMENT_FRAME)
        return Atspi.Text.get_text(page, 0, -1)
    assert "The east channel reaches high water at noon." in page_text()
    def status_is(suffix):
        node = named(root, "Document status")
        node.clear_cache()
        return node.get_description().endswith(suffix)
    assert status_is("Match 1 of 1 · 125%")
    for label, suffix in [("Zoom out", "Match 1 of 1 · 100%"), ("Done", "Page 2 of 2 · 100%"),
                          ("Previous page", "Page 1 of 2 · 100%")]:
        button = named(root, label)
        assert button.get_role() == Atspi.Role.PUSH_BUTTON and button.do_action(0)
        wait(lambda: status_is(suffix), label)
    assert "Survey overview" in page_text()
if suite == "Calculator":
    field = named(root, "Expression")
    assert field.get_state_set().contains(Atspi.StateType.FOCUSED)
    assert Atspi.Text.get_text(field, 0, -1) == "23.25 × 4"
    assert named(root, "Result").get_description() == "93"
    def click(name):
        button = named(root, name)
        assert button and button.get_role() == Atspi.Role.PUSH_BUTTON
        assert button.get_action_name(0) == "click" and button.do_action(0)
    click("Reuse 18.75 + 4.50 = 23.25")
    wait(lambda: Atspi.Text.get_text(field, 0, -1) == "23.25", "reuse history entry")
    for label in ("Add", "2", "Equals"):
        click(label)
    wait(lambda: named(root, "Result").get_description() == "25.25", "keypad calculation")
    click("Clear history")
    wait(lambda: not any(n.get_name().startswith("Reuse ") for n in walk(root)), "history cleared")
    assert named(root, "Result").get_description() == "25.25"
if suite == "ImageViewer":
    picture = named(root, "North Point.jpg")
    assert picture.get_role() == Atspi.Role.IMAGE
    assert picture.get_description() == "480 × 300 pixels"
    assert picture.get_state_set().contains(Atspi.StateType.FOCUSED)
    for label in ("Open", "Zoom out", "Zoom in", "Fit"):
        button = named(root, label)
        assert button.get_role() == Atspi.Role.PUSH_BUTTON and button.get_action_name(0) == "click"
    for label, expected_zoom in [("Zoom in", "125% of fit"), ("Zoom out", "Fit to window"),
                                 ("Zoom out", "75% of fit"), ("Fit", "Fit to window")]:
        assert named(root, label).do_action(0)
        def zoom_status():
            node = named(root, "Image status")
            node.clear_cache()
            return node.get_description() == "480 × 300  ·  " + expected_zoom
        wait(zoom_status, label + " accessibility action")
if suite == "TextEditor":
    body = named(root, "Document text")
    assert "Inspect the tide pools." in Atspi.Text.get_text(body, 0, -1)
    assert Atspi.Text.get_text(named(root, "Find in document"), 0, -1) == "tide pools"
if suite == "Terminal":
    field = named(root, "Find text")
    assert "Text" in field.get_interfaces()
    assert Atspi.Text.get_text(field, 0, -1) == "Tokyo"
    assert field.get_caret_offset() == 5
    component = Atspi.Component
    frame = children(root)[0]
    dialog = named(root, "Find in Terminal")
    win = component.get_extents(field, Atspi.CoordType.WINDOW)
    parent = component.get_extents(field, Atspi.CoordType.PARENT)
    origin = component.get_extents(dialog, Atspi.CoordType.WINDOW)
    assert (win.x, win.y) == (parent.x+origin.x, parent.y+origin.y)
    assert win.width > 0 and win.height > 0
    hit = component.get_accessible_at_point(frame, win.x+win.width//2, win.y+win.height//2, Atspi.CoordType.WINDOW)
    assert hit and hit.get_name() == "Find text", (hit, win)
    for offset in range(5):
        box = Atspi.Text.get_character_extents(field, offset, Atspi.CoordType.WINDOW)
        assert win.x <= box.x < win.x+win.width and win.y <= box.y < win.y+win.height
        assert box.width > 0 and box.height > 0
    field.clear_cache()
    assert field.get_state_set().contains(Atspi.StateType.FOCUSED)
    start_typing = time.monotonic()
    subprocess.run(["wtype", "-k", "z", "-s", "400"], check=True)
    wait(lambda: Atspi.Text.get_text(field, 0, -1) == "Tokyoz", "typed character")
    subprocess.run(["wtype", "-k", "BackSpace", "-s", "200"], check=True)
    assert field.add_selection(1, 4)
    wait(lambda: (lambda r: (r.start_offset, r.end_offset) == (1, 4))(Atspi.Text.get_selection(field, 0)), "remote text selection")
    subprocess.run(["wtype", "😀"], check=True)
    wait(lambda: Atspi.Text.get_text(field, 0,-1) == "T😀o", "replacement of remote selection")
    assert field.get_caret_offset() == 2
    subprocess.run(["wtype", "-k", "BackSpace"], check=True)
    wait(lambda: Atspi.Text.get_text(field, 0,-1) == "To", "delete non-BMP character")
    assert field.get_caret_offset() == 1
    heading = named(root, "Find in terminal")
    assert any("Text" in n.get_interfaces() and Atspi.Text.get_text(n, 0, -1) == "Find in terminal" for n in walk(heading))
    subprocess.run(["wtype", "-M", "ctrl", "-k", "a", "-m", "ctrl", "Software rendering"], check=True)
    wait(lambda: Atspi.Text.get_text(field, 0, -1) == "Software rendering", "known match")
    def orca(module, method):
        args = ["gdbus", "call", "--session", "--dest", "org.gnome.Orca1.Service",
                "--object-path", "/org/gnome/Orca1/Service/"+module,
                "--method", f"org.gnome.Orca1.{module}.{method}", "true"]
        assert "true" in subprocess.check_output(args, text=True)
        drain(0.2)
    orca("ObjectNavigator", "MoveToPreviousSibling")
    button = named(root, "Find match")
    assert button.get_action_name(0) == "click" and button.do_action(0)
    wait(lambda: not named(root, "Find in Terminal"), "removal after AT-SPI click")
    shell = wait(lambda: named(root, "Shell"), "restored Shell context")
    shell.clear_cache()
    assert shell.get_state_set().contains(Atspi.StateType.FOCUSED)
    drain(0.6)
    orca("WhereAmIPresenter", "WhereAmIBasic")
    events = [json.loads(line) for line in Path(os.environ["EVENT_LOG"]).read_text().splitlines()]
    field_events = [e for e in events if e[2] == "Find text" and e[0] >= start_typing]
    for kind, start, length, data in [("insert", 5, 1, "z"), ("delete", 5, 1, "z"),
                                      ("insert", 1, 1, "😀"), ("delete", 1, 1, "😀")]:
        assert any(e[1] == "object:text-changed:"+kind and e[3:] == [start,length,data] for e in field_events), field_events
    assert any(e[1] == "object:text-selection-changed" for e in field_events)
    assert any(e[1] == "object:text-caret-moved" and e[3] == 1 for e in field_events)
    spoken = [json.loads(line) for line in Path(os.environ["ORCA_TRANSCRIPT"]).read_text().splitlines()]
    phrases = [s["text"].strip() for s in spoken]
    for phrase in ["Find text entry Tokyo", "Find match button", "Find in terminal heading", "Shell terminal $"]:
        assert phrase in phrases, phrases
    assert any(s["text"] == "z" and s["time"] >= start_typing for s in spoken), "Orca did not echo z typed into Find text"
