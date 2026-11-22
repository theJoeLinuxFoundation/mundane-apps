"""Generate short original MP3 and AAC clips with system GStreamer encoders."""
from pathlib import Path
import subprocess


def generate(folder):
    for name, hz, chain in [("Radio.mp3", 550, ["lamemp3enc", "!", "id3v2mux"]),
                            ("Interview.m4a", 660, ["avenc_aac", "!", "mp4mux"])]:
        subprocess.run(["gst-launch-1.0", "-q", "audiotestsrc", f"freq={hz}", "volume=0.18",
                        "num-buffers=100", "samplesperbuffer=4800", "!", "audio/x-raw,rate=48000,channels=1",
                        "!", "audioconvert", "!", *chain, "!", "filesink", f"location={folder / name}"],
                       check=True)


if __name__ == "__main__":
    generate(Path(__file__).resolve().parent)
