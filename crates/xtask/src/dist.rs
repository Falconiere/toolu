//! `cargo xtask dist checksums <dir>`: the release `SHA256SUMS` (#440 removed
//! `dist check`, the Bun bundle drift gate).

use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use crate::options::Options;
use crate::{Verdict, output};

/// The four archives `homebrew-formula` already requires, in its order.
const ARCHIVES: [&str; 4] = [
  "toolu-darwin-arm64.tar.gz",
  "toolu-darwin-amd64.tar.gz",
  "toolu-linux-arm64.tar.gz",
  "toolu-linux-amd64.tar.gz",
];

/// `dist checksums <dir>`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  match options.files.as_slice() {
    [mode, dir] if mode == Path::new("checksums") => Ok(output::findings("dist", &checksums(dir)?)),
    _ => Err("usage: cargo xtask dist checksums <dir>".to_owned()),
  }
}

fn checksums(dir: &Path) -> Result<Vec<String>, String> {
  let missing: Vec<&str> = ARCHIVES
    .into_iter()
    .filter(|name| !dir.join(name).is_file())
    .collect();
  if !missing.is_empty() {
    return Ok(
      missing
        .into_iter()
        .map(|name| format!("missing {name}"))
        .collect(),
    );
  }
  let mut body = String::new();
  for name in ARCHIVES {
    let hex = sha256(&dir.join(name))?;
    let _ = std::fmt::Write::write_fmt(&mut body, format_args!("{hex}  {name}\n"));
  }
  let dest = dir.join("SHA256SUMS");
  let tmp = dir.join(".SHA256SUMS.tmp");
  std::fs::write(&tmp, body).map_err(|err| format!("cannot write {}: {err}", tmp.display()))?;
  std::fs::rename(&tmp, &dest).map_err(|err| format!("cannot write {}: {err}", dest.display()))?;
  Ok(Vec::new())
}

fn sha256(path: &Path) -> Result<String, String> {
  if let Some(hex) = digest_command("sha256sum", &[], path)? {
    return Ok(hex);
  }
  if let Some(hex) = digest_command("shasum", &["-a", "256"], path)? {
    return Ok(hex);
  }
  Err("sha256sum is not installed".to_owned())
}

fn digest_command(program: &str, prefix: &[&str], path: &Path) -> Result<Option<String>, String> {
  match Command::new(program).args(prefix).arg(path).output() {
    Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
    Err(err) => Err(format!("{program}: {err}")),
    Ok(output) if output.status.success() => Ok(Some(hex_token(&output.stdout)?)),
    Ok(output) => Err(format!(
      "{program} failed: {}",
      String::from_utf8_lossy(&output.stderr).trim()
    )),
  }
}

fn hex_token(stdout: &[u8]) -> Result<String, String> {
  let text = String::from_utf8_lossy(stdout);
  let Some(token) = text.split_whitespace().next() else {
    return Err("sha256: empty output".to_owned());
  };
  if token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
    return Ok(token.to_ascii_lowercase());
  }
  Err(format!("sha256: unexpected output {token}"))
}

#[cfg(test)]
#[path = "tests/dist_test.rs"]
mod tests;
