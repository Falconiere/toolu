#!/usr/bin/env python3
"""Use the Ubuntu archive when the x86_64 runner's Azure mirror stalls."""

import sys
from pathlib import Path


def configure(target: str, mirrors: Path) -> None:
    """Replace the stalled mirror for the x86_64 musl job when its list exists."""
    if target != "x86_64-unknown-linux-musl":
        return
    if not mirrors.is_file():
        return

    contents = mirrors.read_text(encoding="utf-8")
    mirrors.write_text(
        contents.replace("azure.archive.ubuntu.com", "archive.ubuntu.com"),
        encoding="utf-8",
    )
    if "azure.archive.ubuntu.com" in mirrors.read_text(encoding="utf-8"):
        raise SystemExit("Azure apt mirror remains in apt-mirrors.txt")


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: configure_musl_apt_mirror.py TARGET")
    configure(sys.argv[1], Path("/etc/apt/apt-mirrors.txt"))


if __name__ == "__main__":
    main()
