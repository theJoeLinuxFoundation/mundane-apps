"""Private PipeWire playback and loopback recording; the policy profile has no hardware monitors."""
from array import array
import os
from pathlib import Path
import signal
import subprocess
import wave


def start(stack, runtime, launch, until):
    os.environ.update(PIPEWIRE_RUNTIME_DIR=str(runtime), PIPEWIRE_REMOTE="pipewire-0",
                      PULSE_SERVER=f"unix:{runtime}/no-pulse")
    conf = Path(os.environ["XDG_CONFIG_HOME"]) / "pipewire/pipewire.conf.d"
    conf.mkdir(parents=True)
    (conf / "test-output.conf").write_text("""context.objects = [
      { factory = adapter args = { factory.name = support.null-audio-sink
        node.name = mundane-test-output media.class = Audio/Sink
        audio.position = [ FL FR ] } }
    ]""")
    launch(stack, "pipewire", log="video-pipewire.log")
    until(lambda: (runtime / "pipewire-0").is_socket(), "private PipeWire socket")
    launch(stack, "wireplumber", "--profile", "policy", log="video-policy.log")
    def ready():
        result = subprocess.run(["wpctl", "inspect", "@DEFAULT_AUDIO_SINK@"], capture_output=True, text=True)
        return result.returncode == 0 and "mundane-test-output" in result.stdout
    until(ready, "private default null sink")


def record(stack, runtime, launch, tag):
    path = runtime / f"{tag}.wav"
    process = launch(stack, "pw-record", "--target", "mundane-test-output", "--properties",
                     "{ stream.capture.sink = true }", "--rate", "48000", "--channels", "1",
                     "--format", "s16", str(path), log=f"video-record-{tag}.log")
    return process, path


def check_tone(recording, until, frequency=440):
    process, path = recording
    def detected():
        if not path.exists():
            return False
        # The WAV length is finalized on exit. Check accumulated PCM first.
        raw = path.read_bytes()
        start = raw.find(b"data")
        if start < 0:
            return False
        samples = array("h")
        pcm = raw[start+8:]
        samples.frombytes(pcm[:len(pcm)//2*2])
        for i in range(0, len(samples)-4800, 4800):
            chunk = samples[i:i+4800]
            crossings = sum(a <= 0 < b for a,b in zip(chunk, chunk[1:]))
            if abs(crossings - frequency / 10) <= 2 and max(chunk)-min(chunk)>1500:
                return True
        return False
    until(detected, f"recorded {frequency} Hz tone on private PipeWire output")
    process.send_signal(signal.SIGINT)
    process.wait(timeout=4)
    with wave.open(str(path)) as wav:
        assert wav.getframerate() == 48000 and wav.getnchannels() == 1
