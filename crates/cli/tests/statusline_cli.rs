//! Native statusline rendering against real repositories and linked worktrees.

#[path = "helpers/feed.rs"]
mod feed;
#[macro_use]
#[path = "helpers/ensure.rs"]
mod ensure;
#[path = "helpers/statusline.rs"]
mod statusline;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use statusline::{TOOLU, TestResult, cmd, text};

fn prime(home: &Path, cfg: &Path, cwd: &Path) -> TestResult<()> {
  let worktree = cwd
    .to_str()
    .ok_or_else(|| std::io::Error::other("worktree path"))?;
  let result = cmd(
    home,
    cfg,
    cwd,
    &["statusline", "refresh-cache", worktree],
    None,
  )?;
  ensure!(
    result.status.success(),
    "{}",
    String::from_utf8_lossy(&result.stderr)
  );
  Ok(())
}

fn git(dir: &Path, args: &[&str]) -> TestResult<()> {
  let output = Command::new("git").arg("-C").arg(dir).args(args).output()?;
  ensure!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  Ok(())
}

fn repo(dir: &Path) -> TestResult<()> {
  fs::create_dir_all(dir)?;
  git(dir, &["init", "-q", "-b", "main"])?;
  git(dir, &["config", "user.email", "test@example.test"])?;
  git(dir, &["config", "user.name", "Statusline Test"])?;
  fs::write(dir.join("tracked.txt"), "one\n")?;
  fs::write(dir.join(".gitignore"), ".claude/\n*.profraw\n")?;
  git(dir, &["add", "tracked.txt", ".gitignore"])?;
  git(dir, &["commit", "-q", "-m", "initial"])
}

fn payload(dir: &Path) -> String {
  json!({
    "model":{"display_name":"Opus"},
    "workspace":{"current_dir":dir.display().to_string()},
    "context_window":{"context_window_size":200_000,"total_input_tokens":45_000,"used_percentage":22.5}
  }).to_string()
}

fn state(repo: &Path, cfg: &Path) -> TestResult<()> {
  let gate = repo.join(".claude/tmp/quality-gate-status.json");
  fs::create_dir_all(
    gate
      .parent()
      .ok_or_else(|| std::io::Error::other("gate parent"))?,
  )?;
  fs::write(
    gate,
    r#"{"status":"failing","reason":"test","source":"statusline","file":"tracked.txt","violations":"test","updatedAt":"2026-10-09T00:00:00Z"}"#,
  )?;
  let marker = cfg.join("comemory-status/repo.json");
  fs::create_dir_all(
    marker
      .parent()
      .ok_or_else(|| std::io::Error::other("marker parent"))?,
  )?;
  fs::write(marker, r#"{"count":42}"#)?;
  fs::write(
    cfg.join(".claude.json"),
    r#"{"oauthAccount":{"emailAddress":"user@example.test"}}"#,
  )?;
  Ok(())
}

fn assert_full_line(line: &str) -> TestResult<()> {
  ensure!(!line.ends_with('\n'));
  ensure!(line.contains("ctx:45k/200k (22%)"));
  ensure!(line.contains("example.test"));
  ensure!(line.contains("✗ gate:failing"));
  ensure!(line.contains("main\u{1b}[0m\u{1b}[33m[~1 ?1]"));
  ensure!(line.contains("[COMEMORY:42]"));
  ensure!(line.ends_with("[JEV:READY]\u{1b}[0m"));
  Ok(())
}

struct Instrumentation<'a> {
  home: &'a Path,
  cfg: &'a Path,
  repo: &'a Path,
  bin: &'a Path,
  log: &'a Path,
  release: &'a Path,
}

fn instrumented_render(test: &Instrumentation<'_>) -> TestResult<Output> {
  let mut command = Command::new(TOOLU);
  command
    .args(["statusline", "render"])
    .current_dir(test.repo)
    .env_clear()
    .env("HOME", test.home)
    .env("CLAUDE_CONFIG_DIR", test.cfg)
    .env("TOOLU_HOST_OVERRIDE", "claude")
    .env("PATH", format!("{}:/usr/bin:/bin", test.bin.display()))
    .env("TOOLU_TEST_LOG", test.log)
    .env("TOOLU_TEST_STATUS", test.log.with_extension("status"))
    .env("TOOLU_TEST_ERROR", test.log.with_extension("error"))
    .env("TOOLU_TEST_RELEASE", test.release)
    .env(
      "TMPDIR",
      std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_owned()),
    )
    .stdin(Stdio::piped())
    .stdout(Stdio::piped());
  if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
    command.env("LLVM_PROFILE_FILE", profile);
  }
  let mut child = command.spawn()?;
  feed::feed(child.stdin.take(), payload(test.repo).as_bytes())?;
  Ok(child.wait_with_output()?)
}

fn wait_for(mut ready: impl FnMut() -> bool) -> TestResult<()> {
  let deadline = Instant::now() + Duration::from_secs(5);
  while !ready() {
    ensure!(Instant::now() < deadline, "timed out waiting for refresher");
    std::thread::sleep(Duration::from_millis(10));
  }
  Ok(())
}

fn check_updated(cache: &Path, log: &Path) -> TestResult<()> {
  let updated: Value = serde_json::from_slice(&fs::read(cache)?)?;
  ensure!(
    updated.get("untracked").and_then(Value::as_u64) == Some(1)
      && updated.get("unstaged").and_then(Value::as_u64) == Some(1),
    "cache: {updated}; git stdout: {}; git stderr: {}",
    fs::read_to_string(log.with_extension("status")).unwrap_or_default(),
    fs::read_to_string(log.with_extension("error")).unwrap_or_default()
  );
  Ok(())
}

#[test]
fn render_has_exact_line_ending_and_json_shape_on_a_real_dirty_repo() -> TestResult<()> {
  let temp = tempfile::tempdir()?;
  let dir = temp.path().join("repo");
  let cfg = temp.path().join("cfg");
  repo(&dir)?;
  fs::create_dir_all(&cfg)?;
  state(&dir, &cfg)?;
  fs::write(dir.join("tracked.txt"), "one\ntwo\n")?;
  fs::write(dir.join("new.txt"), "new\n")?;
  prime(temp.path(), &cfg, &dir)?;
  let input = payload(&dir);
  let plain = cmd(
    temp.path(),
    &cfg,
    &dir,
    &["statusline", "render"],
    Some(&input),
  )?;
  ensure!(
    plain.status.success(),
    "{}",
    String::from_utf8_lossy(&plain.stderr)
  );
  let line = text(&plain);
  assert_full_line(&line)?;
  let output = cmd(
    temp.path(),
    &cfg,
    &dir,
    &["--json", "statusline", "render"],
    Some(&input),
  )?;
  let document: Value = serde_json::from_slice(&output.stdout)?;
  ensure_eq!(document, json!({"line":line}));
  Ok(())
}

#[test]
fn linked_worktree_uses_its_branch_and_the_main_repo_comemory_marker() -> TestResult<()> {
  let temp = tempfile::tempdir()?;
  let dir = temp.path().join("repo");
  let cfg = temp.path().join("cfg");
  repo(&dir)?;
  fs::create_dir_all(&cfg)?;
  let worktree = temp.path().join("linked");
  let worktree_arg = worktree
    .to_str()
    .ok_or_else(|| std::io::Error::other("worktree path"))?;
  git(
    &dir,
    &["worktree", "add", "-q", "-b", "feat/x", worktree_arg],
  )?;
  fs::write(worktree.join("untracked.txt"), "new\n")?;
  prime(temp.path(), &cfg, &worktree)?;
  let marker = cfg.join("comemory-status/repo.json");
  fs::create_dir_all(
    marker
      .parent()
      .ok_or_else(|| std::io::Error::other("marker parent"))?,
  )?;
  fs::write(marker, r#"{"count":7}"#)?;
  let output = cmd(
    temp.path(),
    &cfg,
    &worktree,
    &["statusline", "render"],
    Some(&payload(&worktree)),
  )?;
  ensure!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let line = text(&output);
  ensure!(line.contains("feat/x\u{1b}[0m\u{1b}[33m[?1]"));
  ensure!(line.contains("[COMEMORY:7]"));
  Ok(())
}

#[test]
fn stale_counts_lag_one_refresh_and_warm_render_starts_no_git_child() -> TestResult<()> {
  let temp = tempfile::tempdir()?;
  let repo_dir = temp.path().join("repo");
  let cfg = temp.path().join("cfg");
  repo(&repo_dir)?;
  fs::write(repo_dir.join("first.txt"), "first\n")?;
  prime(temp.path(), &cfg, &repo_dir)?;
  let cache_dir = cfg.join("toolu/statusline-cache");
  let cache = fs::read_dir(&cache_dir)?
    .next()
    .ok_or_else(|| std::io::Error::other("missing cache record"))??
    .path();
  let mut record: Value = serde_json::from_slice(&fs::read(&cache)?)?;
  record["writtenAtMs"] = json!(0);
  fs::write(&cache, serde_json::to_vec(&record)?)?;
  fs::write(repo_dir.join("tracked.txt"), "one\ntwo\n")?;
  let bin = temp.path().join("bin");
  fs::create_dir_all(&bin)?;
  let log = temp.path().join("git.log");
  let release = temp.path().join("release");
  fs::write(
    bin.join("git"),
    "#!/bin/sh\nprintf 'git\\n' >> \"$TOOLU_TEST_LOG\"\nwhile [ ! -e \"$TOOLU_TEST_RELEASE\" ]; do sleep 0.01; done\n/usr/bin/git \"$@\" > \"$TOOLU_TEST_STATUS\" 2> \"$TOOLU_TEST_ERROR\"\nresult=$?\ncat \"$TOOLU_TEST_STATUS\"\nexit \"$result\"\n",
  )?;
  fs::set_permissions(bin.join("git"), fs::Permissions::from_mode(0o700))?;
  let test = Instrumentation {
    home: temp.path(),
    cfg: &cfg,
    repo: &repo_dir,
    bin: &bin,
    log: &log,
    release: &release,
  };
  let first = instrumented_render(&test)?;
  ensure!(first.status.success());
  ensure!(text(&first).contains("[?1]"));
  wait_for(|| log.exists())?;
  let second = instrumented_render(&test)?;
  ensure!(second.status.success());
  ensure!(text(&second).contains("[?1]"));
  ensure_eq!(fs::read_to_string(&log)?, "git\n");
  ensure!(cache.with_extension("lock").exists());
  fs::write(&release, "go")?;
  wait_for(|| !cache.with_extension("lock").exists())?;
  check_updated(&cache, &log)?;
  let warm = instrumented_render(&test)?;
  ensure!(warm.status.success());
  ensure!(text(&warm).contains("[~1 ?1]"));
  ensure_eq!(fs::read_to_string(&log)?, "git\n");
  Ok(())
}
