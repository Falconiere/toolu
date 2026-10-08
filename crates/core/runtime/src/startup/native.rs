//! `SessionStart` advice when the non-login shell has no native `toolu`
//! (`packages/toolu-core/src/startup/native-toolu.ts`). A probe error is not a
//! native binary, so the caller still gets the install line.

use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::host::roots::Roots;
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
      "toolu: native binary not found. Install it with `{INSTALLER}` or `{HOMEBREW}`, \
then use `toolu` directly."
    ),
  }
}

fn shell_quote(path: &Path) -> String {
  format!("'{}'", path.to_string_lossy().replace('\'', r#"'"'"'"#))
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
  if std::fs::create_dir_all(&dir).is_err() {
    return true;
  }
  match OpenOptions::new()
    .write(true)
    .create_new(true)
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
