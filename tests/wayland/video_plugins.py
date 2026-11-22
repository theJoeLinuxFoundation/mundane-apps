"""Create a GStreamer registry containing just Video's shipping plugins."""
import os
from pathlib import Path
import subprocess

PLUGINS = ("coreelements", "matroska", "vpx", "opus", "vorbis", "videoconvertscale",
           "app", "audioconvert", "audioresample", "pipewire")


MUSIC_PLUGINS = ("coreelements", "ogg", "opus", "vorbis", "audioparsers", "flac",
                 "wavparse", "audioconvert", "audioresample", "pipewire")

def configure(directory, extra=(), omit=(), plugins=PLUGINS):
    directory.mkdir(parents=True, exist_ok=True)
    system = subprocess.check_output(["pkg-config", "--variable=pluginsdir", "gstreamer-1.0"], text=True).strip()
    roots = [Path(p) for p in os.environ.get("MUNDANE_TEST_GST_PLUGIN_DIRS", system).split(os.pathsep)]
    for name in (*plugins, *extra):
        if name in omit:
            continue
        target = f"libgst{name}.so"
        source = next((root / target for root in roots if (root / target).is_file()), None)
        if source is None:
            raise RuntimeError(f"Missing GStreamer plugin {target} in {roots}")
        link = directory / target
        if not link.exists():
            link.symlink_to(source.resolve())
    os.environ.update(GST_PLUGIN_SYSTEM_PATH_1_0="", GST_PLUGIN_PATH_1_0=str(directory),
                      GST_REGISTRY_1_0=str(directory / "registry.bin"))

def loaded_plugins(pid):
    libraries = set()
    for line in Path(f"/proc/{pid}/maps").read_text().splitlines():
        path = line.split()[-1]
        if "/gstreamer-1.0/libgst" in path:
            libraries.add(Path(path).name.removeprefix("libgst").removesuffix(".so"))
    return libraries
