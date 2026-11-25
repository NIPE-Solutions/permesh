#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Exercise manifest generation against synthetic release checksum files."""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import package_manifests as manifests  # noqa: E402

SCRIPT = Path(__file__).with_name("package_manifests.py")
VERSION = "0.1.0-alpha.3"
BASE = f"https://github.com/NIPE-Solutions/permesh/releases/download/v{VERSION}"
TARGETS = {
    "aarch64-apple-darwin": "tar.gz",
    "x86_64-apple-darwin": "tar.gz",
    "aarch64-unknown-linux-gnu": "tar.gz",
    "x86_64-unknown-linux-gnu": "tar.gz",
    "x86_64-pc-windows-msvc": "zip",
}


def digest(index):
    return f"{index:x}" * 64 if index < 10 else ("%064x" % index)


class PackageManifests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.sums = Path(self.temp.name) / "sums"
        self.out = Path(self.temp.name) / "out"
        self.sums.mkdir()
        self.out.mkdir()
        self.expected = {}
        for index, (target, ext) in enumerate(TARGETS.items(), start=1):
            name = f"permesh-{VERSION}-{target}.{ext}"
            self.expected[target] = digest(index)
            (self.sums / f"{name}.sha256").write_text(f"{digest(index)}  {name}\n", encoding="ascii")

    def run_script(self):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--version", VERSION,
             "--checksums", str(self.sums), "--out", str(self.out)],
            text=True, capture_output=True, check=False,
        )

    def test_formula_pins_every_unix_target(self):
        formula = manifests.homebrew_formula(VERSION, manifests.read_checksums(self.sums, VERSION))
        for target, ext in TARGETS.items():
            if ext != "tar.gz":
                continue
            self.assertIn(f'url "{BASE}/permesh-{VERSION}-{target}.tar.gz"', formula)
            self.assertIn(f'sha256 "{self.expected[target]}"', formula)
        self.assertIn(f'version "{VERSION}"', formula)
        self.assertIn('bin.install "permesh"', formula)
        self.assertIn("test do", formula)
        self.assertIn('shell_output("#{bin}/permesh --version")', formula)

    def test_scoop_manifest_uses_windows_zip(self):
        manifest = manifests.scoop_manifest(VERSION, manifests.read_checksums(self.sums, VERSION))
        self.assertEqual(manifest["version"], VERSION)
        windows = manifest["architecture"]["64bit"]
        self.assertEqual(windows["url"], f"{BASE}/permesh-{VERSION}-x86_64-pc-windows-msvc.zip")
        self.assertEqual(windows["hash"], self.expected["x86_64-pc-windows-msvc"])
        self.assertEqual(manifest["bin"], "permesh.exe")

    def test_script_writes_both_manifests(self):
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(sorted(p.name for p in self.out.iterdir()), ["permesh.json", "permesh.rb"])

    def test_missing_target_fails_and_writes_nothing(self):
        (self.sums / f"permesh-{VERSION}-x86_64-apple-darwin.tar.gz.sha256").unlink()
        with self.assertRaises(ValueError):
            manifests.read_checksums(self.sums, VERSION)
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(list(self.out.iterdir()), [])

    def test_malformed_digest_fails(self):
        name = f"permesh-{VERSION}-aarch64-apple-darwin.tar.gz"
        for bad in ["A" * 64, "a" * 63]:
            (self.sums / f"{name}.sha256").write_text(f"{bad}  {name}\n", encoding="ascii")
            with self.assertRaises(ValueError):
                manifests.read_checksums(self.sums, VERSION)

    def test_checksum_for_a_different_file_name_fails(self):
        name = f"permesh-{VERSION}-aarch64-apple-darwin.tar.gz"
        (self.sums / f"{name}.sha256").write_text(f"{'a' * 64}  other.tar.gz\n", encoding="ascii")
        with self.assertRaises(ValueError):
            manifests.read_checksums(self.sums, VERSION)

    def test_script_creates_missing_output_directory(self):
        self.out.rmdir()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.out / "permesh.rb").is_file())

    def test_non_ascii_checksum_fails_cleanly(self):
        name = f"permesh-{VERSION}-aarch64-apple-darwin.tar.gz"
        (self.sums / f"{name}.sha256").write_bytes("\u00e4  x\n".encode("utf-8"))
        with self.assertRaises(ValueError):
            manifests.read_checksums(self.sums, VERSION)
        result = self.run_script()
        self.assertEqual(result.returncode, 1)
        self.assertNotIn("Traceback", result.stderr)


if __name__ == "__main__":
    unittest.main()
