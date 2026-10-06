//! The release packager against Cargo's real `toolu` executable.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn root() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn script(args: &[&str]) -> Output {
  Command::new("python3")
    .arg(".github/scripts/release_native.py")
    .args(args)
    .current_dir(root())
    .output()
    .unwrap()
}

fn verify_archive(version: &str, archive: &Path, sums: &Path, dest: &Path) -> Output {
  let archive_text = archive.to_string_lossy();
  let sums_text = sums.to_string_lossy();
  let dest_text = dest.to_string_lossy();
  script(&[
    "verify-package",
    version,
    &archive_text,
    &sums_text,
    &dest_text,
  ])
}

#[test]
fn tag_matches_the_workspace_and_plugin_manifests() {
  let version = env!("CARGO_PKG_VERSION");
  let good = script(&["verify-tag", &format!("v{version}")]);
  assert!(
    good.status.success(),
    "{}",
    String::from_utf8_lossy(&good.stderr)
  );
  let bad = script(&["verify-tag", "v0.0.0"]);
  assert!(!bad.status.success());
  assert!(String::from_utf8_lossy(&bad.stderr).contains("does not match workspace"));
  let dry_run = script(&["verify-tag", &format!("v{version}-test.1"), "--test-tag"]);
  assert!(dry_run.status.success());
  let unpublished = script(&["verify-tag", &format!("v{version}-test.1")]);
  assert!(!unpublished.status.success());
}

#[test]
fn real_binary_archive_checks_checksum_layout_and_version() {
  let temp = TempDir::new().unwrap();
  let dist = temp.path().join("dist");
  let dist_text = dist.to_string_lossy();
  let binary = env!("CARGO_BIN_EXE_toolu");
  let built = script(&["package", "linux", "amd64", binary, &dist_text]);
  assert!(
    built.status.success(),
    "{}",
    String::from_utf8_lossy(&built.stderr)
  );
  let archive = dist.join("toolu-linux-amd64.tar.gz");
  let digest = Command::new("shasum")
    .args(["-a", "256"])
    .arg(&archive)
    .output()
    .unwrap();
  assert!(digest.status.success());
  let line = String::from_utf8(digest.stdout).unwrap();
  let hash = line.split_whitespace().next().unwrap();
  let sums = dist.join("SHA256SUMS");
  std::fs::write(&sums, format!("{hash}  toolu-linux-amd64.tar.gz\n")).unwrap();
  let dest = temp.path().join("extracted");
  let version = format!("v{}", env!("CARGO_PKG_VERSION"));
  let good = verify_archive(&version, &archive, &sums, &dest);
  assert!(
    good.status.success(),
    "{}",
    String::from_utf8_lossy(&good.stderr)
  );
  let wrong_version = verify_archive("v0.0.0", &archive, &sums, &dest);
  assert!(!wrong_version.status.success());
  assert!(String::from_utf8_lossy(&wrong_version.stderr).contains("binary version"));
  let missing = verify_archive(&version, &dist.join("missing.tar.gz"), &sums, &dest);
  assert!(!missing.status.success());
  assert!(String::from_utf8_lossy(&missing.stderr).contains("archive or checksums missing"));
  std::fs::write(
    &sums,
    format!("{}  toolu-linux-amd64.tar.gz\n", "0".repeat(64)),
  )
  .unwrap();
  let corrupt = verify_archive(&version, &archive, &sums, &dest);
  assert!(!corrupt.status.success());
  assert!(String::from_utf8_lossy(&corrupt.stderr).contains("checksum mismatch"));
}
