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


def speech_track(folder):
    import array
    import math
    import wave
    path = folder / "Voice notes.wav"
    samples = array.array("h")
    for i in range(16000 * 12):
        t = i / 16000
        envelope = (0.09 + 0.8 * abs(math.sin(t * 1.3)) ** 2) if t % 3.3 < 2.8 else 0
        samples.append(round(21000 * envelope * math.sin(t * 440 * math.tau)))
    with wave.open(str(path), "wb") as wav:
        wav.setparams((1, 2, 16000, 0, "NONE", ""))
        wav.writeframes(samples.tobytes())
    return path


def music(side, root, log, key, until, compare, record, check_tone):
    until(lambda: "Voice notes.wav — Music" in log.read_text(), "voice notes loaded")
    key("Shift_L")
    key("space")
    key("Home")
    key("Right")
    key("bracketright")
    key("bracketright")
    compare("music-waveform.png", 1, root / "docs/parity/round-4" / side)
    if side == "after":
        capture = record("speech-speed")
        key("space")
        check_tone(capture, until)

        for name, hz in [("Radio.mp3", 550), ("Interview.m4a", 660)]:
            if name.endswith(".m4a") and not (Path(os.environ["GST_PLUGIN_PATH_1_0"]) / "libgstlibav.so").exists():
                continue
            (Path(os.environ["XDG_RUNTIME_DIR"]) / "OpenFile").unlink(missing_ok=True)
            key("o", "ctrl")
            until((Path(os.environ["XDG_RUNTIME_DIR"]) / "OpenFile").exists, "append " + name)
            key("Escape")
            capture = record(name)
            key("n")
            until(lambda: name + " — Music" in log.read_text(), name)
            check_tone(capture, until, hz)


def calculator(side, root, key, type_text, compare, clipboard):
    key("Shift_L")
    type_text("120 + 15%")
    key("Return")
    compare("calculator-percent.png", 1, root / "docs/parity/round-4" / side)
    key("l", "ctrl")
    type_text("3 + 2")
    key("u", "ctrl")
    if side == "after":
        type_text("mi to km")
        key("Down")
        key("Return")
    compare("calculator-units.png", 1, root / "docs/parity/round-4" / side)
    if side == "after":
        key("r", "ctrl")
        key("a", "ctrl")
        key("c", "ctrl")
        assert clipboard() == "8.04672"


def clocks(side, app, root, key, type_text, until, compare):
    key("Shift_L")
    if side == "after":
        key("a", "ctrl")
        key("n", "ctrl")
        type_text("Morning walk")
        key("Tab")
        key("a", "ctrl")
        type_text("07:30")
        key("Tab")
        key("Return")  # Monday
        key("Tab")
        key("Return")  # Tuesday
        key("Tab", "shift")
        key("Tab", "shift")
        key("Return")  # Save from the time field
        path = Path(os.environ["XDG_CONFIG_HOME"]) / "mundane-clocks/alarms.json"
        until(path.exists, "saved alarm")
        alarm, = json.loads(path.read_text())
        assert (alarm["name"], alarm["hour"], alarm["minute"], alarm["days"]) == ("Morning walk", 7, 30, 3)
    compare("clocks-alarms.png", 1, root / "docs/parity/round-4" / side)
    if side == "after":
        key("w", "ctrl")
        assert app.wait(timeout=5) == 0
        alarm["next"] = 1
        path.write_text(json.dumps([alarm]))
        binary = os.environ.get("MUNDANE_CLOCKS_BIN", root / "target/debug/mundane-clocks")
        worker = subprocess.Popen([binary, "--watch-alarms"])
        try:
            sent = Path(os.environ["XDG_RUNTIME_DIR"]) / "alarm-notified"
            until(lambda: sent.exists() and sent.read_text() == "Morning walk", "notification with Clocks closed")
            assert json.loads(path.read_text())[0]["ringing"]
        finally:
            worker.terminate()
            worker.wait(timeout=5)


def weather(side, root, log, key, type_text, until, compare):
    key("Shift_L")
    for i,name in enumerate(["Oslo","London"]):
        if i:
            key("l", "ctrl")
        type_text(name)
        key("Return")
        until(lambda: name + " — Weather" in log.read_text(), "forecast for " + name)
    key("d", "ctrl")
    compare("weather-daily.png", 1, root / "docs/parity/round-4" / side)
    key("l", "ctrl")
    compare("weather-places.png", 1, root / "docs/parity/round-4" / side)
    if side == "after":
        runtime = Path(os.environ["XDG_RUNTIME_DIR"])
        (runtime/"location-deny").touch()
        key("g", "ctrl")
        until((runtime/"location-closed").exists, "closed refused location session")
        assert "Current location — Weather" not in log.read_text()
        (runtime/"location-deny").unlink()
        key("g", "ctrl")
        until(lambda: "Current location — Weather" in log.read_text(), "permitted location forecast")


def maps(side,root,log,key,type_text,until,compare):
    key("Shift_L")
    key("f","ctrl")
    type_text("quay")
    key("Return")
    until(lambda:"Place results — Maps" in log.read_text(),"place results")
    key("Down")
    key("Return")
    until(lambda:"Harbour Point · Zoom" in log.read_text(),"selected place")
    key("d","ctrl")
    key("b","ctrl")
    compare("maps-favourites.png",1,root/"docs/parity/round-4"/side)
    key("Escape")
    if side=="after":
        runtime=Path(os.environ["XDG_RUNTIME_DIR"])
        (runtime/"location-deny").touch()
        key("g","ctrl")
        until((runtime/"location-closed").exists,"denied location request closed")
        assert "Current location · Zoom" not in log.read_text()
        (runtime/"location-deny").unlink()
        key("g","ctrl")
        until(lambda:"Current location · Zoom" in log.read_text(),"permitted current location")
    compare("maps-location.png",1,root/"docs/parity/round-4"/side)
    if side=="after":
        key("b","ctrl")
        type_text("Harbour")
        key("Down")
        key("Return")
        until(lambda:log.read_text().count("Harbour Point · Zoom")>=2,"reopened favourite")
        assert sum("/search?" in line for line in (runtime/"requests").read_text().splitlines())==1


def characters(side,root,key,type_text,until,compare,clipboard):
    key("Shift_L")
    if side=="after":
        key("b","ctrl")
        key("End")
        key("Up")
        key("Return")
    compare("characters-category.png",1,root/"docs/parity/round-4"/side)
    key("f","ctrl")
    type_text("woman technologist")
    key("Down")
    key("c","ctrl")
    if side=="after":
        until(lambda:clipboard()=="👩‍💻","copied complete emoji sequence")
        key("r","ctrl")
    compare("characters-recent.png",1,root/"docs/parity/round-4"/side)
