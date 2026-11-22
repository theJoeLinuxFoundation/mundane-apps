"""Compare the same user workflows before and after round three on private Sway."""
from pathlib import Path
import subprocess
import os


def exercise(app, side, key, type_text, until, compare, root, log):
    after = side == "after"

    def shot(state):
        compare(f"{app}-{state}.png", 1, root / "docs/parity/round-3" / side)

    key("Shift_L")
    if app == "files":
        folder = Path.home() / "Documents"
        until(lambda: any((Path.home().parent / "cache/thumbnails/normal").glob("*.png")), "folder previews")
        key("Down")
        key("Down", "shift")
        shot("selection")
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
            socket = Path(os.environ["XDG_RUNTIME_DIR"]) / os.environ["WAYLAND_DISPLAY"]
            subprocess.run([root / "target/debug/examples/header-pointer", socket, "gesture", "340,194", "press", "355,187", "350,149", "release"], check=True)
            until(lambda: all((folder / "Archive" / n).exists() and not (folder/n).exists()
                              for n in ("draft.txt", "palette.png")), "dragged selection moved to folder")
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
