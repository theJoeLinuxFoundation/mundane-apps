"""The representative content each packaged handler must expose after opening."""
from configparser import ConfigParser
from pathlib import Path
import shutil

# Fixture paths relative to tests/. The folder and text document are generated.
FILES = {
    "archive-manager": ("fixtures/archive/Coastal survey.zip", "Read me.txt"),
    "document-viewer": ("fixtures/Coastal survey.pdf", "Survey overview"),
    "files": ("Smoke folder", "Dispatch.txt"),
    "fonts": ("fonts/Survey.ttc", "Preview in Survey Fixture"),
    "image-viewer": ("fixtures/North Point.jpg", "North Point.jpg"),
    "music": ("fixtures/music/02.opus", "Harbour Lights"),
    "text-editor": ("Dispatch.txt", "Coastal smoke document"),
    "video": ("fixtures/Coastal passage.webm", "Coastal passage.webm"),
    "disk-usage": ("Smoke folder", "Dispatch.txt"),
}


def cases(root):
    result = []
    for entry in sorted(root.glob("apps/*/data/*.desktop")):
        config = ConfigParser(interpolation=None)
        config.read(entry)
        data = config["Desktop Entry"]
        slug = entry.parents[1].name
        case = {"app": slug, "desktop": entry.name, "binary": data["Exec"].split()[0],
                "mimes": data.get("MimeType", "").strip(";").split(";") if "MimeType" in data else []}
        result.append({**case, "mode": "desktop"})
        if slug in FILES:
            source, evidence = FILES[slug]
            result.append({**case, "mode": "mime" if case["mimes"] else "folder",
                           "file": Path(source).name, "evidence": evidence})
        elif case["mimes"] or "%" in data["Exec"]:
            raise ValueError(f"Missing fixture for {entry}")
    if len({c["app"] for c in result}) != 18:
        raise ValueError("Expected all eighteen suite desktop entries")
    return result


def stage(root, share):
    share.mkdir()
    folder = share / "Smoke folder"
    folder.mkdir()
    (folder / "Dispatch.txt").write_text("Coastal smoke document\n")
    shutil.copyfile(folder / "Dispatch.txt", share / "Dispatch.txt")
    for source, _ in FILES.values():
        if "/" in source:
            shutil.copyfile(root / "tests" / source, share / Path(source).name)
    for name in ("guest.py", "packaging_checks.py"):
        shutil.copyfile(Path(__file__).with_name(name), share / name)
