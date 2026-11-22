"""Unprivileged packaged-session checks, run inside the disposable guest."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time


def seed_alarm():
    config = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    path = config / "mundane-clocks/alarms.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        tomorrow = int(time.time()) + 86400
        local = time.localtime(tomorrow)
        with path.open("x") as f:
            json.dump([dict(id=1, name="VM login fixture", hour=local.tm_hour, minute=local.tm_min,
                            days=0, next=tomorrow, snoozed=None, ringing=False)], f)
    return str(path)


def watcher():
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        for proc in Path("/proc").glob("[0-9]*"):
            try:
                if proc.stat().st_uid != os.getuid() or (proc / "exe").resolve() != Path("/usr/bin/mundane-clocks"):
                    continue
                if (proc / "cmdline").read_bytes().split(b"\0")[1:] == [b"--watch-alarms", b""]:
                    return {"pid": int(proc.name), "uid": os.getuid(),
                            "cgroup": (proc / "cgroup").read_text().strip()}
            except (FileNotFoundError, ProcessLookupError, PermissionError):
                continue
        time.sleep(.25)
    return None


def mime_defaults(types):
    result = {}
    for desktop in ("mundane", "GNOME", "unset"):
        env = {**os.environ, "LC_ALL": "C"}
        if desktop == "unset":
            env.pop("XDG_CURRENT_DESKTOP", None)
        else:
            env["XDG_CURRENT_DESKTOP"] = desktop
        result[desktop] = {}
        for mime in types:
            text = subprocess.check_output(["gio", "mime", mime], env=env, text=True)
            first = text.splitlines()[0]
            result[desktop][mime] = first.split(": ", 1)[1] if first.startswith("Default application for ") else None
    return result


def dave(*args):
    return json.loads(subprocess.check_output(["/usr/bin/dave", *args], timeout=20))


def installed_state():
    entries = dave("list", "--json")
    if entries["schema"] != "dave.list/1":
        raise ValueError("Unexpected installed package schema")
    ordered = sorted(entries["packages"], key=lambda p: p["name"])
    digest = hashlib.sha256(json.dumps(ordered, sort_keys=True).encode()).hexdigest()
    return ordered, digest


def review(visible):
    installed, inventory = installed_state()
    versions = {p["name"]: p["version"] for p in installed}
    if not visible or any(name not in versions for name in visible):
        raise RuntimeError("Installed view disagrees with dave")
    # Pick a leaf package with a single change. Only use dave's unprivileged preview.
    for p in installed:
        if not p["requested"]:
            continue
        trial = subprocess.run(["dave", "remove", "--dry-run", "--json", p["name"]],
                               capture_output=True, text=True, timeout=15)
        if trial.returncode:
            continue
        plan = json.loads(trial.stdout)
        if (plan["schema"] == "dave.plan/1" and plan["operation"] == "remove"
                and re.fullmatch("sha256:[0-9a-f]{64}", plan["digest"] or "")
                and not plan["install"] and not plan["upgrade"] and len(plan["remove"]) == 1
                and plan["remove"][0]["name"] == p["name"] and plan["remove"][0]["named"]):
            break
    else:
        raise RuntimeError("No installed package had a one-package removal preview")
    name = p["name"]
    return {"installed_count": len(installed), "visible_installed": visible,
            "installed_sha256": inventory, "plan": plan,
            "displayed_change": f"Remove {name} · {versions[name]} · Selected"}
