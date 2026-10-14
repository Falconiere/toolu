//! The fixed sandbox of `cargo xtask bench-statusline`: a repository with a linked
//! worktree holding an untracked file, a primed config directory and a cold one, in
//! a private home, all removed on drop.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Environment keys a host session sets, never inherited by a measured render.
const HOST_PREFIXES: [&str; 4] = ["CLAUDE_", "CODEX_", "TOOLU_", "CURSOR_"];

/// The sandbox directories.
pub(crate) struct Fixture {
  root: PathBuf,
  pub(crate) worktree: PathBuf,
  pub(crate) cfg: PathBuf,
  pub(crate) cold_cfg: PathBuf,
}

impl Drop for Fixture {
  fn drop(&mut self) {
    let _gone = std::fs::remove_dir_all(&self.root);
  }
}

fn git(dir: &Path, args: &[&str]) -> Result<(), String> {
  let output = Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .output()
    .map_err(|err| format!("bench-statusline: cannot run git: {err}"))?;
  if output.status.success() {
    return Ok(());
  }
  let stderr = String::from_utf8_lossy(&output.stderr);
  Err(format!(
    "bench-statusline: git {} failed: {}",
    args.join(" "),
    stderr.trim()
  ))
}

impl Fixture {
  /// Build the repository, its linked worktree and the two config directories.
  pub(crate) fn new() -> Result<Fixture, String> {
    static TICK: AtomicU64 = AtomicU64::new(0);
    let tick = TICK.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
      "toolu-statusline-bench-{}-{tick}",
      std::process::id()
    ));
    let fixture = Fixture {
      worktree: root.join("linked"),
      cfg: root.join("cfg"),
      cold_cfg: root.join("cold-cfg"),
      root,
    };
    let io = |err: std::io::Error| format!("bench-statusline: {err}");
    let repo = fixture.root.join("repo");
    for dir in [&repo, &fixture.cfg, &fixture.cold_cfg] {
      std::fs::create_dir_all(dir).map_err(io)?;
    }
    git(&repo, &["init", "-q", "-b", "main"])?;
    let identity = [
      "-c",
      "user.email=bench@example.test",
      "-c",
      "user.name=Bench",
    ];
    let commit = [
      "-c",
      "commit.gpgsign=false",
      "commit",
      "-q",
      "--allow-empty",
      "-m",
      "init",
    ];
    git(&repo, &[identity.as_slice(), commit.as_slice()].concat())?;
    let linked = fixture.worktree.display().to_string();
    git(
      &repo,
      &["worktree", "add", "-q", "-b", "bench/statusline", &linked],
    )?;
    std::fs::write(fixture.worktree.join("untracked.txt"), "statusline\n").map_err(io)?;
    Ok(fixture)
  }

  /// The environment of a render: this process's minus host-session keys, the
  /// private home, `cfg` as the config directory, and `bin`'s directory on `PATH`.
  pub(crate) fn env(&self, bin: &Path, cfg: &Path) -> Vec<(String, String)> {
    let managed = ["HOME", "PATH", "TYPESAFE_API_KEY"];
    let mut env: Vec<(String, String)> = std::env::vars()
      .filter(|(key, _)| {
        !managed.contains(&key.as_str()) && !HOST_PREFIXES.iter().any(|p| key.starts_with(p))
      })
      .collect();
    let inherited = std::env::var("PATH").unwrap_or_default();
    let dir = bin.parent().unwrap_or(Path::new("")).display();
    env.extend([
      ("HOME".to_owned(), self.root.display().to_string()),
      ("CLAUDE_CONFIG_DIR".to_owned(), cfg.display().to_string()),
      (
        "TYPESAFE_API_KEY".to_owned(),
        "statusline-benchmark-key".to_owned(),
      ),
      ("PATH".to_owned(), format!("{dir}:{inherited}")),
    ]);
    env
  }
}

/// The performance fixture's JSON with `workspace.current_dir` set to `worktree`.
pub(crate) fn payload(fixture: &str, worktree: &Path) -> Result<String, String> {
  let mut doc: serde_json::Value = serde_json::from_str(fixture)
    .map_err(|err| format!("bench-statusline: the performance fixture: {err}"))?;
  let workspace = doc
    .get_mut("workspace")
    .and_then(serde_json::Value::as_object_mut)
    .ok_or("bench-statusline: the performance fixture has no workspace")?;
  workspace.insert(
    "current_dir".to_owned(),
    serde_json::Value::String(worktree.display().to_string()),
  );
  Ok(doc.to_string())
}

#[cfg(test)]
#[path = "tests/bench_statusline_repo_test.rs"]
mod tests;
