//! Select or stage the reporting executable without changing a worker install.

use crate::hc::remote::Transfer;
use toolu_runtime::{
  env::Env,
  process::{Spec, run},
};

/// Select a matching executable, staging the relay binary on compatible hosts.
pub(super) fn reporting_binary(transfer: &Transfer<'_>, env: &Env) -> Result<String, String> {
  if compatible(transfer, "toolu") {
    let resolved = transfer
      .exec(&["which", "toolu"], &[])
      .map_err(|e| e.message)?;
    let path = String::from_utf8(resolved)
      .map_err(|e| e.to_string())?
      .trim()
      .to_owned();
    if std::path::Path::new(&path).is_absolute() && compatible(transfer, &path) {
      return Ok(path);
    }
  }
  let remote = transfer
    .exec(&["uname", "-sm"], &[])
    .map_err(|e| e.message)?;
  let mut spec = Spec::new(["uname", "-sm"]);
  spec.env = Some(env.clone());
  let local = run(&spec).map_err(|e| format!("cannot identify relay platform: {e:?}"))?;
  if local.exit_code != 0
    || local.timed_out
    || local.truncated
    || !same_platform(&local.stdout_bytes, &remote)
  {
    return Err("worker platform differs from relay; install matching Toolu on the worker".into());
  }
  let exe = toolu_runtime::invocation::current_exe().ok_or("cannot locate relay executable")?;
  let bytes = std::fs::read(exe).map_err(|e| format!("reading relay executable: {e}"))?;
  let path = transfer
    .upload_file("toolu", &bytes)
    .map_err(|e| e.message)?;
  transfer
    .exec(&["chmod", "755", &path], &[])
    .map_err(|e| e.message)?;
  if !compatible(transfer, &path) {
    return Err("staged Toolu lacks matching version or delegate reporting".into());
  }
  Ok(path)
}

fn compatible(transfer: &Transfer<'_>, binary: &str) -> bool {
  let Ok(version) = transfer.exec(&[binary, "--version"], &[]) else {
    return false;
  };
  let Ok(help) = transfer.exec(&[binary, "delegate", "report", "--help"], &[]) else {
    return false;
  };
  supports(&version, &help)
}

fn same_platform(local: &[u8], remote: &[u8]) -> bool {
  local == remote
}

fn supports(version: &[u8], help: &[u8]) -> bool {
  if String::from_utf8_lossy(version).trim() != format!("toolu {}", env!("CARGO_PKG_VERSION")) {
    return false;
  }
  let help = String::from_utf8_lossy(help);
  ["--file ", "--attempt ", "--base ", "--status ", "--commit "]
    .iter()
    .all(|flag| help.contains(flag))
}

#[cfg(test)]
#[path = "tests/bootstrap_test.rs"]
mod tests;
