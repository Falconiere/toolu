//! The process side of `interleave.rs`: the writer processes of a race, reaped
//! even when a check fails, and the checks on each writer's exit and output.

use std::io::Read as _;
use std::path::Path;
use std::process::{Child, Output};
use std::time::Duration;

use toolu_runtime::json::ordered::Ordered;

use crate::sandbox::Res;
use crate::writers::Writer;

/// No single operation of a Rust writer may wait this long (a stale-lock stall).
pub(crate) const STALL: Duration = Duration::from_secs(2);

/// The lock exists and names a pid and a version 4 UUID, as both writers write it.
pub(crate) fn lock_held(lock: &Path) -> bool {
  let Ok(content) = std::fs::read_to_string(lock) else {
    return false;
  };
  let Some((pid, token)) = content
    .strip_suffix('\n')
    .and_then(|line| line.split_once(' '))
  else {
    return false;
  };
  let uuid = token.len() == 36
    && token
      .bytes()
      .all(|byte| byte.is_ascii_hexdigit() || byte == b'-');
  !pid.is_empty() && pid.bytes().all(|byte| byte.is_ascii_digit()) && uuid
}

/// The whole numbers of a JSON array, as text.
pub(crate) fn numbers(value: &Ordered) -> Vec<String> {
  let Ordered::Array(items) = value else {
    return Vec::new();
  };
  items
    .iter()
    .filter_map(|item| {
      if let Ordered::Number(n) = item {
        Some(n.to_string())
      } else {
        None
      }
    })
    .collect()
}

/// Writer processes that are killed and reaped if a check fails before they exit.
pub(crate) struct Fleet<'a>(pub(crate) Vec<(&'a Writer, Child)>);

impl Drop for Fleet<'_> {
  fn drop(&mut self) {
    for (_, child) in &mut self.0 {
      if child.try_wait().ok().flatten().is_none() {
        child.kill().ok();
      }
      child.wait().ok();
    }
  }
}

/// The output of a child that has exited.
pub(crate) fn finished(child: &mut Child) -> Res<Output> {
  let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
  if let Some(mut out) = child.stdout.take() {
    out
      .read_to_end(&mut stdout)
      .map_err(|err| err.to_string())?;
  }
  if let Some(mut err) = child.stderr.take() {
    err
      .read_to_end(&mut stderr)
      .map_err(|err| err.to_string())?;
  }
  let status = child.wait().map_err(|err| err.to_string())?;
  Ok(Output {
    status,
    stdout,
    stderr,
  })
}

/// A writer exited 0 with nothing on stderr; a Rust writer never stalled and,
/// when `overlap` is required, cycled at least once.
pub(crate) fn check_writer(writer: &Writer, out: &Output, overlap: bool) -> Res<()> {
  let stderr = String::from_utf8_lossy(&out.stderr);
  if !out.status.success() || !stderr.is_empty() {
    return Err(format!(
      "writer {} failed ({}): {stderr}",
      writer.id(),
      out.status
    ));
  }
  let Writer::Rust(id) = writer else {
    return Ok(());
  };
  let stdout = String::from_utf8_lossy(&out.stdout);
  let field = |key: &str| {
    let found = stdout
      .split_whitespace()
      .find_map(|pair| pair.strip_prefix(key));
    found.and_then(|value| value.parse::<u64>().ok())
  };
  let (Some(ms), Some(cycles)) = (field("slowest_ms="), field("cycles=")) else {
    return Err(format!("writer {id} printed {stdout:?}"));
  };
  if Duration::from_millis(ms) >= STALL {
    return Err(format!("writer {id} stalled {ms} ms"));
  }
  if overlap && cycles == 0 {
    return Err(format!(
      "writer {id} stopped before the TypeScript writers ran"
    ));
  }
  Ok(())
}
