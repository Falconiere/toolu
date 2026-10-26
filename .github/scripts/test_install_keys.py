"""install.sh signature checks against real release signatures, served from disk."""

import os
import platform
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = Path(__file__).with_name("fixtures") / "install"
OS_NAMES = {"Darwin": "darwin", "Linux": "linux"}
ARCHES = {"x86_64": "amd64", "amd64": "amd64", "aarch64": "arm64", "arm64": "arm64"}


def install(tag: str) -> subprocess.CompletedProcess[str]:
    """Run install.sh for `tag` against the fixture's signed sums and a stand-in archive."""
    asset = f"toolu-{OS_NAMES[platform.system()]}-{ARCHES[platform.machine()]}.tar.gz"
    with tempfile.TemporaryDirectory() as directory:
        base = Path(directory)
        release = base / "Falconiere/toolu/releases/download" / tag
        release.mkdir(parents=True)
        for name in ("SHA256SUMS", "SHA256SUMS.minisig"):
            shutil.copy(FIXTURES / tag / name, release / name)
        (release / asset).write_bytes(b"not the release archive")
        env = {
            key: value
            for key, value in os.environ.items()
            if key not in {"TOOLU_MINISIGN_PUB", "TOOLU_REPO"}
        }
        env["TOOLU_DOWNLOAD_BASE"] = f"file://{base}"
        return subprocess.run(
            ["bash", str(ROOT / "install.sh"), "--version", tag, "--install-dir", str(base / "bin")],
            capture_output=True,
            text=True,
            check=False,
            env=env,
        )


class InstallKeysTest(unittest.TestCase):
    # The stand-in archive cannot match SHA256SUMS, so reaching the checksum
    # check proves the signature was accepted.
    def test_retired_key_still_verifies_v8_4_0(self) -> None:
        result = install("v8.4.0")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("checksum mismatch", result.stderr)

    def test_signature_from_an_unknown_key_is_refused(self) -> None:
        result = install("v8.4.1-dev.9")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("is not signed by the toolu release key", result.stderr)


if __name__ == "__main__":
    unittest.main()
