"""A headless KVM machine, with only its private overlay writable."""
import contextlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import socket
import subprocess
import time
import uuid


class VM:
    def __init__(self, run):
        self.run = run

    def connect(self, name, timeout):
        sock = socket.socket(socket.AF_UNIX)
        try:
            sock.settimeout(timeout)
            sock.connect(str(self.run / name))
            return sock
        except BaseException:
            sock.close()
            raise

    def qmp(self, command, **arguments):
        with self.connect("qmp", 10) as sock, sock.makefile("rwb") as wire:
            if "QMP" not in json.loads(wire.readline()):
                raise RuntimeError("Missing QMP greeting")
            for message in ({"execute": "qmp_capabilities"},
                            {"execute": command, "arguments": arguments}):
                wire.write(json.dumps(message).encode() + b"\n")
                wire.flush()
                while True:
                    reply = json.loads(wire.readline())
                    if "error" in reply:
                        raise RuntimeError(reply["error"])
                    if "return" in reply:
                        break
            return reply["return"]

    def key(self, *keys):
        self.qmp("send-key", keys=[{"type": "qcode", "data": k} for k in keys])

    def shell(self, command, timeout=65):
        token = uuid.uuid4().hex
        # Terminal echo must never satisfy the output delimiter.
        script = (f"printf '\\nBEGIN%s\\n' {token}; sh -c {shlex.quote(command)}; "
                  f"printf '\\nEND%s %d\\n' {token} $?\n")
        end = re.compile(rb"\nEND" + token.encode() + rb" (\d+)\r?\n")
        data = bytearray()
        deadline = time.monotonic() + timeout
        with self.connect("serial", timeout) as sock:
            sock.sendall(script.encode())
            while not (match := end.search(data)):
                left = deadline - time.monotonic()
                if left <= 0:
                    raise TimeoutError("guest serial response")
                sock.settimeout(left)
                chunk = sock.recv(65536)
                if not chunk:
                    raise EOFError("guest serial console closed")
                data.extend(chunk)
        start = data.index(b"\nBEGIN" + token.encode())
        result = data[data.index(b"\n", start + 1) + 1:match.start()].decode(errors="replace")
        if int(match[1]):
            raise RuntimeError(f"Guest exited {match[1].decode()}: {result.strip()}")
        return result.strip()

    def capture(self, filename):
        self.qmp("screendump", filename=str(self.run / filename))


def stamp(image):
    st = image.stat()
    return st.st_dev, st.st_ino, st.st_size, st.st_mtime_ns, st.st_ctime_ns


@contextlib.contextmanager
def boot(image, run, share):
    before = stamp(image)
    env = {"HOME": str(run), "XDG_RUNTIME_DIR": str(run), "TMPDIR": str(run),
           "PATH": os.environ["PATH"], "LC_ALL": "C"}
    def img(*args):
        return subprocess.check_output(["qemu-img", *args], cwd=run, env=env)
    info = json.loads(img("info", "--output=json", str(image)))
    fmt = info["format"]
    if fmt not in ("raw", "qcow2") or "backing-filename" in info or info.get(
        "format-specific", {}).get("data", {}).get("data-file"):
        raise ValueError("Supply a standalone raw or qcow2 desktop disk image")
    img("create", "-q", "-f", "qcow2", "-F", fmt, "-b", str(image), "system.qcow2")
    fw = Path(os.environ.get("OVMF_DIR", "/usr/share/edk2/x64")).resolve()
    shutil.copyfile(fw / "OVMF_VARS.4m.fd", run / "vars.fd")
    args = ["qemu-system-x86_64", "-machine", "q35,accel=kvm", "-cpu", "host",
            "-smp", "4", "-m", "6G", "-nodefaults", "-display", "none",
            "-monitor", "none", "-nic", "none", "-no-reboot",
            "-device", "virtio-vga,xres=1280,yres=800", "-device", "virtio-keyboard-pci",
            "-audiodev", "none,id=silent", "-device", "ich9-intel-hda",
            "-device", "hda-duplex,audiodev=silent",
            "-drive", f"if=pflash,format=raw,readonly=on,file={str(fw).replace(',', ',,')}/OVMF_CODE.4m.fd",
            "-drive", "if=pflash,format=raw,file=vars.fd",
            "-blockdev", json.dumps({"driver": "file", "node-name": "basefile",
                                     "filename": str(image), "read-only": True}),
            "-blockdev", f"driver={fmt},node-name=base,read-only=on,file=basefile",
            "-blockdev", "driver=file,node-name=layerfile,filename=system.qcow2",
            "-blockdev", "driver=qcow2,node-name=layer,file=layerfile,backing=base",
            "-device", "virtio-blk-pci,drive=layer",
            "-virtfs", f"local,path={share.name},mount_tag=smoke,security_model=none,readonly=on",
            "-chardev", "socket,id=tty,path=serial,server=on,wait=off,logfile=serial.log",
            "-serial", "chardev:tty", "-qmp", "unix:qmp,server=on,wait=off"]
    vm = VM(run)
    with (run / "qemu.log").open("wb") as log:
        proc = subprocess.Popen(args, env=env, cwd=run, stdin=subprocess.DEVNULL,
                                stdout=log, stderr=subprocess.STDOUT)
        try:
            for _ in range(100):
                if proc.poll() is not None:
                    raise RuntimeError((run / "qemu.log").read_text())
                try:
                    nodes = {n["node-name"]: n for n in vm.qmp("query-named-block-nodes")}
                    break
                except (FileNotFoundError, ConnectionRefusedError):
                    time.sleep(.1)
            else:
                raise TimeoutError("QMP startup")
            if not nodes["base"]["ro"] or not nodes["basefile"]["ro"] or nodes["layer"]["ro"]:
                raise RuntimeError("Unexpected QEMU block permissions")
            yield vm
        finally:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()
            if before != stamp(image):
                raise RuntimeError("Backing image metadata changed during the smoke run")
