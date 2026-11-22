"""Run inside the private session created by tests/wayland/run."""
import argparse
from files_fixture import create as create_files_fixture
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


def inspect_accessibility(stack, app_id, app_pid):
    # An accessible GTK control on the same bus distinguishes a missing bridge from a broken probe.
    control = launch(stack, "python3", str(HERE / "accessibility.py"), "--control", log="gtk.log")
    until(lambda: control.poll() is None and "AT-SPI control" in output("swaymsg", "-r", "-t", "get_tree"),
          "GTK control window")
    run("swaymsg", "-q", f'[app_id="{app_id}"] focus')
    key("Tab")
    key("Tab", "shift")
    run("python3", str(HERE / "accessibility.py"), str(app_pid))
    listing = output("orca", "--list-apps")
    (OUTPUT / f"orca-{app_id}.txt").write_text(listing + "\n")
    assert "mundane-probe-control" in listing and str(app_pid) not in listing, listing
    control.terminate()
    control.wait(timeout=4)


def compare_screenshot(name, scale):
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
    baseline = HERE / name
    if not baseline.exists():
        raise RuntimeError(f"Review {image} and supply {name}")
    run("magick", "compare", "-metric", "AE", str(baseline), str(image), "null:")


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


def exercise_files(folder, scale, profile, labwc):
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
    key("v", "ctrl")
    name = f"files-{'labwc' if labwc else 'sway'}-{profile}.png"
    compare_screenshot(name, scale)
    key("Escape")
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--app", choices=["terminal", "files"], default="terminal")
    parser.add_argument("--at-spi", action="store_true")
    parser.add_argument("--profile", choices=["standard", "fractional", "contrast"], default="standard")
    parser.add_argument("--compositor", choices=["sway", "labwc"], default="sway")
    args = parser.parse_args()
    labwc = args.compositor == "labwc"
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
        if args.at_spi:
            address = f"unix:path={RUNTIME}/at-spi"
            os.environ["AT_SPI_BUS_ADDRESS"] = address
            launch(stack, "dbus-daemon", "--nofork", "--config-file=/usr/share/defaults/at-spi2/accessibility.conf",
                   f"--address={address}", log="at-spi-bus.log")
            until(lambda: (RUNTIME / "at-spi").is_socket(), "private AT-SPI bus")
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
        app_id = f"org.joelinux.mundane.{suite_app.title()}"
        argv = [str(Path(os.environ.get(f"MUNDANE_{suite_app.upper()}_BIN", ROOT / f"target/debug/mundane-{suite_app}")).resolve())]
        if suite_app == "files":
            os.environ["GSETTINGS_BACKEND"] = "keyfile"
            run("gsettings", "set", "org.gnome.desktop.interface", "icon-theme", "Papirus")
            folder = create_files_fixture()
            if labwc:
                launch(stack, "python3", str(HERE / "open_portal.py"), log=f"portal-{args.profile}.log")
                until((RUNTIME / "portal-ready").exists, "private OpenURI portal")
            argv.append(str(folder))
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
        if suite_app == "files":
            resize(720, 440)
            exercise_files(folder, scale, args.profile, labwc)
            if args.at_spi:
                key("l", "ctrl")  # Inspect with the location field and dialog focused.
                inspect_accessibility(stack, app_id, app.pid)
                command(f'[app_id="{app_id}"] focus')
                key("Escape")
            if labwc:
                run("wlrctl", "toplevel", "close", app_id)
            else:
                command(f'[app_id="{app_id}"] kill')
            assert app.wait(timeout=8) == 0
            return
        until(lambda: (Path.home() / ".prompt-ready").exists(), "interactive shell prompt")
        resize(640, 360)
        key("Shift_L")
        if contrast:
            key("equal", "ctrl")
        # Wait for a fresh prompt and the resized kernel PTY before exercising the renderer.
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
            key("Escape")
        if labwc:
            check_window_actions(OUTPUT / log)
        else:
            type_text("exit")
            key("Return")
        assert app.wait(timeout=8) == 0
        print(f"Passed {name}, PTY {reported}")


if __name__ == "__main__":
    main()
