"""User-visible parity workflows, with a separate reference set for each revision."""
from pathlib import Path
import os


def exercise(app, side, key, type_text, until, compare, root, log, restart):
    after = side == "after"
    def shot(name):
        filename = app + "-" + name + ".png"
        current = root / "tests/wayland/parity/round-2"
        refs = current if after and (current / filename).exists() else root / "docs/parity/round-2" / side
        compare(filename, 1, refs)
    key("Shift_L")
    if app == "text-editor":
        until(lambda: "survey.py — Text Editor" in log.read_text(), "opened Python file")
        shot("syntax")
        key("p", "ctrl")
        shot("recent")
        key("Escape")
        key("h" if after else "f", "ctrl")
        type_text("station")
        if after:
            key("Tab")
            type_text("location")
        shot("replace")
        key("Escape")
        key("End", "ctrl")
        type_text("# Unsaved observations")
        if after:
            state = Path(os.environ["XDG_STATE_HOME"]) / "mundane-text-editor"
            until(lambda: any("Unsaved observations" in f.read_text() for f in state.glob("draft-*.json")), "autosaved draft")
        restart()
        if after:
            until(lambda: "• survey.py — Text Editor" in (log.parent/"recovered.log").read_text(), "recovered document")
        else:
            until(lambda: "Untitled — Text Editor" in (log.parent/"recovered.log").read_text(), "empty document")
        assert b"Unsaved observations" not in (Path.home()/"survey.py").read_bytes()
        shot("recovery")
    elif app == "terminal":
        until(lambda: (Path.home()/".prompt-ready").exists(), "shell prompt")
        if after:
            key("p","ctrl","shift")
        shot("profiles")
        key("Escape")
        type_text("sleep 120")
        key("Return")
        if after:
            key("w","ctrl","shift")
        shot("running-job")
        if after:
            key("Escape")
            key("c","ctrl")
            type_text("touch survived")
            key("Return")
            until(lambda: (Path.home()/"survived").exists(), "job warning cancelled")
    elif app == "files":
        until(lambda: list((Path(os.environ["XDG_CACHE_HOME"])/"thumbnails/normal").glob("*.png")), "folder ready")
        if after:
            key("t","ctrl")
            key("Return")
        shot("tabs")
        if after:
            key("Page_Up","ctrl")
            key("w","ctrl")
    else:
        raise ValueError(app)
