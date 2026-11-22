"""Compare the same user workflows before and after round three on private Sway."""
from pathlib import Path
import json
import subprocess
import os


def exercise(app, side, key, type_text, until, compare, root, log):
    after = side == "after"

    def shot(state):
        filename = f"{app}-{state}.png"
        current = root / "tests/wayland/parity/round-3"
        refs = current if after and (current / filename).exists() else root / "docs/parity/round-3" / side
        compare(filename, 1, refs)

    key("Shift_L")
    if app == "files":
        folder = Path.home() / "Documents"
        until(lambda: any((Path.home().parent / "cache/thumbnails/normal").glob("*.png")), "folder previews")
        key("Down")
        key("Down", "shift")
        shot("selection")
        socket = Path(os.environ["XDG_RUNTIME_DIR"]) / os.environ["WAYLAND_DISPLAY"]
        if after:
            key("c", "ctrl")
            key("Home")
            key("Return")
            key("v", "ctrl")
            until(lambda: all((folder / "Archive" / n).exists() for n in ("draft.txt", "palette.png")), "both selected files copied")
            for name in ("draft.txt", "palette.png"):
                (folder / "Archive" / name).unlink()
            key("BackSpace")
            key("Down")
            key("Down", "shift")
            # Coordinates are logical pixels on this fixture's 800 × 500 output.
            subprocess.run([root / "target/debug/examples/header-pointer", socket, "gesture", "340,194", "press", "355,187", "350,149", "release"], check=True)
            until(lambda: all((folder / "Archive" / n).exists() and not (folder/n).exists()
                              for n in ("draft.txt", "palette.png")), "dragged selection moved to folder")
            for name in ("draft.txt", "palette.png"):
                (folder / "Archive" / name).rename(folder / name)
        # Two independent clients negotiate a real text/uri-list drop.
        inbox = Path.home() / "Inbox"
        inbox.mkdir()
        names = ("alpha.txt", "beta.txt")
        for name in names:
            (folder / name).write_text(name)
        key("F5")
        key("Home")
        key("Down")
        key("Down", "shift")
        subprocess.run(["swaymsg", "-q", "output HEADLESS-1 mode 1600x500"], check=True)
        subprocess.run(["swaymsg", "-q", "splith"], check=True)
        binary = os.environ.get("MUNDANE_FILES_BIN", root / "target/debug/mundane-files")
        receiver = subprocess.Popen([binary, inbox], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            def both_windows():
                tree = subprocess.check_output(["swaymsg", "-r", "-t", "get_tree"], text=True)
                def count(node):
                    return int(node.get("app_id") == "org.joelinux.mundane.Files") + sum(
                        count(child) for child in node["nodes"] + node["floating_nodes"])
                return count(json.loads(tree)) == 2
            until(both_windows, "both Files windows on the private output")
            # The pointer takes fractions of an 800 × 500 space. Each client is 800 px wide.
            subprocess.run([root / "target/debug/examples/header-pointer", socket, "gesture",
                            "170,194", "press", "180,194", "390,194", "408,194",
                            "460,194", "590,260", "605,285", "release"], check=True)
            if after:
                until(lambda: all((inbox/name).is_file() for name in names), "native multi-file drop")
                assert all((inbox/name).read_bytes() == (folder/name).read_bytes() for name in names)
            subprocess.run(["swaymsg", "-q", f"[pid={receiver.pid}] focus"], check=True)
            key("F5")
            shot("drop")
        finally:
            receiver.terminate()
            receiver.wait(timeout=5)
    elif app == "system-monitor":
        runtime = Path(os.environ["XDG_RUNTIME_DIR"])
        until((runtime/"sampled").exists, "repeatable process samples")
        if after:
            key("f", "ctrl")
            type_text("editor")
        shot("filter")
        if after:
            key("r", "ctrl")
            until((runtime/"history-ready").exists, "five resource samples")
        shot("resources")
    elif app == "document-viewer":
        runtime = Path(os.environ["XDG_RUNTIME_DIR"])
        until(lambda: "Field guide.pdf — Document Viewer" in log.read_text(), "PDF open")
        if after:
            key("j", "ctrl")
            key("Down")
            key("Down")
        shot("continuous")
        if after:
            key("b", "ctrl")
            type_text("Tidal")
        shot("outline")
        if after:
            key("Down")
            key("Return")
            key("j", "ctrl")
            key("a", "ctrl")
            key("c", "ctrl")
            until(lambda: "high water" in subprocess.run(["wl-paste","-n"], capture_output=True,text=True).stdout, "copied page two text")
            socket = runtime / os.environ["WAYLAND_DISPLAY"]
            subprocess.run([root / "target/debug/examples/header-pointer", socket, "gesture",
                            "215,168", "press", "233,168", "349,168", "release"], check=True)
            key("c", "ctrl")
            until(lambda: subprocess.run(["wl-paste","-n"],capture_output=True,text=True).stdout.strip()=="Tidal range", "copied mouse selection")
        shot("selection")
        if after:
            key("p", "ctrl")
            until((runtime / "print-input.pdf").exists, "Print portal received the document")
            assert (runtime / "print-input.pdf").read_bytes() == (Path.home() / "Field guide.pdf").read_bytes()
    elif app == "image-viewer":
        def title(name):
            return name + " — Image Viewer" in log.read_text()
        until(lambda: title("01 North Point.jpg"), "first image")
        key("Page_Down")
        if after:
            until(lambda: title("02 North Point.webp"), "next WebP image")
        shot("webp")
        key("Right")
        if after:
            until(lambda: title("03 Harbour.svg"), "next SVG image")
        shot("svg")
        if after:
            key("Page_Up")
            until(lambda: log.read_text().count("02 North Point.webp — Image Viewer") == 2, "previous image")
    elif app == "packages":
        calls = Path(os.environ["XDG_RUNTIME_DIR"]) / "packages/calls.jsonl"
        until(calls.exists, "dave listing")
        if after:
            key("d", "ctrl")
        shot("discover")
        if after:
            key("Tab")  # Office, Science
            key("Return")
            key("Return")  # Chartroom
            key("Tab")
            key("Return")  # Launch via desktop registry
            until((Path.home() / "chartroom-launched").exists, "launched installed application")
            key("Tab")
            key("Tab")
            key("Return")  # Show the supplied screenshot
        shot("application")
    else:
        raise ValueError(app)
