"""Real component sheets on an isolated labwc output and Settings portal."""
import os
from pathlib import Path
import subprocess
from gi.repository import Gio, GLib


def check(group, stack, launch, until, key, compare, root, runtime, output, recording_profile):
    refs = root / "docs/design"
    bus = Gio.bus_get_sync(Gio.BusType.SESSION)
    launch(stack, "python3", str(root / "tests/wayland/appearance/portal.py"), log="design-portal.log")
    until((runtime / "settings-ready").exists, "private Settings portal")
    subprocess.run(["wlr-randr", "--output", "HEADLESS-1", "--custom-mode", "1080x640"], check=True)
    def set_value(k, sig, value):
        bus.call_sync("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop",
                      "org.joelinux.TestSettings", "Set", GLib.Variant("(sv)", (k, GLib.Variant(sig, value))),
                      None, Gio.DBusCallFlags.NONE, 3000, None)
    bin_dir = Path(os.environ.get("MUNDANE_BIN_DIR", root / "target/debug"))
    program = bin_dir / "examples/design_sheet"
    def start(scale):
        p = launch(stack, program, group, log="design-" + group + ".log",
                   env=dict(os.environ, WAYLAND_DEBUG="client", SHEET_TEXT_SCALE=scale))
        until(lambda: "ack_configure" in (output / ("design-" + group + ".log")).read_text(), "mapped reference window")
        return p
    if group != "core":
        from design_contract import enable, verify
        enable(stack,launch,until,runtime)
    transcript = None
    if group == "notifications":
        transcript = recording_profile("notifications")
        env = dict(os.environ, PYTHONPATH=str(root / "tests/wayland") + os.pathsep + os.environ.get("PYTHONPATH", ""),
                   PULSE_SERVER="unix:" + str(runtime / "no-audio"),
                   SPEECHD_ADDRESS="unix_socket:" + str(runtime / "no-speechd"))
        launch(stack, "orca", log="orca-design.log", env=env)
        until(lambda: transcript.stat().st_size, "Orca recording backend")
    app = start("1")
    key("Shift_L")
    variants = [("dark",1,0,False,0),("light",2,0,False,0),
                ("high-contrast",1,1,False,0),("light-high-contrast",2,1,False,0),
                ("accent",2,0,True,0),("reduced-motion",2,0,True,1)]
    for name, scheme, contrast, custom, reduced in variants:
        set_value("color-scheme", "u", scheme)
        set_value("contrast", "u", contrast)
        set_value("reduced-motion", "u", reduced)
        if custom:
            set_value("accent-color", "(ddd)", (0.6, 0.2, 0.8))
        mode = ("Light" if scheme == 2 else "Dark") + (" high contrast" if contrast else "")
        motion = "Reduced motion" if reduced else "Standard motion"
        focus = "9933cc" if custom else { (1,0):"a0dfb3",(2,0):"23623e",(1,1):"ffff00",(2,1):"000099"}[(scheme,contrast)]
        title = f"Design sheet · {mode} · {motion} · {focus}"
        until(lambda: title in subprocess.check_output(["wlrctl", "toplevel", "list"], text=True), title)
        if group == "launcher":
            # Coordinates use the virtual pointer helper's 800 × 500 coordinate space.
            # Hover Files, hold the primary button, then release outside the row.
            import select
            socket = runtime / os.environ["WAYLAND_DISPLAY"]
            with subprocess.Popen([root / "target/debug/examples/header-pointer", socket, "gesture",
                                   "300,245", "wait", "press", "wait", "780,480", "release"],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True) as pointer:
                for state in ("", "-pressed"):
                    assert select.select([pointer.stdout], [], [], 4)[0] and pointer.stdout.readline().strip() == "ready"
                    compare(f"{group}-{name}{state}.png", 1, refs)
                    pointer.stdin.write("continue\n")
                    pointer.stdin.flush()
                assert pointer.wait(timeout=4) == 0
            # Restore the selected keyboard target independently of pointer focus.
            key("Home")
        else:
            compare(f"{group}-{name}.png", 1, refs)
        if group == "settings":
            key("F8")
            compare(f"confirmation-{name}.png", 1, refs)
            key("Escape")
    if group != "core":
        verify(group, app.pid, key, transcript)
    if group == "core":
        app.terminate()
        app.wait(timeout=4)
        start("1.25")
        key("Shift_L")
        compare("core-text-125.png", 1, refs)
