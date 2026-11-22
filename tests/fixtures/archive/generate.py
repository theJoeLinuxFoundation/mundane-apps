"""Original, deterministic archive fixtures."""
from pathlib import Path
import io
import tarfile
import zipfile

ROOT = Path(__file__).resolve().parent
entries = {"Charts/channel.txt": b"East channel\n", "Charts/tides.csv": b"hour,height\n6,1.8\n",
           "Notes/stations.txt": b"North Point\n", "Read me.txt": b"Coastal field survey\n"}
with zipfile.ZipFile(ROOT / "Coastal survey.zip", "w") as out:
    for name, data in entries.items():
        out.writestr(zipfile.ZipInfo(name), data, compress_type=zipfile.ZIP_DEFLATED)

def tar(name, members):
    with tarfile.open(ROOT / name, "w", format=tarfile.USTAR_FORMAT) as out:
        for path, kind, target in members:
            entry = tarfile.TarInfo(path)
            entry.type = kind
            data = b"East channel\n" if kind == tarfile.REGTYPE else b""
            entry.size = len(data)
            entry.linkname = target
            out.addfile(entry, io.BytesIO(data))

tar("Survey links.tar", [("Charts/channel.txt", tarfile.REGTYPE, ""),
                         ("Notes/current", tarfile.SYMTYPE, "../Charts/channel.txt"),
                         ("copy", tarfile.LNKTYPE, "Charts/channel.txt")])

tar("Outside path.tar", [("../outside", tarfile.REGTYPE, "")])
tar("Outside link.tar", [("Charts/link", tarfile.SYMTYPE, "../../outside")])
tar("Outside hardlink.tar", [("copy", tarfile.LNKTYPE, "../outside")])
