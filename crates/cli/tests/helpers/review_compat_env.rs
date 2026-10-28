//! Host-home isolation for `review_compat` so a real `~/.codex` registry cannot fire.

use std::fs;
use std::path::Path;

/// Point HOME and the host config root at `dir` so installed plugins stay out.
pub(crate) fn isolate(command: &mut assert_cmd::Command, dir: &Path, host: &str) {
  let home = dir.join(format!(".{host}"));
  let _ = fs::create_dir_all(&home);
  command
    .env("HOME", dir)
    .env("PUSH_REVIEW_BASE", "main")
    .env_remove("TOOLU_CONFIG_DIR")
    .env_remove("CODEX_HOME")
    .env_remove("CLAUDE_CONFIG_DIR");
  match host {
    "codex" => {
      command.env("CODEX_HOME", home);
    }
    "claude" => {
      command.env("CLAUDE_CONFIG_DIR", home);
    }
    _ => {}
  }
}
