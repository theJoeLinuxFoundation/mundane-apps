#!/usr/bin/env python3
"""Check the VM boundary, fixture matrix and false-positive rejection without a joe image."""
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
import guest
from cases import cases, stage
from machine import boot

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
runner = runpy.run_path(str(HERE / "run"))


class Harness(unittest.TestCase):
    def setUp(self):
        target = ROOT / "target/joe-vm"
        target.mkdir(parents=True, exist_ok=True)
        self.work = Path(self.enterContext(tempfile.TemporaryDirectory(dir=target)))

    def test_matrix_and_fixtures(self):
        plan = cases(ROOT)
        share = self.work / "share"
        stage(ROOT, share)
        self.assertEqual([sum(c["mode"] == mode for c in plan)
                          for mode in ("desktop", "mime", "folder")], [18, 8, 1])
        for c in plan:
            if "file" in c:
                self.assertTrue((share / c["file"]).exists(), c)
        self.assertFalse(any(p.name.startswith("mundane-") for p in share.rglob("*")))

    def test_kvm_qmp_and_overlay(self):
        image = self.work / "original.raw"
        with image.open("wb") as disk:
            disk.truncate(8 << 20)
        with image.open("rb") as disk:
            digest = hashlib.file_digest(disk, "sha256").digest()
        share = self.work / "share"
        share.mkdir()
        with boot(image, self.work, share) as vm:
            self.assertTrue(vm.qmp("query-kvm")["enabled"])
            # The UEFI console paints through virtio VGA with no host display.
            for _ in range(50):
                vm.capture("firmware.ppm")
                size, pixels = runner["samples"](self.work / "firmware.ppm")
                if len(set(pixels)) > 1:
                    break
                time.sleep(.1)
            self.assertGreater(len(set(pixels)), 1)
            vm.key("esc")
        subprocess.run(["qemu-io", "-f", "qcow2", "-c", "write -P 37 0 512",
                        "-c", "read -P 37 0 512", self.work / "system.qcow2"],
                       check=True, stdout=subprocess.DEVNULL)
        with image.open("rb") as disk:
            self.assertEqual(hashlib.file_digest(disk, "sha256").digest(), digest)

    def test_render_gate(self):
        for slug, ref in [("document-viewer", "document"), ("image-viewer", "viewer"),
                          ("video", "video"), ("calculator", "calculator")]:
            with self.subTest(app=slug):
                subprocess.run(["magick", ROOT / f"tests/wayland/{ref}-sway-standard.png",
                                "-depth", "8", self.work / "ref.ppm"], check=True)
                frame = runner["samples"](self.work / "ref.ppm")
                empty = frame[0], [b"\x70\x60\x50"] * len(frame[1])
                blank = frame[0], [b"\x17\x20\x1e"] * len(frame[1])
                case = {"app": slug, "mode": "desktop" if slug == "calculator" else "mime"}
                self.assertTrue(runner["painted"](empty, frame, case))
                self.assertFalse(runner["painted"](frame, frame, case))
                self.assertFalse(runner["painted"](empty, blank, case))
                if slug == "video":
                    self.assertFalse(runner["painted"](empty, saved_document, case))
                if slug == "document-viewer":
                    saved_document = frame

    def test_journal_attribution(self):
        rows = [
            {"COREDUMP_EXE": "/usr/bin/mundane-text-editor", "COREDUMP_PID": "52"},
            {"_COMM": "kernel", "MESSAGE": "mundane-text-edi[77]: segfault at 0"},
            {"SYSLOG_IDENTIFIER": "joe-smoke-text-editor", "MESSAGE": "thread panicked at foo"},
            {"_EXE": "/usr/bin/mundane-text-editor", "MESSAGE": "Loaded document"},
            {"COREDUMP_EXE": "/usr/bin/other", "COREDUMP_PID": "54"},
        ]
        with patch.object(guest.subprocess, "run"), patch.object(
            guest, "output", return_value="\n".join(map(json.dumps, rows))
        ):
            result = guest.journal("cursor", {"app": "text-editor", "binary": "mundane-text-editor",
                                             "desktop": "org.joelinux.mundane.TextEditor.desktop"})
        self.assertEqual(result["crashes"], rows[:3])
        self.assertEqual(result["entries"], rows[:4])


if __name__ == "__main__":
    unittest.main()
