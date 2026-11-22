"""Regenerate the original WebM test clips with GStreamer's Base and Good encoders."""
from pathlib import Path
import math
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
W, H, FPS = 400, 224, 10


def frame(t):
    pixels = bytearray(W * H * 3)
    def span(y, left, right, color):
        left, right = max(0, math.ceil(left)), min(W, math.ceil(right))
        if 0 <= y < H and left < right:
            pixels[(y*W+left)*3:(y*W+right)*3] = bytes.fromhex(color) * (right-left)

    def polygon(points, color):
        for y in range(min(p[1] for p in points), max(p[1] for p in points)):
            edges = sorted(x+(y+.5-y0)*(x1-x)/(y1-y0) for (x,y0),(x1,y1) in
                           zip(points, points[1:]+points[:1]) if min(y0,y1) <= y+.5 < max(y0,y1))
            for a,b in zip(edges[::2],edges[1::2]):
                span(y,a,b,color)

    for y in range(H):
        span(y,0,W,"e5cfab" if y<128 else "668e88")
    for y in range(30,83):
        dx=math.sqrt(max(0,26**2-(y-56)**2))
        span(y,280-dx,280+dx,"fff1cb")
    polygon([(0,111),(45,95),(72,99),(113,80),(161,99),(190,103),(225,91),
             (257,95),(289,108),(330,101),(370,108),(400,102),(400,131),(0,131)],"a2aa95")
    polygon([(0,107),(30,104),(52,113),(81,116),(96,124),(133,130),(156,135),(0,139)],"48655c")
    polygon([(0,126),(36,122),(53,129),(80,135),(0,142)],"304d47")
    for j in range(28):
        y=145+(j*17)%78
        x=(j*57-int(t*2))%W
        span(y,x,x+7+(j*9)%43,"83a599")
    x=int(100+t*10)
    polygon([(x-42,176),(x+38,176),(x+29,184),(x-31,184)],"2d423d")
    polygon([(x-4,173),(x-4,104),(x-36,173)],"b95d40")
    polygon([(x,109),(x,173),(x+30,173)],"f5e9ca")
    polygon([(x-3,103),(x-1,102),(x-1,177),(x-3,177)],"4b5145")
    span(187,x-48,x+36,"527c77")
    return pixels


def encode(output, count, video, audio=None):
    with tempfile.TemporaryDirectory(dir=HERE.parents[1]/"target") as tmp:
        raw=Path(tmp)/"frames.rgb"
        with raw.open("wb") as f:
            for i in range(count):
                f.write(frame(i/FPS))
        args=["gst-launch-1.0","-q","webmmux","name=m","!","filesink",f"location={output}",
              "filesrc",f"location={raw}","!","rawvideoparse","format=rgb",f"width={W}",
              f"height={H}",f"framerate={FPS}/1","!","videoconvert","!","video/x-raw,format=I420",
              "!",video,"deadline=1","keyframe-max-dist=10","!","queue","!","m.video_0"]
        if audio:
            args.extend(["audiotestsrc","wave=sine","freq=440","volume=0.12",f"num-buffers={count}",
                         "samplesperbuffer=4800","!","audio/x-raw,rate=48000,channels=1",
                         "!","audioconvert","!",audio,"!","queue","!","m.audio_0"])
        subprocess.run(args,check=True)


if __name__=="__main__":
    encode(HERE/"Coastal passage.webm",120,"vp8enc","opusenc")
    encode(HERE/"Harbour.webm",24,"vp9enc","vorbisenc")
    encode(HERE/"Silent passage.webm",80,"vp8enc")
