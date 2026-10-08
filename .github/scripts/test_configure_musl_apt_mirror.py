"""Real-file tests for the musl apt mirror setup."""

import tempfile
import unittest
from pathlib import Path

from configure_musl_apt_mirror import configure


AZURE = "http://azure.archive.ubuntu.com/ubuntu"
AZURE_HTTPS = "https://azure.archive.ubuntu.com/ubuntu"
UBUNTU = "https://archive.ubuntu.com/ubuntu"


class ConfigureMuslAptMirrorTests(unittest.TestCase):
    def test_x86_replaces_azure_mirror(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            mirrors = Path(directory) / "apt-mirrors.txt"
            mirrors.write_text(f"{AZURE}\n{AZURE}/\n{AZURE_HTTPS}\n{UBUNTU}\n")
            configure("x86_64-unknown-linux-musl", mirrors)
            self.assertEqual(
                mirrors.read_text(), f"{UBUNTU}\n{UBUNTU}/\n{UBUNTU}\n{UBUNTU}\n"
            )

    def test_arm_leaves_mirror_unchanged(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            mirrors = Path(directory) / "apt-mirrors.txt"
            mirrors.write_text(f"{AZURE}\n")
            configure("aarch64-unknown-linux-musl", mirrors)
            self.assertEqual(mirrors.read_text(), f"{AZURE}\n")

    def test_missing_mirror_list_is_allowed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            configure("x86_64-unknown-linux-musl", Path(directory) / "missing")


if __name__ == "__main__":
    unittest.main()
