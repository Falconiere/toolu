#!/usr/bin/env python3
"""Package and verify the native toolu release layout."""

import argparse
import hashlib
import json
import re
import subprocess
import tarfile
from pathlib import Path
from typing import NoReturn


def fail(message: str) -> NoReturn:
    raise SystemExit(message)


def verify_tag(tag: str, test_tag: bool) -> None:
    import tomllib

    root = Path.cwd()
    version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    is_test = test_tag and re.fullmatch(
        rf"v{re.escape(version)}-test(?:\.[0-9A-Za-z-]+)*", tag
    ) is not None
    if tag != f"v{version}" and not is_test:
        fail(f"tag {tag} does not match workspace v{version}")
    lock = tomllib.loads((root / "Cargo.lock").read_text())
    for package in lock["package"]:
        if "source" not in package and package["version"] != version:
            fail(
                f"Cargo.lock {package['name']} version {package['version']} "
                f"does not match {version}"
            )
    paths = [root / "package.json"]
    for pattern in (
        "packages/*/package.json",
        "tools/*/package.json",
        "tools/*/npm/package.json",
        "plugins/*/.claude-plugin/plugin.json",
        "plugins/*/.codex-plugin/plugin.json",
    ):
        paths.extend(root.glob(pattern))
    for path in paths:
        found = json.loads(path.read_text())["version"]
        if found != version:
            fail(f"{path.relative_to(root)} version {found} does not match {version}")
    print(f"verified {tag} against workspace and {len(paths)} manifests")


def add_member(archive: tarfile.TarFile, path: Path, name: str, mode: int) -> None:
    info = tarfile.TarInfo(name)
    info.size = path.stat().st_size
    info.mode = mode
    with path.open("rb") as source:
        archive.addfile(info, source)


def package(os_name: str, arch: str, binary: Path, dist: Path) -> None:
    if not binary.is_file():
        fail(f"missing binary: {binary}")
    dist.mkdir(parents=True, exist_ok=True)
    with tarfile.open(dist / f"toolu-{os_name}-{arch}.tar.gz", "w:gz") as archive:
        add_member(archive, binary, "toolu", 0o755)
        add_member(archive, Path("LICENSE"), "LICENSE", 0o644)


def checksum(name: str, sums: Path) -> str:
    for line in sums.read_text().splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[1] == name:
            return fields[0]
    fail(f"no checksum for {name}")


def extract_verified(archive_path: Path, dest: Path) -> None:
    with tarfile.open(archive_path, "r:gz") as archive:
        members = archive.getmembers()
        if [member.name for member in members] != ["toolu", "LICENSE"]:
            fail(f"wrong archive layout: {archive_path.name}")
        if not all(member.isfile() for member in members) or not members[0].mode & 0o111:
            fail(f"archive members are not regular executable files: {archive_path.name}")
        dest.mkdir(parents=True, exist_ok=True)
        for member in members:
            source = archive.extractfile(member)
            if source is None:
                fail(f"cannot extract {member.name}: {archive_path.name}")
            target = dest / member.name
            with source:
                target.write_bytes(source.read())
            target.chmod(member.mode)


def verify_package(tag: str, archive: Path, sums: Path, dest: Path, test_tag: bool) -> None:
    if not archive.is_file() or not sums.is_file():
        fail("archive or checksums missing")
    if not archive.stat().st_size or not sums.stat().st_size:
        fail("archive or checksums missing")
    expected = checksum(archive.name, sums)
    actual = hashlib.sha256(archive.read_bytes()).hexdigest()
    if actual != expected:
        fail(f"checksum mismatch for {archive.name}")
    if archive.stat().st_size < 1048576:
        fail(f"archive too small: {archive.name}")
    extract_verified(archive, dest)
    result = subprocess.run(
        [str((dest / "toolu").resolve()), "--version"],
        capture_output=True,
        text=True,
        check=False,
    )
    version = result.stdout.strip()
    expected_version = tag[1:] if tag.startswith("v") else tag
    if test_tag and "-test" in expected_version:
        expected_version = expected_version.rpartition("-test")[0]
    if result.returncode != 0 or version != f"toolu {expected_version}":
        fail(f"binary version {version} differs from {tag}")
    print(f"verified {archive.name} ({version})")


def main() -> None:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    tag = commands.add_parser("verify-tag")
    tag.add_argument("tag")
    tag.add_argument("--test-tag", action="store_true")
    pack = commands.add_parser("package")
    pack.add_argument("os")
    pack.add_argument("arch")
    pack.add_argument("binary", type=Path)
    pack.add_argument("dist", type=Path)
    verify = commands.add_parser("verify-package")
    verify.add_argument("tag")
    verify.add_argument("archive", type=Path)
    verify.add_argument("sums", type=Path)
    verify.add_argument("dest", type=Path)
    verify.add_argument("--test-tag", action="store_true")
    args = parser.parse_args()
    if args.command == "verify-tag":
        verify_tag(args.tag, args.test_tag)
    elif args.command == "package":
        package(args.os, args.arch, args.binary, args.dist)
    else:
        verify_package(args.tag, args.archive, args.sums, args.dest, args.test_tag)


if __name__ == "__main__":
    main()
