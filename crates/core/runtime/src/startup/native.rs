//! `SessionStart` advice when the non-login shell has no native `toolu`
//! (`packages/toolu-core/src/startup/native-toolu.ts`). A probe error is not a
//! native binary, so the caller still gets the install line.

use std::fs::{DirBuilder, OpenOptions};
use std::io::ErrorKind;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

use super::context::{render_hook_output, session_context};
use crate::env::Env;
use crate::host::roots::Roots;
use crate::json::ordered::Ordered;
use crate::process::commands::{ShellToolu, known_native_toolu, shell_toolu};

const INSTALLER: &str = "curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash";
const HOMEBREW: &str = "brew install falconiere/tap/toolu";

/// One advisory line, or nothing when `command -v toolu` is already native or
/// this session id was already told.
pub fn native_toolu_advice(env: &Env, session_id: Option<&str>) -> Option<String> {
  if shell_is_native(env) {
    return None;
  }
  claim_notice(env, session_id).then_some(advice_line(env))
}

/// The `SessionStart` hook output, without a trailing newline, carrying
/// [`native_toolu_advice`] as context, keyed by the payload's `session_id`,
/// else `TOOLU_SESSION_ID`.
pub fn native_toolu_hook_output(env: &Env, payload: Option<&str>) -> Option<String> {
  let id = payload_session_id(env, payload);
  native_toolu_advice(env, id.as_deref())
    .and_then(|line| session_context("SessionStart", &line))
    .map(|context| render_hook_output(&context, false).trim_end().to_owned())
}

fn payload_session_id(env: &Env, payload: Option<&str>) -> Option<String> {
  let parsed = payload.and_then(|text| Ordered::parse(text).ok());
  match parsed.as_ref().and_then(|parsed| parsed.get("session_id")) {
    Some(Ordered::String(id)) if !id.is_empty() => Some(id.clone()),
    _ => env.get("TOOLU_SESSION_ID").map(str::to_owned),
  }
}

fn shell_is_native(env: &Env) -> bool {
  matches!(shell_toolu(env), Ok(ShellToolu::Native(_)))
}

fn advice_line(env: &Env) -> String {
  match known_native_toolu(env) {
    Some(path) => format!(
      "toolu: native binary for this session: {}. Use that absolute path for toolu commands.",
      shell_quote(&path)
    ),
    None => format!(
      "toolu: native binary not found in the agent command shell. \
Install it with: {INSTALLER} or {HOMEBREW}. Restart the session."
    ),
  }
}

fn shell_quote(path: &Path) -> String {
  format!("'{}'", path.to_string_lossy().replace('\'', r"'\''"))
}

/// `true` when this call may print. A missing id prints every time. A present
/// id prints once; the filename is the FNV-1a hex of that id.
fn claim_notice(env: &Env, session_id: Option<&str>) -> bool {
  let Some(id) = session_id.filter(|id| !id.is_empty()) else {
    return true;
  };
  let dir = Roots::new(env.clone(), None)
    .config_root()
    .join("toolu")
    .join("native-notices");
  if DirBuilder::new()
    .recursive(true)
    .mode(0o700)
    .create(&dir)
    .is_err()
  {
    return true;
  }
  match OpenOptions::new()
    .write(true)
    .create_new(true)
    .mode(0o600)
    .open(dir.join(notice_name(id)))
  {
    Err(error) if error.kind() == ErrorKind::AlreadyExists => false,
    Ok(_) | Err(_) => true,
  }
}

fn notice_name(session_id: &str) -> PathBuf {
  let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
  for byte in session_id.as_bytes() {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(0x0100_0000_01b3);
  }
  PathBuf::from(format!("{hash:016x}"))
}

#[cfg(test)]
#[path = "tests/native_test.rs"]
mod tests;
