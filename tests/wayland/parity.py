"""Daily-use scenarios on the private Wayland compositor."""
from pathlib import Path
import os
import time

def exercise(app, before, folder, key, type_text, until, compare, root, log):
    def shot(state):
        compare(f"{app}-{state}.png", 1, root / "docs/parity" / ("before" if before else "after"))
    key("Shift_L")
    if app == "files":
        thumbs = Path(os.environ["XDG_CACHE_HOME"]) / "thumbnails/normal"
        until(lambda: list(thumbs.glob("*.png")), "thumbnail")
        shot("places")
        if not before:
            key("f", "ctrl")
            type_text("notes")
            key("Return")
            time.sleep(0.4)
        shot("search")
        if not before:
            key("Escape")
            key("d", "ctrl")
            bookmarks = Path(os.environ["XDG_CONFIG_HOME"]) / "gtk-3.0/bookmarks"
            until(lambda: bookmarks.is_file() and folder.as_uri() in bookmarks.read_text().splitlines(), "saved bookmark")
    elif app == "terminal":
        until(lambda: (Path.home() / ".prompt-ready").exists(), "shell")
        type_text("cd Documents && touch .entered")
        key("Return")
        until(lambda: (Path.home() / "Documents/.entered").exists(), "first shell directory")
        if not before:
            key("t", "ctrl", "shift")
        type_text("clear; echo https://example.org/notes; pwd > workdir")
        key("Return")
        until(lambda: (Path.home()/"Documents/workdir").exists(), "inherited working directory")
        shot("shells")
        if not before:
            type_text("exit"); key("Return")
            type_text("echo original > parent"); key("Return")
            until(lambda: (Path.home()/"Documents/parent").exists(), "original shell")
    elif app == "text-editor":
        until(lambda: "Field notes.txt — Text Editor" in log.read_text(), "first document")
        if not before:
            key("o", "ctrl")
            until(lambda: "Second.txt — Text Editor" in log.read_text(), "second document")
            key("Page_Up", "ctrl")
            key("i", "ctrl")
            type_text("4")
            shot("go-to-line")
            key("Return")
        if before:
            shot("go-to-line")
        shot("documents")
    else:
        raise ValueError(app)
