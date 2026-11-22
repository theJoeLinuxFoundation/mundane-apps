"""User-visible parity workflows, with a separate reference set for each revision."""
from pathlib import Path
import os


def exercise(app, side, key, type_text, until, compare, root, log, restart):
    after = side == "after"
    def shot(name):
        compare(app + "-" + name + ".png", 1, root / "docs/parity/round-2" / side)
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
    else:
        raise ValueError(app)
