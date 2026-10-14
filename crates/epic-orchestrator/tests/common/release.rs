//! The release `toolu` binary the Linux budget tests measure, built once into
//! `target/status-budget` (or taken from `TOOLU_RELEASE_BIN`).

use std::io;
use std::path::PathBuf;
use std::process::Command;

pub(crate) fn contextual<T, E: std::fmt::Display>(
  result: Result<T, E>,
  action: &str,
) -> io::Result<T> {
  result.map_err(|error| io::Error::other(format!("{action}: {error}")))
}

pub(crate) fn release_binary() -> io::Result<PathBuf> {
  if let Some(path) = std::env::var_os("TOOLU_RELEASE_BIN") {
    return Ok(PathBuf::from(path));
  }
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
  let target = std::env::var_os("CARGO_TARGET_DIR")
    .map_or_else(|| root.join("target"), PathBuf::from)
    .join("status-budget");
  let output = Command::new("cargo")
    .args(["build", "--release", "--locked", "-p", "toolu-cli"])
    .arg("--manifest-path")
    .arg(root.join("Cargo.toml"))
    .arg("--target-dir")
    .arg(&target)
    .env_remove("RUSTFLAGS")
    .env_remove("CARGO_ENCODED_RUSTFLAGS")
    .output();
  let output = contextual(output, "build release status binary")?;
  if !output.status.success() {
    return Err(io::Error::other(format!(
      "release build failed: {}",
      String::from_utf8_lossy(&output.stderr)
    )));
  }
  Ok(target.join("release/toolu"))
}
