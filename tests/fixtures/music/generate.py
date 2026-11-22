"""Generate original mono test tones with the GStreamer Base and Good encoders."""
from pathlib import Path
import gi
gi.require_version("Gst", "1.0")
from gi.repository import Gst
Gst.init(None)
import wave
import math
from array import array

HERE = Path(__file__).resolve().parent

def track(file, seconds, hz, encoder, title):
    pipeline = Gst.parse_launch(f"audiotestsrc volume=0.12 freq={hz} num-buffers={seconds*10} "
                                f"samplesperbuffer=4800 ! audio/x-raw,rate=48000,channels=1 ! "
                                f"audioconvert ! {encoder.replace('enc', 'enc name=encoder', 1)} ! filesink name=sink")
    pipeline.get_by_name("sink").set_property("location", str(HERE/file))
    tags = Gst.TagList.new_empty()
    for name,value in [("title",title), ("artist","Soundings"), ("album","Coastal Sessions")]:
        tags.add_value(Gst.TagMergeMode.REPLACE, name, value)
    pipeline.get_by_name("encoder").merge_tags(tags, Gst.TagMergeMode.REPLACE)
    try:
        pipeline.set_state(Gst.State.PLAYING)
        msg = pipeline.get_bus().timed_pop_filtered(30*Gst.SECOND, Gst.MessageType.ERROR | Gst.MessageType.EOS)
        assert msg and msg.type == Gst.MessageType.EOS, msg.parse_error() if msg else "timeout"
    finally:
        pipeline.set_state(Gst.State.NULL)

if __name__ == "__main__":
    track("01.flac", 18, 330, "flacenc", "Ebb Tide")
    track("02.opus", 16, 440, "opusenc ! oggmux", "Harbour Lights")
    track("03.ogg", 12, 550, "vorbisenc ! oggmux", "North Channel")
    with wave.open(str(HERE/"Field recording.wav"), "wb") as f:
        f.setparams((1, 2, 16000, 0, "NONE", ""))
        f.writeframes(array("h", (int(4000*math.sin(2*math.pi*660*i/16000)) for i in range(32000))).tobytes())
