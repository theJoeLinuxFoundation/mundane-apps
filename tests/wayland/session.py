"""Run inside the private session created by tests/wayland/run."""
import argparse
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


def inspect_accessibility(stack, terminal_pid):
    # An accessible GTK control on the same bus distinguishes a missing bridge from a broken probe.
    control = launch(stack, "python3", str(HERE / "accessibility.py"), "--control", log="gtk.log")
    until(lambda: control.poll() is None and "AT-SPI control" in output("swaymsg", "-r", "-t", "get_tree"),
          "GTK control window")
    run("swaymsg", "-q", f'[app_id="{APP_ID}"] focus')
    key("Tab")
    key("Tab", "shift")
    run("python3", str(HERE / "accessibility.py"), str(terminal_pid))
    listing = output("orca", "--list-apps")
    (OUTPUT / "orca.txt").write_text(listing + "\n")
    assert "mundane-probe-control" in listing and str(terminal_pid) not in listing, listing
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--at-spi", action="store_true")
    parser.add_argument("--profile", choices=["standard", "fractional", "contrast"], default="standard")
    parser.add_argument("--compositor", choices=["sway", "labwc"], default="sway")
    args = parser.parse_args()
    labwc = args.compositor == "labwc"
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
        log = f"terminal-{args.compositor}.log"
        app = launch(stack, str(Path(os.environ.get("MUNDANE_TERMINAL_BIN", ROOT / "target/debug/mundane-terminal")).resolve()),
                     log=log, env={**os.environ, "WAYLAND_DEBUG": "client"})
        def mapped():
            if app.poll() is not None or compositor.poll() is not None:
                raise RuntimeError(f"Terminal or compositor exited. See {OUTPUT}.")
            if labwc:
                return "ack_configure" in (OUTPUT / log).read_text(errors="replace")
            return APP_ID in command("-t", "get_tree")
        until(mapped, "mapped Terminal")
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
            inspect_accessibility(stack, app.pid)
            command(f'[app_id="{APP_ID}"] focus')
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
