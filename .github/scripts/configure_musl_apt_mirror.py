#!/usr/bin/env python3
"""Use the Ubuntu archive when the x86_64 runner's Azure mirror stalls."""

import subprocess
import sys
from pathlib import Path


def configure(target: str, mirrors: Path) -> None:
    """Replace the stalled mirror for the x86_64 musl job when its list exists."""
    if target != "x86_64-unknown-linux-musl":
        return
    if mirrors.is_file():
        subprocess.run(
            [
                "sudo",
                "sed",
                "-i",
                "s|http://azure.archive.ubuntu.com/ubuntu|https://archive.ubuntu.com/ubuntu|g",
                str(mirrors),
            ],
            check=True,
        )


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: configure_musl_apt_mirror.py TARGET")
    configure(sys.argv[1], Path("/etc/apt/apt-mirrors.txt"))


if __name__ == "__main__":
    main()
