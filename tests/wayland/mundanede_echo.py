"""Drive a physical key through headless Sway into mundaneDE's winit seat."""
import json
import os
from pathlib import Path
import shlex
import subprocess
import time


def check(checkout, stack, launch, until, root, runtime, output, transcript):
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi, GLib
    Atspi.init()
    host = runtime / os.environ["WAYLAND_DISPLAY"]
    binary = checkout / "target/debug/mundane-de"
    panel = runtime / "panel"
    panel.write_text("#!/bin/sh\nexec env LD_PRELOAD=libgtk4-layer-shell.so.0 python3 " +
                     shlex.quote(str(checkout / "session/shell/panel.py")) + "\n")
    panel.chmod(0o700)
    env = dict(os.environ, MUNDANE_PANEL=str(panel), GSETTINGS_BACKEND="keyfile",
               XDG_CURRENT_DESKTOP="mundane", RUST_LOG="info",
               DBUS_SYSTEM_BUS_ADDRESS=os.environ["DBUS_SESSION_BUS_ADDRESS"],
               PATH=str(binary.parent) + os.pathsep + os.environ["PATH"],
               PYTHONPATH=str(Path(__file__).parent) + os.pathsep + os.environ.get("PYTHONPATH", ""),
               SPEECHD_ADDRESS="unix_socket:" + str(runtime / "no-speechd"),
               PULSE_SERVER="unix:" + str(runtime / "no-audio"))
    subprocess.run(["gsettings", "set", "org.gnome.desktop.a11y.applications",
                    "screen-reader-enabled", "true"], env=env, check=True)
    launch(stack, binary, "--winit", log="mundane-de.log", env=env)
    inner = until(lambda: next((p for p in runtime.glob("wayland-*") if p != host and p.is_socket()), None),
                  "mundaneDE socket")
    until(lambda: "Screen reader on" in transcript.read_text(), "session-owned Orca")
    terminal = os.environ.get("MUNDANE_TERMINAL_BIN", root / "target/debug/mundane-terminal")
    app = launch(stack, terminal, log="terminal.log", env=dict(env, WAYLAND_DISPLAY=inner.name, WAYLAND_DEBUG="client"))
    until(lambda: (Path.home() / ".prompt-ready").exists(), "nested PTY prompt")
    keymap = runtime / "us.xkb"
    keymap.write_bytes(subprocess.check_output(["xkbcli", "compile-keymap", "--layout", "us"]) + b"\0")
    def key(*codes):
        subprocess.run([binary.parent / "examples/nested-input", host, keymap, *map(str, codes)], check=True)
    def walk(node):
        yield node
        for i in range(node.get_child_count()):
            if child := node.get_child_at_index(i):
                yield from walk(child)
    def field():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)
        desktop = Atspi.get_desktop(0)
        for i in range(desktop.get_child_count()):
            app_node = desktop.get_child_at_index(i)
            if app_node and app_node.get_process_id() == app.pid:
                return next((n for n in walk(app_node) if n.get_name() == "Find text"), None)
    until(lambda: any(json.loads(line)["text"].strip() == "$" for line in transcript.read_text().splitlines()), "Orca focused on the terminal")
    key(29, 42, 33, -33, -42, -29)  # Ctrl+Shift+F
    entry = until(field, "Terminal Find")
    key(42, 20, -20, -42, 24, -24, 37, -37, 21, -21, 24, -24)
    until(lambda: Atspi.Text.get_text(entry, 0, -1) == "Tokyo", "initial Find text")
    inserts = []
    def event(e):
        if e.source.get_process_id() == app.pid and e.source.get_name() == "Find text":
            inserts.append((e.detail1, e.detail2, str(e.any_data)))
    listener = Atspi.EventListener.new(event)
    assert listener.register("object:text-changed:insert")
    start = time.monotonic()
    key(44, -44)
    def echoed():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)
        return any(s["text"] == "z" and s["time"] >= start
                   for s in map(json.loads, transcript.read_text().splitlines()))
    until(echoed, "Orca speech-server request for z")
    assert Atspi.Text.get_text(entry, 0, -1) == "Tokyoz"
    assert Atspi.Text.get_caret_offset(entry) == 6 and (5, 1, "z") in inserts, inserts
    print("mundaneDE: Terminal Find contains Tokyoz, insert (5, 1, z), caret 6, Orca said z.")
