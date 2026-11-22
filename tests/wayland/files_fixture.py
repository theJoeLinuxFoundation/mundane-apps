"""Create original thumbnail and icon fixtures, and a private preferred handler."""
import os
from pathlib import Path
import struct
import zlib


def png(path, width, height, pixel):
    def chunk(name, data):
        return struct.pack(">I", len(data)) + name + data + struct.pack(">I", zlib.crc32(name + data))
    pixels = b"".join(b"\0" + bytes(c for x in range(width) for c in pixel(x, y)) for y in range(height))
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">2I5B", width, height, 8, 6, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b""))


def create():
    data = Path(os.environ["XDG_DATA_HOME"])
    apps = data / "applications"
    apps.mkdir(parents=True)
    handler = Path.home() / "record-open"
    handler.write_text('#!/bin/sh\nprintf "%s" "$1" > "$HOME/opened"\n')
    handler.chmod(0o755)
    (apps / "fixture-text.desktop").write_text(
        f'[Desktop Entry]\nName=Test reader\nType=Application\nExec="{handler}" %f\nMimeType=text/plain;\n')
    (Path(os.environ["XDG_CONFIG_HOME"]) / "mimeapps.list").write_text(
        "[Default Applications]\ntext/plain=fixture-text.desktop;\n")

    # Resolve real icon names with a fixed test theme, independent of distribution artwork.
    theme = data / "icons/Papirus"
    icons = theme / "32x32/mimetypes"
    icons.mkdir(parents=True)
    (theme / "index.theme").write_text("[Icon Theme]\nName=Papirus\nComment=Test fixtures\nDirectories=32x32/mimetypes\n"
                                      "[32x32/mimetypes]\nSize=32\nType=Fixed\nContext=MimeTypes\n")
    png(icons / "inode-directory.png", 32, 32,
        lambda x, y: (226, 188, 116, 255) if (2 <= x < 30 and 9 <= y < 27) or (2 <= x < 13 and 6 <= y < 9) else (0, 0, 0, 0))
    png(icons / "text-plain.png", 32, 32,
        lambda x, y: (208, 221, 214, 255) if 6 <= x < 26 and 3 <= y < 29 else (0, 0, 0, 0))
    folder = Path.home() / "Documents"
    (folder / "Archive").mkdir(parents=True)
    (folder / "draft.txt").write_text("Field notes\n")
    old = folder / "Archive/notes.txt"
    old.write_text("Earlier survey notes\n")
    os.utime(old, (1736326800, 1736326800))
    os.utime(folder / "draft.txt", (1736503200, 1736503200))
    png(folder / "palette.png", 160, 100,
        lambda x, y: (*[(52, 98, 99), (230, 180, 114), (193, 110, 92)][(x//40 + y//30) % 3], 255))
    return folder
