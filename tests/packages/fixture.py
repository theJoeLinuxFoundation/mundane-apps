#!/usr/bin/env python3
"""Recording substitutes for the two executables, confined to a test directory."""
import hashlib
import json
import sys
from pathlib import Path

program = Path(sys.argv[0])
work = program.parent
args = sys.argv[1:]
with (work / "calls.jsonl").open("a") as calls:
    print(json.dumps([program.name, *args]), file=calls)

def answer(kind, **fields):
    print(json.dumps({"schema": f"dave.{kind}/1", **fields}))
    sys.exit(int(kind == "error"))

def flag(name):
    return (work / name).exists()

def version():
    return "2.1" if flag("moved") else "2.0"

def change(name, ver, named=False, old=None):
    return dict(name=name, version=ver, named=named, requested=named, **{"from": old})

def plan(op):
    add, upgrade, remove = [], [], []
    if op == "install" and not flag("installed"):
        add = [change("chartroom", version(), True), change("libtide", "1.4")]
        upgrade = [change("coast-data", "3.1", old="3.0")]
    elif op == "remove" and flag("installed"):
        remove = [change("chartroom", version(), True)]
    elif op == "upgrade" and not flag("prepared"):
        upgrade = [change("coast-data", "3.1", old="3.0")]
    changes = [add, upgrade, remove]
    digest = "sha256:" + hashlib.sha256(json.dumps(changes).encode()).hexdigest() if any(changes) else None
    return dict(operation=op, index="cached", digest=digest, install=add, upgrade=upgrade,
                remove=remove, download_size=180224 if add or upgrade else 0,
                disk_change=-524288 if remove else 524288, prepared=None)

def prepared():
    return dict(summary="Coastal data 3.1", install=1, remove=0) if flag("prepared") else None

if program.name == "pkexec":
    assert args[0] == "/usr/bin/dave" and args[2:5] == ["-y", "--json", "--expect"]
    op = args[1]
    assert op in ("install","remove","upgrade")
    assert args[6:] == ([] if op == "upgrade" else ["chartroom"])
    mode = (work / "mode").read_text().strip() if flag("mode") else "success"
    if mode in ("dismiss", "denied"):
        sys.exit(126 if mode == "dismiss" else 127)
    if mode == "moved":
        (work / "moved").touch()
    digest = plan(op)["digest"]
    if args[5] != digest or mode == "refused":
        answer("error", message="The requested change was refused.")
    with (work / "applied").open("a") as outcomes:
        print(op, file=outcomes)
    if op == "remove":
        (work / "installed").unlink()
    else:
        (work / ("prepared" if op == "upgrade" else "installed")).touch()
    answer("result", operation=op, digest=digest, rollback_point=12)

op = args[0]
if op == "list":
    assert args == ["list","--json"]
    packages = [dict(name="coast-data", version="3.0", requested=True)]
    if flag("installed"):
        packages += [dict(name="chartroom", version=version(), requested=True),
                     dict(name="libtide", version="1.4", requested=False)]
    answer("list", packages=packages)
if op == "search":
    assert args[1:3] == ["--json","--"]
    packages = [dict(name="chartroom", version=version(), description="Coastal chart reader",
                     installed=version() if flag("installed") else None)] if "chart" in " ".join(args[3:]) else []
    answer("search", index="cached", packages=packages)
if op == "info":
    assert args[1] == "--json"
    name = args[2]
    details = dict(version=version(),reason="requested",needed_by=None,size=524288,files=8)
    answer("info", name=name, description="Coastal chart reader", license="MIT",
           installed=details if flag("installed") else None, depends=["libtide", "coast-data"], required_by=[],
           available=dict(version=version(),repository="coast",download_size=131072,installed_size=524288))
if op == "status":
    assert args == ["status","--json"]
    answer("status", revision=str(flag("prepared")), managed=True, complete=True, running=12, next_boot=12,
           deployments=[dict(id=12, running=True, next_boot=True, health="good", pinned=False, reason="Installation")],
           prepared_upgrade=prepared(), last_upgrade=None)
assert op in ("install","remove","upgrade")
assert args[1:] == ["--dry-run","--json"] + ([] if op == "upgrade" else ["chartroom"])
answer("plan", **plan(op))
