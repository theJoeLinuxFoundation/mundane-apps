"""Exercise round four on a disposable, software-rendered Wayland session."""
import json
import os
from pathlib import Path
import subprocess

from video_plugins import PLUGINS, MP4_PLUGINS, loaded_plugins


def video(side, app, root, log, key, until, compare, record, check_tone):
    after = side == "after"
    references = root / "docs/parity/round-4" / side

    def fullscreen():
        def active(node):
            if node.get("pid") == app.pid:
                return node["fullscreen_mode"] != 0
            return any(active(n) for n in node["nodes"] + node["floating_nodes"])
        return active(json.loads(subprocess.check_output(["swaymsg", "-r", "-t", "get_tree"])))

    until(lambda: "Coastal passage.webm — Video" in log.read_text(), "WebM title")
    key("Shift_L")
    key("space")
    key("Home")
    key("Right")
    key("F11")
    if after:
        until(fullscreen, "compositor accepted fullscreen")
    compare("video-fullscreen.png", 1, references)
    if after:
        key("F6")
        key("Return")
        until(lambda: not fullscreen(), "keyboard exit button")
        key("F11")
        until(fullscreen, "second fullscreen request")
    key("Escape")
    until(lambda: not fullscreen(), "windowed playback")
    capture = record("h264-opus") if after else None
    key("o", "ctrl")
    until((Path(os.environ["XDG_RUNTIME_DIR"]) / "OpenFile").exists, "MP4 chosen")
    if after:
        until(lambda: "Passage.mp4 — Video" in log.read_text(), "MP4 playback")
        check_tone(capture, until)
        key("space")
        key("Home")
        key("Right")
        assert loaded_plugins(app.pid) == set(PLUGINS + MP4_PLUGINS) - {"vorbis"}
    compare("video-mp4.png", 1, references)

    if after and (Path(os.environ["GST_PLUGIN_PATH_1_0"]) / "libgstlibav.so").exists():
        capture = record("h264-aac")
        key("o", "ctrl")
        until(lambda: "Passage with AAC.mp4 — Video" in log.read_text(), "H.264/AAC playback")
        check_tone(capture, until, frequency=660)
        assert "libav" in loaded_plugins(app.pid)
        print("Passed AAC playback through the system gst-libav plugin")
    elif after:
        print("Skipped AAC playback: system gst-libav plugin absent")
