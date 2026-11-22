"""Exercise all installed app entry points in one isolated headless Sway session."""
from contextlib import ExitStack
import json
import os
from pathlib import Path
import subprocess
import time
from gi.repository import Gio, GLib


def check(stack, root, runtime, launch, until, key, compare):
    refs = root / "tests/wayland/appearance"
    bindir = Path(os.environ.get("MUNDANE_BIN_DIR", root / "target/debug")).resolve()
    bus = Gio.bus_get_sync(Gio.BusType.SESSION)
    env = dict(os.environ, DBUS_SYSTEM_BUS_ADDRESS=os.environ["DBUS_SESSION_BUS_ADDRESS"])
    launch(stack, "python3", str(root / "tests/providers/server.py"), str(runtime), log="appearance-providers.log")
    address = until(lambda: (runtime / "address").exists() and (runtime / "address").read_text(), "loopback providers")
    env.update(MUNDANE_WEATHER_ENDPOINT=address + "/forecast", MUNDANE_MAPS_TILEJSON=address + "/tilejson",
               MUNDANE_MAPS_SEARCH=address + "/search")

    def start_portal():
        ready = runtime / "settings-ready"
        ready.unlink(missing_ok=True)
        proc = launch(stack, "python3", str(refs / "portal.py"), log="appearance-portal.log")
        until(ready.exists, "Settings portal name")
        return proc

    def set_value(key, value, signature="u"):
        bus.call_sync("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop",
                      "org.joelinux.TestSettings", "Set", GLib.Variant("(sv)", (key, GLib.Variant(signature, value))),
                      None, Gio.DBusCallFlags.NONE, 3000, None)

    def tree_window(pid, title=None):
        def find(node):
            if node.get("pid") == pid and (title is None or title == node.get("name")):
                return node["id"]
            return next((found for n in node["nodes"] + node["floating_nodes"] if (found := find(n))), None)
        return find(json.loads(subprocess.check_output(["swaymsg", "-rt", "get_tree"])))

    def app(scope, name, *args):
        # Install a fresh virtual keyboard/keymap before each native client maps.
        seat_log = f"appearance-seat-{name}.log"
        launch(scope, "wtype", "-k", "Shift_L", "-s", "300000", log=seat_log,
               env={**env, "WAYLAND_DEBUG": "client"})
        until(lambda: ".key(" in (root / "target/wayland" / seat_log).read_text(), "private keyboard keymap")
        path = bindir / ("examples/motion" if name == "motion" else "mundane-" + name)
        proc = launch(scope, str(path), *args, env=env, log=f"appearance-{name}.log")
        def mapped():
            if proc.poll() is not None:
                raise RuntimeError(f"{name} exited before mapping. See appearance-{name}.log")
            return tree_window(proc.pid)
        until(mapped, f"mapped {name}")
        return proc

    def frame():
        ppm = subprocess.check_output(["grim", "-t", "ppm", "-o", "HEADLESS-1", "-"])
        magic, size, depth, pixels = ppm.split(b"\n", 3)
        assert (magic, size, depth) == (b"P6", b"800 500", b"255")
        return pixels

    def header(color):
        # Inside the shared header panel, clear of its controls and heading.
        offset = 3 * (3 * 800 + 3)
        until(lambda: frame()[offset:offset + 3] == bytes.fromhex(color), f"header #{color}")

    def map_ground(color):
        # A run of map ground is independent of the application's chrome.
        until(lambda: bytes.fromhex(color) * 80 in frame(), f"rendered vector ground #{color}")

    def reference(name):
        compare(name + ".png", 1, refs)

    light, dark = "e9eeea", "222e2a"
    document = Path.home() / "Coastal notes.txt"
    document.write_text("Coastal survey\n\nNorth quay   08:30\nWind from the east\n")
    with ExitStack() as window:
        editor = app(window, "text-editor", str(document))
        initial = tree_window(editor.pid)
        header(dark)  # Missing portal at startup.
        until(lambda: tree_window(editor.pid, "Coastal notes.txt — Text Editor"), "document ready")
        key("a", "ctrl")
        portal = start_portal()
        header(light)  # Initial snapshot when a portal first takes the name.
        set_value("color-scheme", 1)
        header(dark)
        reference("dark")
        set_value("color-scheme", 2)
        header(light)
        reference("light")
        set_value("contrast", 1)
        header("ffffff")
        reference("light-high-contrast")
        set_value("color-scheme", 1)
        header("000000")
        reference("high-contrast")
        set_value("color-scheme", 2)
        set_value("contrast", 0)
        set_value("accent-color", (0.6, 0.2, 0.8), "(ddd)")
        header(light)
        until(lambda: bytes.fromhex("9933cc") * 10 in frame(), "custom accent on focused text field")
        reference("custom-accent")
        portal.terminate()
        portal.wait(timeout=5)
        header(dark)
        start_portal()
        header(light)
        assert tree_window(editor.pid) == initial

    with ExitStack() as window:
        app(window, "motion")
        first = frame()
        until(lambda: frame() != first, "animated marker with optional key absent")
        set_value("reduced-motion", 1)
        reference("reduced-motion")
        still = frame()
        for delay in (0.19, 0.32, 0.53):
            time.sleep(delay)
            assert frame() == still, "decorative animation continued"
        set_value("reduced-motion", 0)
        first = frame()
        until(lambda: frame() != first, "animation resumed")
        first = frame()
        until(lambda: frame() != first, "marker continued moving")

    for manifest in sorted((root / "apps").glob("*/Cargo.toml")):
        name = manifest.parent.name
        if name == "text-editor":
            continue
        set_value("color-scheme", 2)
        if name == "calculator":
            (runtime / "change-during-read").touch()
        with ExitStack() as window:
            proc = app(window, name)
            initial = tree_window(proc.pid)
            if name == "calculator":
                header("ffffff")  # The queued signal supersedes the in-flight snapshot.
                set_value("contrast", 0)
            header(light)
            if name == "maps":
                map_ground("e9eee5")
            set_value("color-scheme", 1)
            header(dark)
            if name == "maps":
                map_ground("383e36")
            set_value("contrast", 1)
            header("000000")
            if name == "maps":
                map_ground("101010")
            set_value("contrast", 0)
            header(dark)
            assert tree_window(proc.pid) == initial
        print(f"Passed live appearance in {name}", flush=True)
    print("Passed 18 live app windows, portal disappearance and restart, and reduced motion")
