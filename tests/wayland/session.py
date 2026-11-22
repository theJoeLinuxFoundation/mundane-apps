"""Run inside the private session created by tests/wayland/run."""
import argparse
import json
from files_fixture import create as create_files_fixture, png
from contextlib import ExitStack
import os
from pathlib import Path
import subprocess
import time
from xml.sax.saxutils import escape

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
RUNTIME = Path(os.environ["XDG_RUNTIME_DIR"])
OUTPUT = ROOT / "target/wayland"
APP_ID = "org.joelinux.mundane.Terminal"


def run(*args, **kwargs):
    return subprocess.run(args, check=True, timeout=15, text=True, **kwargs)


def output(*args):
    return run(*args, stdout=subprocess.PIPE).stdout.strip()


def until(check, description):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        result = check()
        if result:
            return result
        time.sleep(0.05)
    if "WAYLAND_DISPLAY" in os.environ:
        run("grim", "-o", "HEADLESS-1", str(OUTPUT / "timeout.png"))
    raise RuntimeError(f"Timed out waiting for {description}. See {OUTPUT} for logs.")


def launch(stack, *args, log, env=None):
    stream = stack.enter_context((OUTPUT / log).open("w"))
    child = subprocess.Popen(args, cwd=Path.home(), stdout=stream, stderr=stream, env=env)
    def stop():
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=4)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
    stack.callback(stop)
    return child


def configure_fonts():
    config = RUNTIME / "fonts.conf"
    fonts = RUNTIME / "fonts"
    fonts.mkdir()
    # Use the exact families in the baseline. Extra fonts on the host change fallback.
    for name in ("DejaVuSans.ttf", "DejaVuSans-Bold.ttf", "DejaVuSansMono.ttf",
                 "DejaVuSansMono-Bold.ttf", "NotoSansCJK-Regular.ttc"):
        roots = [Path(os.environ.get("TEST_FONT_DIR", "/usr/share/fonts")), Path("/usr/share/fonts")]
        path = next((p for root in roots for p in root.rglob(name)), None)
        if path is None:
            raise RuntimeError(f"Missing test font {name}")
        (fonts / name).symlink_to(path)
    config.write_text(f"""<fontconfig><dir>{escape(str(fonts))}</dir>
<cachedir>{escape(os.environ['XDG_CACHE_HOME'])}</cachedir>
<alias><family>sans-serif</family><prefer><family>DejaVu Sans</family></prefer></alias>
<alias><family>monospace</family><prefer><family>DejaVu Sans Mono</family></prefer></alias>
</fontconfig>""")
    os.environ["FONTCONFIG_FILE"] = str(config)


def key(name, *modifiers):
    args = [arg for mod in modifiers for arg in ("-M", mod)]
    args += ["-k", name]
    args += [arg for mod in reversed(modifiers) for arg in ("-m", mod)]
    run("wtype", "-s", "150", *args, "-s", "100")


def type_text(text):
    run("wtype", "-s", "150", "-d", "12", text, "-s", "100")


def clipboard():
    result = subprocess.run(["wl-paste", "--no-newline"], text=True, capture_output=True, timeout=2)
    return result.stdout if result.returncode == 0 else ""


def find_and_copy(text):
    key("f", "ctrl", "shift")
    type_text(text)
    key("Return")
    key("c", "ctrl", "shift")
    until(lambda: clipboard() == text, f"selected text {text!r} on the private clipboard")


def recording_profile(suite="terminal"):
    profile = Path(os.environ["XDG_DATA_HOME"]) / "orca"
    profile.mkdir(exist_ok=True)
    (profile / "orca-customizations.py").write_text(
        "from orca import gsettings_registry, speech_manager\n"
        "gsettings_registry.get_registry().set_runtime_value('typing-echo', 'character-echo', True)\n"
        "gsettings_registry.get_registry().set_runtime_value('typing-echo', 'key-echo', True)\n"
        "gsettings_registry.get_registry().set_runtime_value('typing-echo', 'alphabetic-keys', True)\n"
        "speech_manager.SPEECH_FACTORY_MODULES[:] = ['recording_speech']\n"
        "gsettings_registry.get_registry().set_runtime_value('speech', 'speech-server-factory', 'recording_speech')\n")
    transcript = OUTPUT / f"orca-{suite}.jsonl"
    transcript.write_text("")
    os.environ["ORCA_TRANSCRIPT"] = str(transcript)
    return transcript


def inspect_accessibility(stack, app_id, app_pid, suite=None):
    suite = suite or app_id.rsplit(".", 1)[1]
    listing = output("orca", "--list-apps")
    (OUTPUT / f"orca-{suite}.txt").write_text(listing + "\n")
    assert "mundane-probe-control" in listing and str(app_pid) in listing, listing
    run("swaymsg", "-q", f'[app_id="{app_id}"] focus')
    key("Tab")
    key("Tab", "shift")
    inspection = subprocess.run(["python3", str(HERE / "accessibility.py"), str(app_pid), suite],
                                timeout=35, stdout=subprocess.PIPE, text=True)
    (OUTPUT / f"tree-{suite}.json").write_text(inspection.stdout)
    inspection.check_returncode()


def compare_screenshot(name, scale, references=HERE):
    image = OUTPUT / name
    # Clipboard and key replies can precede frame presentation. Wait for stable pixels.
    previous = None
    def settled():
        nonlocal previous
        run("grim", "-s", str(scale), "-o", "HEADLESS-1", str(image))
        current = image.read_bytes()
        same = previous == current
        previous = current
        return same
    until(settled, "stable screenshot")
    if os.environ.get('MUNDANE_CAPTURE_ONLY') == '1':
        return
    baseline = references / name
    if not baseline.exists():
        raise RuntimeError(f"Review {image} and supply {name}")
    # PAM includes the dimensions and uncompressed RGBA samples. Compare bytes
    # directly so even a one-level change in one channel fails the reference.
    def raster(path):
        return subprocess.check_output(
            ["magick", str(path), "-alpha", "on", "-depth", "8", "pam:-"], timeout=15)
    if raster(baseline) != raster(image):
        raise AssertionError(f"Pixel mismatch: compare {image} with {baseline}")


def check_window_actions(log):
    def request(name, count=1):
        until(lambda: log.read_text(errors="replace").count("." + name + "()") >= count, name)
    def state(name):
        run("wlrctl", "toplevel", "waitfor", APP_ID, f"state:{name}")
    key("F6")  # Find, Minimize, Restore, Close
    key("Tab")
    key("Tab")
    key("Return")
    request("unset_maximized")
    key("Return")
    request("set_maximized")
    key("Tab", "shift")
    key("Return")
    state("minimized")
    run("wlrctl", "toplevel", "focus", APP_ID)
    state("active")
    run(str(ROOT / "target/debug/examples/header-pointer"), str(RUNTIME / os.environ["WAYLAND_DISPLAY"]))
    request("unset_maximized", 2)
    run("wlrctl", "toplevel", "maximize", APP_ID)
    run(str(ROOT / "target/debug/examples/header-pointer"), str(RUNTIME / os.environ["WAYLAND_DISPLAY"]), "drag")
    until(lambda: ".move(wl_seat" in log.read_text(errors="replace"), "interactive move request")
    # Focus the content explicitly, then enter the header and close by keyboard.
    key("Escape")
    key("F6")
    for _ in range(3):
        key("Tab")
    key("Return")


def file_conflict(folder):
    cache = Path(os.environ["XDG_CACHE_HOME"]) / "thumbnails/normal"
    until(lambda: list(cache.glob("*.png")), "visible palette thumbnail")
    key("Home")
    key("Down")  # draft.txt
    key("F2")
    type_text("notes.txt")
    key("Return")
    notes = folder / "notes.txt"
    until(notes.is_file, "rename")
    key("c", "ctrl")
    key("Home")
    key("Return")  # Open Archive with the other notes.txt.
    key("v", "ctrl")
    return notes


def exercise_files(folder, scale, profile, labwc, inspect_conflict):
    notes = file_conflict(folder)
    name = f"files-{'labwc' if labwc else 'sway'}-{profile}.png"
    compare_screenshot(name, scale)
    if inspect_conflict:
        inspect_conflict()
    dest = folder / "Archive/notes.txt"
    key("Escape")
    assert dest.read_text() == "Earlier survey notes\n"
    key("v", "ctrl")
    key("Tab")
    key("Tab")
    key("Return")
    until(lambda: dest.read_bytes() == notes.read_bytes(), "reviewed replacement")
    key("BackSpace")
    assert sorted(p.name for p in folder.iterdir()) == ["Archive", "notes.txt", "palette.png"]
    key("v", "ctrl")
    key("Tab")
    key("Return")
    copied = folder / "notes (copy).txt"
    until(copied.is_file, "keep both")
    assert copied.read_bytes() == notes.read_bytes()
    key("x", "ctrl")  # The completed copy is selected.
    key("Home")
    key("Return")  # Archive
    key("v", "ctrl")
    moved = folder / "Archive" / copied.name
    until(moved.is_file, "move between folders")
    assert not copied.exists()
    assert "fixture-text.desktop" in output("gio", "mime", "text/plain").splitlines()[0]
    if labwc:
        (RUNTIME / "cancel-open").touch()
        key("Return")
        until((RUNTIME / "cancelled").exists, "portal cancellation")
        assert not (Path.home() / "opened").exists()
        (RUNTIME / "cancel-open").unlink()
    key("Return")  # Open in the private default handler.
    until(lambda: (Path.home() / "opened").exists(), "preferred text handler")
    assert (Path.home() / "opened").read_text() == str(moved)
    if labwc:
        until((RUNTIME / "accepted").exists, "accepted portal response")
    key("Delete")
    key("Tab")
    key("Return")
    trash = Path(os.environ["XDG_DATA_HOME"]) / "Trash"
    until(lambda: (trash / "files" / copied.name).is_file(), "trashed item")
    assert not moved.exists() and notes.is_file()
    assert (trash / "info" / (copied.name + ".trashinfo")).is_file()
    print(f"Passed {name}, rename, collision, copy, move, preferred handler and Trash", flush=True)


def exercise_monitor(args, scale, stack, app, app_id):
    until((RUNTIME / "sampled").exists, "two process samples")
    key("c")
    key("Home")
    key("Down")  # file-indexer, second by CPU, 128 MiB RSS
    if args.profile == "contrast":
        key("equal", "ctrl")
    name = f"monitor-{args.compositor}-{args.profile}.png"
    compare_screenshot(name, scale)
    if args.at_spi:
        run("python3", str(HERE / "accessibility.py"), str(app.pid), "Processes")
    key("Delete")
    if args.at_spi:
        inspect_accessibility(stack, app_id, app.pid)
    key("Escape")
    signals = RUNTIME / "signals"
    assert not signals.exists()
    key("Delete")
    key("Tab")
    key("Return")
    until(signals.is_file, "confirmed fixture SIGTERM")
    key("w", "ctrl")
    assert app.wait(timeout=8) == 0
    assert signals.read_text() == "1978 19780\n"
    print(f"Passed {name}, CPU sort, selection, cancel and confirmed termination")


def main():
    global OUTPUT
    parser = argparse.ArgumentParser()
    parser.add_argument("--app", choices=["terminal", "files", "text-editor", "system-monitor", "image-viewer", "calculator", "document-viewer", "video", "packages", "music", "archive-manager", "disk-usage", "disks", "clocks", "characters", "fonts", "weather", "maps"], default="terminal")
    parser.add_argument("--appearance", action="store_true", help="run the suite-wide live Settings portal test")
    parser.add_argument("--parity", choices=["before", "after"], help="compare the daily-use scenario for this app")
    parser.add_argument("--polish", action="store_true", help="compare the user-facing polish states on headless Sway")
    parser.add_argument("--missing-video-decoder", action="store_true", help="inspect Video with the vpx plugin absent")
    parser.add_argument("--fields", action="store_true", help="exercise two-window activation, password and multiline fields")
    parser.add_argument("--cost", type=Path, help="directory containing the two release cost fixture binaries")
    parser.add_argument("--at-spi", action="store_true")
    parser.add_argument("--profile", choices=["standard", "fractional", "contrast"], default="standard")
    parser.add_argument("--compositor", choices=["sway", "labwc"], default="sway")
    parser.add_argument("--mundanede-echo", type=Path, metavar="CHECKOUT", help="test physical typing echo in a scratch build of mundaneDE")
    parser.add_argument("--round-four", choices=["before", "after"], help="round four daily-use comparisons on private Sway")
    parser.add_argument("--round-three", choices=["before","after"], help="round three isolated daily-use workflows")
    parser.add_argument("--round-two", choices=["before","after"], help="daily-use round two workflow")
    parser.add_argument("--design-sheet",choices=["core","panel","menus","settings","notifications"],help="capture the shared components on private labwc")
    args = parser.parse_args()
    if args.design_sheet:
        args.compositor="labwc"
    if args.mundanede_echo:
        args.at_spi = True
    if args.missing_video_decoder:
        args.app = "video"
        args.at_spi = True
    if args.cost or args.fields:
        args.at_spi = True
    if args.at_spi:
        suite = "fields" if args.fields else "cost" if args.cost else args.app
        if args.mundanede_echo:
            suite = "mundanede-echo"
        if args.missing_video_decoder:
            suite = "missing-video-decoder"
        OUTPUT = OUTPUT / f"accessibility-{suite}-{args.profile}"
        OUTPUT.mkdir(exist_ok=True)
    labwc = args.compositor == "labwc"
    if args.appearance and (labwc or args.profile != "standard" or args.at_spi):
        parser.error("Appearance references use the standard Sway profile.")
    if args.at_spi and labwc:
        parser.error("The AT-SPI control window requires the Sway test configuration.")
    # Requiring lavapipe prevents an accidental pass using the host GPU.
    icd = os.environ.get("VK_DRIVER_FILES") or next(iter(Path("/usr/share/vulkan/icd.d").glob("lvp_icd*.json")), None)
    if not icd or not Path(icd).is_file() or "lvp" not in Path(icd).read_text():
        raise RuntimeError("Set VK_DRIVER_FILES to Mesa's lavapipe ICD manifest")
    os.environ["VK_DRIVER_FILES"] = str(icd)
    contrast = args.profile == "contrast"
    os.environ["MUNDANE_HIGH_CONTRAST"] = "1" if contrast else "0"
    configure_fonts()
    scale = {"standard": 1, "fractional": 1.5, "contrast": 2}[args.profile]
    config = RUNTIME / "sway.conf"
    config.write_text(f"""xwayland disable
default_border none
default_floating_border none
output HEADLESS-1 mode {int(800*scale)}x{int(500*scale)} scale {scale}
seat seat0 fallback true
""")
    shell = Path.home() / "shell"
    shell.write_text("#!/bin/sh\nexport PS1='$ ' PROMPT_COMMAND='touch .prompt-ready' HISTFILE=/dev/null\nexec bash --noprofile --norc -i\n")
    shell.chmod(0o755)
    os.environ["SHELL"] = str(shell)
    (Path.home() / "notes.txt").write_text("Terminal checks\n\nSoftware rendering\nWide characters   東京\n")
    with ExitStack() as stack:
        if labwc:
            lab_config = RUNTIME / "labwc"
            lab_config.mkdir()
            (lab_config / "rc.xml").write_text("""<labwc_config>
<core><decoration>client</decoration><xwaylandPersistence>no</xwaylandPersistence></core>
<keyboard><default/></keyboard><windowRules>
<windowRule identifier="org.joelinux.mundane.*"><action name="Maximize"/></windowRule>
</windowRules></labwc_config>""")
            compositor = launch(stack, "labwc", "-C", str(lab_config), log="labwc.log")
        else:
            compositor = launch(stack, "sway", "-c", str(config), log="sway.log")
            sock = until(lambda: next(RUNTIME.glob("sway-ipc.*.sock"), None), "private Sway IPC")
            os.environ["SWAYSOCK"] = str(sock)
        display = until(lambda: next((p for p in RUNTIME.glob("wayland-*") if p.is_socket()), None), "private Wayland socket")
        os.environ["WAYLAND_DISPLAY"] = display.name
        run("dbus-update-activation-environment", "WAYLAND_DISPLAY", "XDG_CURRENT_DESKTOP")
        if args.design_sheet:
            from design_check import check
            check(args.design_sheet, stack, launch, until, key, compare_screenshot, ROOT, RUNTIME, OUTPUT, recording_profile)
            return
        if args.appearance:
            from appearance_check import check
            check(stack, ROOT, RUNTIME, launch, until, key, compare_screenshot)
            return
        if args.app in ("packages", "music", "archive-manager", "disk-usage", "disks", "clocks", "characters", "fonts", "weather", "maps") or (args.at_spi and args.app in (
                "terminal", "files", "image-viewer", "calculator", "document-viewer", "video") and not args.cost):
            launch(stack, "wtype", "-k", "Shift_L", "-s", "300000", log="seat.log")
        if args.at_spi:
            from gi.repository import Gio, GLib
            launch(stack, "/usr/lib/at-spi-bus-launcher", "--launch-immediately", log="a11y-bus.log")
            bus = Gio.bus_get_sync(Gio.BusType.SESSION)
            def private_address():
                try:
                    return bus.call_sync("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus",
                                         "GetAddress", None, None, Gio.DBusCallFlags.NONE, 1000, None).unpack()[0]
                except GLib.Error:
                    return None
            address = until(private_address, "private accessibility launcher")
            socket = Path(address.split("unix:path=", 1)[1].split(",")[0])
            assert socket.is_relative_to(RUNTIME) and socket.is_socket(), address
            os.environ["AT_SPI_BUS_ADDRESS"] = address
            properties = ("gdbus", "call", "--session", "--dest", "org.a11y.Bus", "--object-path", "/org/a11y/bus", "--method")
            output(*properties, "org.freedesktop.DBus.Properties.Set", "org.a11y.Status", "IsEnabled", "<true>")
            assert "true" in output(*properties, "org.freedesktop.DBus.Properties.Get", "org.a11y.Status", "IsEnabled")
            launch(stack, "/usr/lib/at-spi2-registryd", log="registry.log")
            until(lambda: "true" in output("gdbus", "call", "--address", address, "--dest", "org.freedesktop.DBus",
                  "--object-path", "/org/freedesktop/DBus", "--method", "org.freedesktop.DBus.NameHasOwner",
                  "org.a11y.atspi.Registry"), "private AT-SPI registry")
            if args.mundanede_echo:
                from mundanede_echo import check
                check(args.mundanede_echo.resolve(), stack, launch, until, ROOT, RUNTIME, OUTPUT, recording_profile())
                return
            launch(stack, "python3", str(HERE / "accessibility.py"), "--control", log="gtk.log")
            until(lambda: "AT-SPI control" in output("swaymsg", "-r", "-t", "get_tree"), "GTK window")
            run("swaymsg", "-q", '[title="AT-SPI control"] move scratchpad')
            os.environ["EVENT_LOG"] = str(OUTPUT / ("events-" + ("fields" if args.fields else args.app) + ".jsonl"))
            launch(stack, "python3", str(HERE / "accessibility.py"), "--observe", log=f"events-{'fields' if args.fields else args.app}.log")
            until((RUNTIME / "observer-ready").exists, "registered GTK control and event observer")
            if args.fields:
                output(*properties, "org.freedesktop.DBus.Properties.Set", "org.a11y.Status", "IsEnabled", "<false>")
                fixture = launch(stack, str(ROOT / "target/debug/examples/accessible-fields"), log="fields.log")
                until(lambda: "Password fixture" in output("swaymsg", "-r", "-t", "get_tree"), "two mapped windows")
                subprocess.run(["python3", str(HERE / "accessibility.py"), str(fixture.pid), "Fields"],
                               check=True, timeout=60)
                return
            if args.cost:
                from frame_cost import measure
                def set_enabled(enabled):
                    output(*properties, "org.freedesktop.DBus.Properties.Set", "org.a11y.Status",
                           "IsEnabled", "<true>" if enabled else "<false>")
                measure(args.cost.resolve(), set_enabled)
                return
            if args.app == "terminal":
                transcript = recording_profile()
                launch(stack, "orca", "--debug", "--debug-file", str(OUTPUT / "orca-debug.log"), log="orca.log", env={**os.environ, "PYTHONPATH": str(HERE)+os.pathsep+os.environ.get("PYTHONPATH", ""),
                       "ORCA_TRANSCRIPT": str(transcript), "PULSE_SERVER": "unix:"+str(RUNTIME / "no-audio"),
                       "SPEECHD_ADDRESS": "unix_socket:"+str(RUNTIME / "no-speechd")})
                until(lambda: transcript.stat().st_size, "Orca recording backend")
        def command(*args):
            return output("swaymsg", "-r", *args)
        def resize(width, height):
            mode = f"{int(width*scale)}x{int(height*scale)}"
            if labwc:
                run("wlr-randr", "--output", "HEADLESS-1", "--custom-mode", mode, "--scale", str(scale))
            else:
                command("output", "HEADLESS-1", "mode", mode)

        if labwc:
            resize(800, 500)
        suite_app = args.app
        app_id = f"org.joelinux.mundane.{suite_app.title().replace('-', '')}"
        argv = [str(Path(os.environ.get(f"MUNDANE_{suite_app.upper().replace('-', '_')}_BIN", ROOT / f"target/debug/mundane-{suite_app}")).resolve())]
        if suite_app == "system-monitor" and args.round_three:
            argv = [str(Path(argv[0]).parent / "examples/monitor-fixture")]
        if suite_app in ("weather", "maps"):
            if args.round_four:
                (RUNTIME / "multi-day").touch()
                launch(stack, "python3", str(ROOT / "tests/providers/location_portal.py"), log="location-portal.log")
                until((RUNTIME / "location-ready").exists, "private location portal")
            provider = launch(stack, "python3", str(ROOT / "tests/providers/server.py"), str(RUNTIME), log="provider.log")
            address = until(lambda: (RUNTIME / "address").exists() and (RUNTIME / "address").read_text(), "loopback provider")
            if suite_app == "weather":
                os.environ["MUNDANE_WEATHER_ENDPOINT"] = address + "/forecast"
            else:
                os.environ["MUNDANE_MAPS_TILEJSON"] = address + "/tilejson"
                os.environ["MUNDANE_MAPS_SEARCH"] = address + "/search"
        if suite_app == "clocks":
            argv = [str(Path(argv[0]).parent / "examples/clocks-fixture")]
            launch(stack, "python3", str(ROOT / "tests/clocks/notifications.py"), log="notifications.log")
            until((RUNTIME / "notify-ready").exists, "private notification service")
        if suite_app == "disks":
            fake_disks = RUNTIME / "udisks"
            fake_disks.mkdir()
            # Route the release binary's system-bus connection to this test's private daemon.
            os.environ["DBUS_SYSTEM_BUS_ADDRESS"] = os.environ["DBUS_SESSION_BUS_ADDRESS"]
            launch(stack, "python3", str(ROOT / "tests/disks/udisks.py"),
                   os.environ["DBUS_SYSTEM_BUS_ADDRESS"], str(fake_disks), log="udisks.log")
            until((fake_disks / "ready").exists, "fake UDisks bus owner")
        if suite_app == "disk-usage":
            survey = Path.home() / "Coastal survey"
            for name, size in [("Charts/soundings.bin", 512 * 1024), ("Photographs/coast.dat", 256 * 1024),
                               ("Field notes.txt", 8192)]:
                path = survey / name
                path.parent.mkdir(exist_ok=True, parents=True)
                path.write_bytes(bytes(size))
            restricted = survey / "Private records"
            restricted.mkdir(mode=0o700)
            restricted.chmod(0)
            stack.callback(restricted.chmod, 0o700)
            argv.append(str(survey))
            launch(stack, "python3", str(HERE / "chooser_portal.py"), str(survey / "Charts"), log="usage-portal.log")
            until((RUNTIME / "chooser-ready").exists, "private FileChooser")
        if suite_app == "archive-manager":
            archive_dest = Path.home() / "Extracted"
            archive_dest.mkdir()
            argv.append(str(ROOT / "tests/fixtures/archive/Coastal survey.zip"))
            if args.polish:
                from zipfile import ZipFile
                empty = Path.home() / "Empty.zip"
                with ZipFile(empty, "w"):
                    pass
                archive_dest = Path.home() / "Notes.zip"
                with ZipFile(archive_dest, "w") as z:
                    z.writestr("Notes.txt", "Survey notes")
                argv[-1] = str(empty)
            launch(stack, "python3", str(HERE / "chooser_portal.py"), str(archive_dest), log="archive-portal.log")
            until((RUNTIME / "chooser-ready").exists, "private FileChooser")
        if suite_app == "music":
            from video_audio import start, record, check_tone
            from video_plugins import MUSIC_PLUGINS, configure, loaded_plugins
            configure(RUNTIME / "gst-plugins", plugins=MUSIC_PLUGINS,
                      extra=("mpg123", "isomp4") if args.round_four else (),
                      optional=("libav",) if args.round_four else ())
            start(stack, RUNTIME, launch, until)
            recording = record(stack, RUNTIME, launch, "flac")
            tracks = ROOT / "tests/fixtures/music"
            if args.round_four:
                from round_four import speech_track
                argv.append(str(speech_track(Path.home())))
            else:
                argv.extend(str(tracks / name) for name in ("01.flac", "03.ogg", "02.opus"))
            launch(stack, "python3", str(HERE / "chooser_portal.py"), *(str(tracks / n) for n in (("Radio.mp3", "Interview.m4a") if args.round_four else ("Field recording.wav",))),
                   log="music-portal.log")
            until((RUNTIME / "chooser-ready").exists, "private FileChooser")
        if suite_app == "packages":
            if args.round_three:
                from catalog_fixture import create
                create()
            pkg_dir = RUNTIME / "packages"
            pkg_dir.mkdir()
            for cmd in ("dave", "pkexec"):
                (pkg_dir / cmd).symlink_to(ROOT / "tests/packages/fixture.py")
            argv = [str(Path(argv[0]).parent / "examples/packages-fixture"), str(pkg_dir)]
        if suite_app == "files":
            os.environ["GSETTINGS_BACKEND"] = "keyfile"
            run("gsettings", "set", "org.gnome.desktop.interface", "icon-theme", "Papirus")
            folder = create_files_fixture()
            if args.parity:
                config_dir = Path(os.environ["XDG_CONFIG_HOME"])
                (config_dir / "user-dirs.dirs").write_text('XDG_DOCUMENTS_DIR="$HOME/Documents"\n')
                bookmarks = config_dir / "gtk-3.0/bookmarks"
                bookmarks.parent.mkdir(exist_ok=True)
                bookmarks.write_text((folder / "Archive").as_uri() + " Survey archive\n")
            if labwc:
                launch(stack, "python3", str(HERE / "open_portal.py"), log=f"portal-{args.profile}.log")
                until((RUNTIME / "portal-ready").exists, "private OpenURI portal")
            argv.append(str(folder))
        if suite_app == "video":
            from video_audio import start, record, check_tone
            from video_plugins import PLUGINS, MP4_PLUGINS, configure, loaded_plugins
            configure(RUNTIME / "gst-plugins", extra=MP4_PLUGINS if args.round_four else (),
                      omit=("vpx",) if args.missing_video_decoder else (), optional=("libav",) if args.round_four else ())
            start(stack, RUNTIME, launch, until)
            if not args.missing_video_decoder:
                recording = record(stack, RUNTIME, launch, "vp8-opus")
            argv.append(str(ROOT / "tests/fixtures/Coastal passage.webm"))
            second = ROOT / "tests/fixtures" / ("Passage.mp4" if args.round_four else "Harbour.webm")
            launch(stack, "python3", str(HERE / "chooser_portal.py"), str(second), str(ROOT / "tests/fixtures/Passage with AAC.mp4") if args.round_four else str(second), log="video-portal.log")
            until((RUNTIME / "chooser-ready").exists, "private FileChooser")
        if suite_app == "document-viewer":
            document = ROOT / "tests/fixtures/Coastal survey.pdf"
            if args.round_three:
                os.environ["MUNDANE_TEST_PRINT"] = "1"
                document = Path.home() / "Field guide.pdf"
                run("python3", str(ROOT / "tests/fixtures/coastal_survey.py"), str(document))
            argv.append(str(document))
            second = Path.home() / "Survey copy.pdf"
            second.write_bytes(document.read_bytes())
            launch(stack, "python3", str(HERE / "chooser_portal.py"), str(second), log="document-portal.log")
            until((RUNTIME / "chooser-ready").exists, "private FileChooser")
        if suite_app == "image-viewer":
            argv.append(str(ROOT / "tests/fixtures/North Point.jpg"))
            if args.round_three:
                gallery = Path.home() / "Coastal charts"
                gallery.mkdir()
                first = gallery / "01 North Point.jpg"
                first.write_bytes(Path(argv[-1]).read_bytes())
                run("magick", str(first), "-auto-orient", "-resize", "320x200", "-colorspace", "gray", str(gallery/"02 North Point.webp"))
                (gallery/"03 Harbour.svg").write_bytes((ROOT/"tests/fixtures/gallery/Harbour.svg").read_bytes())
                argv[-1]=str(first)
            second = Path.home() / "Blue tile.png"
            png(second, 16, 12, lambda x, y: (24, 63, 190, 128))
            launch(stack, "python3", str(HERE / "chooser_portal.py"), str(second), log="viewer-portal.log")
            until((RUNTIME / "chooser-ready").exists, "private FileChooser")
        if suite_app == "text-editor":
            document = Path.home() / "Field notes.txt"
            original = "Coastal survey\r\n\r\nStation 04   東京\r\nWater temperature   12 °C\r\n\r\n".encode()
            if args.round_two:
                document=Path.home()/"survey.py"
                original=b'# A coastal weather report\nstation = "North quay"\n\ndef report(station):\n    return f"Forecast for {station}"\n'
            document.write_bytes(original)
            (Path.home() / "Second.txt").write_text("Second document\n")
            launch(stack, "python3", str(HERE / "chooser_portal.py"), log=f"chooser-{args.profile}.log")
            until((RUNTIME / "chooser-ready").exists, "private FileChooser")
            argv.append(str(document))
        if args.polish and suite_app in ("image-viewer", "document-viewer", "video"):
            bad = {"image-viewer":"Damaged.png", "document-viewer":"Damaged.pdf", "video":"Damaged.webm"}[suite_app]
            (Path.home() / bad).write_bytes(b"Incomplete file")
            argv[-1] = bad
        if args.parity and suite_app == "terminal":
            (Path.home() / "Documents").mkdir(exist_ok=True)
        log = f"{suite_app}-{args.compositor}-{args.profile}.log"
        app = launch(stack, *argv,
                     log=log, env={**os.environ, "WAYLAND_DEBUG": "client"})
        def mapped():
            if app.poll() is not None or compositor.poll() is not None:
                raise RuntimeError(f"{suite_app} or compositor exited. See {OUTPUT}.")
            if labwc:
                return "ack_configure" in (OUTPUT / log).read_text(errors="replace")
            return app_id in command("-t", "get_tree")
        until(mapped, f"mapped {suite_app}")
        if args.round_four:
            from round_four import video, music, calculator
            if labwc:
                raise ValueError("Round-four comparisons use headless Sway.")
            if suite_app == "video":
                video(args.round_four, app, ROOT, OUTPUT / log, key, until, compare_screenshot,
                      lambda tag: record(stack, RUNTIME, launch, tag), check_tone)
            elif suite_app == "music":
                music(args.round_four, ROOT, OUTPUT / log, key, until, compare_screenshot,
                      lambda tag: record(stack, RUNTIME, launch, tag), check_tone)
            elif suite_app == "calculator":
                calculator(args.round_four, ROOT, key, type_text, compare_screenshot, clipboard)
            elif suite_app == "clocks":
                from round_four import clocks
                clocks(args.round_four, app, ROOT, key, type_text, until, compare_screenshot)
            elif suite_app == "weather":
                from round_four import weather
                weather(args.round_four, ROOT, OUTPUT / log, key, type_text, until, compare_screenshot)
            elif suite_app == "maps":
                from round_four import maps
                maps(args.round_four, ROOT, OUTPUT / log, key, type_text, until, compare_screenshot)
            elif suite_app == "characters":
                from round_four import characters
                characters(args.round_four, ROOT, key, type_text, until, compare_screenshot, clipboard)
            else:
                raise ValueError(f"No round-four scenario for {suite_app}")
            return
        if args.round_three:
            from round_three import exercise
            exercise(suite_app, args.round_three, key, type_text, until, compare_screenshot, ROOT, OUTPUT / log)
            return
        if args.round_two:
            from round_two import exercise
            def restart():
                app.kill()
                app.wait(timeout=5)
                return launch(stack, argv[0], log="recovered.log",env={**os.environ,"WAYLAND_DEBUG":"client"})
            exercise(suite_app,args.round_two,key,type_text,until,compare_screenshot,ROOT,OUTPUT/log,restart)
            return
        if args.parity:
            from parity import exercise
            exercise(suite_app, args.parity == "before", folder if suite_app == "files" else None,
                     key, type_text, until, compare_screenshot, ROOT, OUTPUT / log)
            return
        if args.polish:
            def shot(state):
                compare_screenshot(suite_app + "-" + state + ".png", scale, HERE if suite_app == "files" else ROOT / "docs/polish/after")
            key("Shift_L")
            if suite_app == "files":
                resize(720, 440)
                file_conflict(folder)
                shot("replace")
                key("Escape")
                key("BackSpace")
                key("v", "ctrl")
                shot("same-item")
            elif suite_app == "clocks":
                key("F6")
                shot("header")
                key("F6")
                key("Delete")
                shot("empty")
                key("n", "ctrl")
                type_text("Europe/Oslo")
                key("Down")
                shot("one-zone")
                key("n", "ctrl")
                key("a", "ctrl")
                type_text("unmapped time zone")
                key("Down")
                shot("no-results")
            elif suite_app == "characters":
                type_text("U+2605")
                key("Return")
                key("F6")
                shot("header")
            elif suite_app == "fonts":
                until(lambda: "DejaVu Sans · Book — Fonts" in (OUTPUT / log).read_text(), "font preview")
                key("F6")
                shot("header")
            elif suite_app == "weather":
                type_text("Oslo")
                key("Return")
                until(lambda: "Oslo — Weather" in (OUTPUT / log).read_text(), "forecast")
                key("F6")
                shot("forecast")
                (RUNTIME / "unavailable").touch()
                key("l", "ctrl")
                type_text("London")
                key("Return")
                until(lambda: len((RUNTIME / "requests").read_text().splitlines()) == 2, "unavailable forecast")
                shot("error")
            elif suite_app == "archive-manager":
                until(lambda: "Empty.zip — Archive Manager" in (OUTPUT / log).read_text(), "empty archive")
                shot("empty")
                key("o", "ctrl")
                until(lambda: "Notes.zip — Archive Manager" in (OUTPUT / log).read_text(), "single-entry archive")
                shot("one-entry")
            elif suite_app == "maps":
                until(lambda: "Map · Zoom 2 — Maps" in (OUTPUT / log).read_text(), "vector tiles")
                key("F6")
                shot("header")
                key("f", "ctrl")
                (RUNTIME / "single-result").touch()
                type_text("quay")
                key("Return")
                until(lambda: "Place results — Maps" in (OUTPUT / log).read_text(), "one place")
                shot("one-result")
            elif suite_app in ("image-viewer", "document-viewer", "video"):
                # The status is produced asynchronously after probing the damaged file.
                time.sleep(1)
                shot("error")
            else:
                raise ValueError(f"No polish reference for {suite_app}")
            return
        if suite_app == "maps":
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            def titled(title):
                return title in (OUTPUT / log).read_text()
            until(lambda: titled("Map · Zoom 2 — Maps"), "initial vector map")
            key("f", "ctrl")
            type_text("harbour")
            assert "/search?" not in (RUNTIME / "requests").read_text()
            key("Return")
            until(lambda: titled("Place results — Maps"), "explicit place search")
            key("Down")
            key("Return")
            until(lambda: titled("Harbour Point · Zoom 12 — Maps"), "selected place")
            key("Right")
            key("equal")
            until(lambda: titled("Harbour Point · Zoom 13 — Maps"), "zoomed harbour")
            if args.profile == "contrast":
                key("equal", "ctrl")
            name = f"maps-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            key("s", "ctrl")
            until(lambda: list((Path(os.environ["XDG_CACHE_HOME"]) / "mundane-maps").glob("*.area")), "saved offline area")
            calls = (RUNTIME / "requests").read_text().splitlines()
            assert sum("/search?" in line for line in calls) == 1
            assert all("mundane-maps/" in line for line in calls)
            provider.terminate()
            provider.wait(timeout=5)
            key("o", "ctrl")
            until(lambda: titled("Harbour Point · Zoom 13 · Offline — Maps"), "reopened view with the provider stopped")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, explicit search, panning, zoom and offline area")
            return
        if suite_app == "weather":
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            type_text("Oslo")
            key("Return")
            until(lambda: "Oslo — Weather" in (OUTPUT / log).read_text(), "received Oslo forecast")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            provider.terminate()
            provider.wait(timeout=5)
            # Give the reopened client a persistent seat with an installed keymap.
            launch(stack, "wtype", "-k", "Shift_L", "-s", "300000", log="reopen-seat.log",
                   env={**os.environ, "WAYLAND_DEBUG": "client"})
            until(lambda: ".key(" in (OUTPUT / "reopen-seat.log").read_text(), "reopen seat keymap")
            app = launch(stack, *argv, log=log, env={**os.environ, "WAYLAND_DEBUG": "client"})
            until(lambda: "Oslo — Weather" in (OUTPUT / log).read_text(), "reopened offline forecast")
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            if args.profile == "contrast":
                key("equal", "ctrl")
            name = f"weather-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, persisted location and offline forecast")
            return
        if suite_app == "fonts":
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            until(lambda: "DejaVu Sans · Book — Fonts" in (OUTPUT / log).read_text(), "first installed face")
            key("Right")
            until(lambda: "DejaVu Sans · Bold — Fonts" in (OUTPUT / log).read_text(), "selected bold face")
            key("e", "ctrl")
            type_text("Coast · Αθήνα · שלום")
            key("Return")
            if args.profile == "contrast":
                key("equal", "ctrl")
            name = f"fonts-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, face selection and multiscript preview")
            return
        if suite_app == "characters":
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            for query, glyph in [("U+1D11E", "𝄞"), ("black star", "★")]:
                key("f", "ctrl")
                type_text(query)
                key("Return")
                key("c", "ctrl")
                until(lambda: clipboard() == glyph, f"{glyph} on the private clipboard")
            if args.profile == "contrast":
                key("equal", "ctrl")
            name = f"characters-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, Unicode name and code point search and copy")
            return
        if suite_app == "clocks":
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            key("n", "ctrl")
            type_text("New York")
            key("Return")
            cities = Path(os.environ["XDG_CONFIG_HOME"]) / "mundane-clocks/cities"
            until(lambda: cities.exists() and "America/New_York" in cities.read_text(), "saved New York")
            if args.profile == "contrast":
                key("equal", "ctrl")
            name = f"clocks-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            key("t", "ctrl")
            key("a", "ctrl")
            type_text("1")
            key("Return")
            until((RUNTIME / "notified").exists, "countdown notification")
            key("Return")
            until((RUNTIME / "dismissed").exists, "dismissed notification")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, city search, timer expiry and dismissal")
            return
        if suite_app == "disks":
            until((fake_disks / "listed").exists, "UDisks inventory")
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            key("Home")
            key("Down")
            key("Down")
            key("m", "ctrl")
            until((fake_disks / "mounted").exists, "fake volume mounted")
            if args.profile == "contrast":
                key("equal", "ctrl")
            name = f"disks-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            key("m", "ctrl")
            until(lambda: not (fake_disks / "mounted").exists(), "fake volume unmounted")
            (fake_disks / "remove-during").touch()
            key("m", "ctrl")
            until((fake_disks / "removed").exists, "hot removal during requested mount")
            calls = [json.loads(line) for line in (fake_disks / "calls.jsonl").read_text().splitlines()]
            target = "/org/freedesktop/UDisks2/block_devices/testdisk1"
            assert calls == [[target, op, True, {}] for op in ["Mount", "Unmount", "Mount"]], calls
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, private UDisks mount, unmount and hot removal")
            return
        if suite_app == "disk-usage":
            until(lambda: "Coastal survey — Disk Usage" in (OUTPUT / log).read_text(), "finished usage scan")
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            key("a")  # Logical sizes are identical across CI filesystems.
            key("Home")
            key("Return")
            key("BackSpace")
            key("End")
            if args.profile == "contrast":
                key("equal", "ctrl")
            name = f"usage-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            key("o", "ctrl")
            until(lambda: "Charts — Disk Usage" in (OUTPUT / log).read_text(), "scan folder chosen through portal")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, unreadable directory, drill down, back and portal scan")
            return
        if suite_app == "archive-manager":
            until(lambda: "Coastal survey.zip — Archive Manager" in (OUTPUT / log).read_text()
                  and "wl_keyboard" in (OUTPUT / log).read_text() and ".enter(" in (OUTPUT / log).read_text(),
                  "mapped and loaded archive")
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            key("Home")
            key("Down")
            key("space")
            key("Down")
            key("space")
            if args.profile == "contrast":
                key("equal", "ctrl")
            name = f"archive-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            key("e", "ctrl")
            result = archive_dest / "Coastal survey (extracted)"
            until(result.is_dir, "published archive extraction")
            assert (result / "Charts/channel.txt").read_bytes() == b"East channel\n"
            assert (result / "Charts/tides.csv").is_file() and not (result / "Notes").exists()
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, marked subtree members and portal extraction")
            return
        if suite_app == "music":
            def playing(title):
                return title + " — Music" in (OUTPUT / log).read_text(errors="replace")
            until(lambda: playing("Ebb Tide"), "tagged FLAC playback")
            check_tone(recording, until, 330)
            key("o", "ctrl")
            until((RUNTIME / "OpenFile").exists, "append WAV through private chooser")
            key("Escape")
            key("Down")
            key("Down")
            key("Up", "ctrl")
            recording = record(stack, RUNTIME, launch, "opus")
            key("n")
            until(lambda: playing("Harbour Lights"), "skip in reordered queue")
            check_tone(recording, until, 440)
            key("space")
            key("Home")
            key("Right")
            if args.profile == "contrast":
                key("equal", "ctrl")
            name = f"music-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
                key("Escape")  # Return from accessible reorder buttons to the queue.
            recording = record(stack, RUNTIME, launch, "vorbis")
            key("End")
            key("space")
            until(lambda: playing("North Channel"), "advance on real Opus EOS")
            check_tone(recording, until, 550)
            recording = record(stack, RUNTIME, launch, "wav")
            key("End")
            until(lambda: playing("Field recording.wav"), "untagged WAV filename")
            check_tone(recording, until, 660)
            loaded = loaded_plugins(app.pid)
            assert loaded == set(MUSIC_PLUGINS), (loaded, MUSIC_PLUGINS)
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, four audio formats, metadata, queue reorder, pause, EOS and private portal")
            return
        if suite_app == "packages":
            def calls():
                return [json.loads(line) for line in (pkg_dir / "calls.jsonl").read_text().splitlines()]
            until((pkg_dir / "calls.jsonl").exists, "initial package list")
            until(lambda: ".enter(" in (OUTPUT / log).read_text(errors="replace"), "package window on output")
            key("f", "ctrl")
            type_text("chart")
            key("Return")
            until(lambda: ["dave", "search", "--json", "--", "chart"] in calls(), "package search")
            key("Return")
            until(lambda: ["dave", "info", "--json", "chartroom"] in calls(), "package details")
            key("Return")
            plan = ["dave", "install", "--dry-run", "--json", "chartroom"]
            until(lambda: plan in calls(), "package plan")
            if args.profile == "contrast":
                key("equal", "ctrl")
            name = f"packages-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            key("Escape")
            assert not any(c[0] == "pkexec" for c in calls())
            key("Return")
            until(lambda: calls().count(plan) == 2, "second review")
            key("Tab")
            key("Return")
            until((pkg_dir / "applied").exists, "confirmed change")
            assert (pkg_dir / "applied").read_text() == "install\n"
            auth, = (c for c in calls() if c[0] == "pkexec")
            assert auth[:6] == ["pkexec", "/usr/bin/dave", "install", "-y", "--json", "--expect"]
            assert auth[6].startswith("sha256:") and auth[7:] == ["chartroom"]
            key("u", "ctrl")
            until(lambda: ["dave", "status", "--json"] in calls(), "deployment status")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, cancellation, one digest-bound change and update status")
            return
        if args.missing_video_decoder:
            run("python3", str(HERE / "accessibility.py"), str(app.pid), "MissingVideoDecoder")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print("Passed missing VP8 decoder diagnosis on the private accessibility bus")
            return
        if suite_app == "video":
            resize(720, 480)
            until(lambda: "Coastal passage.webm — Video" in (OUTPUT / log).read_text(), "playing VP8/Opus")
            check_tone(recording, until)
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            key("space")
            key("Home")
            key("Right")
            name = f"video-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            recording = record(stack, RUNTIME, launch, "vp9-vorbis")
            key("o", "ctrl")
            until((RUNTIME / "OpenFile").exists, "private video chooser response")
            until(lambda: "Harbour.webm — Video" in (OUTPUT / log).read_text(), "portal VP9/Vorbis")
            check_tone(recording, until)
            loaded = loaded_plugins(app.pid)
            assert loaded == set(PLUGINS), (loaded, PLUGINS)
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, paused seek and both WebM audio tracks on private PipeWire")
            return
        if suite_app == "document-viewer":
            resize(720, 560)
            until(lambda: "Coastal survey.pdf — Document Viewer" in (OUTPUT / log).read_text(), "opened PDF")
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            key("l", "ctrl")
            type_text("2")
            key("Return")
            key("f", "ctrl")
            type_text("high water")
            key("Return")
            key("equal", "ctrl")
            name = f"document-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            key("Escape")
            key("Prior")
            key("o", "ctrl")
            until((RUNTIME / "OpenFile").exists, "private PDF chooser response")
            until(lambda: "Survey copy.pdf — Document Viewer" in (OUTPUT / log).read_text(), "portal Open PDF")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, page jump, search, zoom and portal Open")
            return
        if suite_app == "calculator":
            resize(372, 550)
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            type_text("18.75 + 4.50")
            key("Return")
            key("r", "ctrl")
            type_text(" × 4")
            key("Return")
            name = f"calculator-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            else:
                key("h", "alt")
                key("Tab")
                key("Return")
                key("a", "ctrl")
                key("c", "ctrl")
                until(lambda: clipboard() == "23.25", "reuse earlier result")
                key("h", "alt")
                key("Tab", "shift")
                key("Return")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, keyboard calculation, result reuse and clear history")
            return
        if suite_app == "image-viewer":
            resize(720, 440)
            until(lambda: "North Point.jpg — Image Viewer" in (OUTPUT / log).read_text(), "oriented JPEG")
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            key("equal", "ctrl")
            # Compare only the canvas so changing the status text cannot satisfy zoom or pan.
            crop = f"{int(400*scale)}x{int(240*scale)}+{int(160*scale)}+{int(64*scale)}"
            def canvas_pixels(path):
                return subprocess.check_output(["magick", str(path), "-crop", crop, "rgb:-"], timeout=15)
            capture = RUNTIME / "zoomed.png"
            def snapshot():
                run("grim", "-s", str(scale), "-o", "HEADLESS-1", str(capture))
                return canvas_pixels(capture)
            centered = snapshot()
            key("Down")
            panned = snapshot()
            assert centered != panned, "zoomed image did not pan"
            key("0", "ctrl")
            name = f"viewer-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            assert centered != canvas_pixels(OUTPUT / name), "image did not return to fit"
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
            (RUNTIME / "cancel-choice").touch()
            key("o", "ctrl")
            until((RUNTIME / "OpenFile").exists, "cancelled Open")
            (RUNTIME / "cancel-choice").unlink()
            key("o", "ctrl")
            until(lambda: "Blue tile.png — Image Viewer" in (OUTPUT / log).read_text(), "portal PNG")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, zoom, pan, fit and portal Open")
            return
        if suite_app == "system-monitor":
            resize(720, 440)
            # Smoke-test the installed entry point using the real procfs service.
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            key("m")
            key("Home")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0

            # The same view receives deterministic counters and a recording signal sink.
            fixture = Path(argv[0]).parent / "examples/monitor-fixture"
            log = f"fixture-{args.compositor}-{args.profile}.log"
            app = launch(stack, str(fixture), log=log, env={**os.environ, "WAYLAND_DEBUG": "client"})
            until(mapped, "mapped monitor fixture")
            exercise_monitor(args, scale, stack, app, app_id)
            return
        if suite_app == "text-editor":
            resize(720, 440)
            until(lambda: "Field notes.txt" in (OUTPUT / log).read_text(errors="replace"), "document title")
            # The first virtual keyboard establishes the seat on a headless output.
            key("Shift_L")
            if labwc:
                run("wlrctl", "toplevel", "waitfor", app_id, "state:active")
            key("End", "ctrl")
            if contrast:
                key("equal", "ctrl")
            edit = "Inspect the tide pools."
            type_text(edit)
            key("f", "ctrl")
            type_text("tide pools")
            key("Return")
            name = f"editor-{args.compositor}-{args.profile}.png"
            compare_screenshot(name, scale)
            if args.at_spi:
                inspect_accessibility(stack, app_id, app.pid)
                command(f'[app_id="{app_id}"] focus')
            key("Escape")
            key("c", "ctrl")
            until(lambda: clipboard() == "tide pools", "selected search match")
            if labwc:
                key("F6")
                for _ in range(6):  # Open, Find, Save, Save As, Minimize, Restore, Close
                    key("Tab")
                key("Return")
                key("Escape")
                assert app.poll() is None
            # Exercise a real compositor close request on a modified document.
            if labwc:
                run("wlrctl", "toplevel", "close", app_id)
            else:
                command(f'[app_id="{app_id}"] kill')
            key("Escape")
            assert app.poll() is None and document.read_bytes() == original
            key("s", "ctrl")
            saved = original + edit.encode()
            until(lambda: document.read_bytes() == saved, "saved CRLF document")
            key("s", "ctrl", "shift")
            copy = Path.home() / "Saved copy.txt"
            until(lambda: copy.is_file() and copy.read_bytes() == saved and
                  "Saved copy.txt — Text Editor" in (OUTPUT / log).read_text(), "completed Save As")
            (RUNTIME / "cancel-choice").touch()
            key("o", "ctrl")
            until((RUNTIME / "OpenFile").exists, "cancelled Open")
            (RUNTIME / "cancel-choice").unlink()
            key("o", "ctrl")
            until(lambda: 'Second.txt — Text Editor' in (OUTPUT / log).read_text(errors="replace"), "portal Open")
            key("w", "ctrl")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            print(f"Passed {name}, find, unsaved close, save and portal choices")
            return
        if suite_app == "files":
            resize(720, 440)
            exercise_files(folder, scale, args.profile, labwc,
                           (lambda: inspect_accessibility(stack, app_id, app.pid, "FileConflict")) if args.at_spi else None)
            if args.at_spi:
                key("l", "ctrl")  # Inspect with the location field and dialog focused.
                inspect_accessibility(stack, app_id, app.pid)
                command(f'[app_id="{app_id}"] focus')
                key("Escape")
            key("w", "ctrl")
            assert app.wait(timeout=8) == 0
            return
        until(lambda: (Path.home() / ".prompt-ready").exists(), "interactive shell prompt")
        resize(640, 360)
        key("Shift_L")
        if contrast:
            key("equal", "ctrl")
        # A configure can precede keyboard focus on the first headless seat.
        # Wait until the shell has actually received a key after the resize.
        ready = Path.home() / ".prompt-ready"
        ready.unlink()
        def received_enter():
            key("Return")
            return ready.exists()
        until(received_enter, "input at the resized shell prompt")
        type_text("stty size > size.tmp && mv size.tmp size.txt")
        key("Return")
        until(lambda: (Path.home() / "size.txt").exists(), "shell command")
        reported = (Path.home() / "size.txt").read_text().strip()
        assert reported == ("11 56" if contrast else "12 62"), f"unexpected PTY dimensions {reported}"
        type_text("clear; cat notes.txt")
        key("Return")
        find_and_copy("Software rendering")
        prefix = "labwc-" if labwc else ""
        name = f"{prefix}terminal-{scale:g}{'-contrast' if contrast else ''}.png"
        compare_screenshot(name, scale)
        if contrast:
            key("f", "ctrl", "shift")
            type_text("Tokyo")
            key("Tab")
            key("Tab", "shift")
            compare_screenshot(f"{prefix}find-2-contrast.png", scale)
            key("Escape")
        if args.at_spi:
            key("f", "ctrl", "shift")
            type_text("Tokyo")
            inspect_accessibility(stack, app_id, app.pid)
            command(f'[app_id="{app_id}"] focus')
        if labwc:
            check_window_actions(OUTPUT / log)
        else:
            type_text("exit")
            key("Return")
        assert app.wait(timeout=8) == 0
        print(f"Passed {name}, PTY {reported}")


if __name__ == "__main__":
    main()
