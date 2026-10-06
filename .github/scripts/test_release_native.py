"""Release helper checks against this checkout and a real local executable."""

import subprocess
import sys
import tarfile
import tempfile
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = Path(__file__).with_name("release_native.py")


def run_script(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


class ReleaseNativeTest(unittest.TestCase):
    def test_tag_matches_real_workspace(self) -> None:
        version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"][
            "version"
        ]
        self.assertEqual(run_script("verify-tag", f"v{version}").returncode, 0)
        self.assertEqual(run_script("verify-tag", f"v{version}-test.1", "--test-tag").returncode, 0)
        wrong = run_script("verify-tag", "v0.0.0")
        self.assertNotEqual(wrong.returncode, 0)
        self.assertIn("does not match workspace", wrong.stderr)

    def test_package_has_exact_root_layout_and_executable_mode(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            result = run_script("package", "linux", "amd64", sys.executable, str(dist))
            self.assertEqual(result.returncode, 0, result.stderr)
            with tarfile.open(dist / "toolu-linux-amd64.tar.gz", "r:gz") as archive:
                members = archive.getmembers()
                self.assertEqual([member.name for member in members], ["toolu", "LICENSE"])
                self.assertTrue(members[0].mode & 0o111)

    def test_missing_inputs_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            missing = root / "missing"
            package = run_script("package", "linux", "amd64", str(missing), str(root))
            self.assertIn("missing binary", package.stderr)
            verify = run_script(
                "verify-package", "v0.0.0", str(missing), str(missing), str(root / "extract")
            )
            self.assertIn("archive or checksums missing", verify.stderr)


if __name__ == "__main__":
    unittest.main()
