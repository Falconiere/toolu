use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::json;
use toolu_runtime::env::Env;

use super::{MAX_FILES, await_session, capture_session, require_captured_session};
use crate::hosts::HostKind::{Claude, Codex, Cursor};
use crate::launch::digest::md5_hex;

const FIRST: &str = "0199a272-0b4e-7cc3-8c4f-071c81275655";
const SECOND: &str = "0199a272-0b4e-7cc3-8c4f-071c81275656";

fn since() -> u64 {
  toolu_state::time::epoch_millis(SystemTime::now()) - 1_000
}

fn env(pairs: &[(&str, String)]) -> Env {
  let mut env = Env::from_pairs([
    ("HOME", "/nonexistent".to_owned()),
    ("PATH", "/usr/bin:/bin".to_owned()),
  ]);
  for (key, value) in pairs {
    env = env.with(key, value);
  }
  env
}

/// `<root>/sessions/<yyyy>/<mm>/<dd>` for the UTC date of `at` (epoch ms).
fn codex_day(root: &Path, at: u64) -> PathBuf {
  let day = toolu_state::time::iso_seconds(SystemTime::UNIX_EPOCH + Duration::from_millis(at));
  day
    .get(..10)
    .expect("date")
    .split('-')
    .fold(root.join("sessions"), |dir, part| dir.join(part))
}

fn codex_session(root: &Path, worktree: &str, at: u64, id: &str) {
  let dir = codex_day(root, at);
  std::fs::create_dir_all(&dir).expect("dir");
  let line = json!({"type": "session_meta", "payload": {"id": id, "cwd": worktree}});
  std::fs::write(dir.join(format!("{id}.jsonl")), format!("{line}\n")).expect("session");
}

fn cursor_chat(config: &Path, worktree: &str, id: &str, meta: Option<serde_json::Value>) {
  let chat = config
    .join("chats")
    .join(md5_hex(worktree.as_bytes()))
    .join(id);
  std::fs::create_dir_all(&chat).expect("chat");
  if let Some(meta) = meta {
    std::fs::write(chat.join("meta.json"), meta.to_string()).expect("meta");
  }
}

#[test]
fn cursor_chat_metadata_attests_only_the_exact_session() {
  let tmp = tempfile::tempdir().expect("temp");
  let (since, wt) = (since(), tmp.path().join("project").display().to_string());
  let config = tmp.path().join("cursor");
  cursor_chat(
    &config,
    &wt,
    FIRST,
    Some(json!({"createdAtMs": since + 300, "cwd": wt})),
  );
  let env = env(&[("CURSOR_CONFIG_DIR", config.display().to_string())]);
  assert_eq!(
    capture_session(Cursor, &wt, since, &env),
    Ok(Some(FIRST.to_owned()))
  );
  assert_eq!(
    require_captured_session(Cursor, &wt, since, FIRST, &env),
    Ok(())
  );
  assert_eq!(
    require_captured_session(Cursor, &wt, since, SECOND, &env),
    Err("started agent did not attach the recorded session".to_owned())
  );
}

#[test]
fn cursor_capture_skips_older_foreign_unwritten_and_ambiguous_chats() {
  let tmp = tempfile::tempdir().expect("temp");
  let (since, wt) = (since(), tmp.path().join("project").display().to_string());
  let config = tmp.path().join("xdg").join("cursor");
  let env = env(&[(
    "XDG_CONFIG_HOME",
    tmp.path().join("xdg").display().to_string(),
  )]);
  assert_eq!(capture_session(Cursor, &wt, since, &env), Ok(None));
  cursor_chat(
    &config,
    &wt,
    FIRST,
    Some(json!({"createdAtMs": since + 10, "cwd": wt})),
  );
  cursor_chat(
    &config,
    &wt,
    SECOND,
    Some(json!({"createdAtMs": since - 60_000, "cwd": wt})),
  );
  cursor_chat(
    &config,
    &wt,
    "0199a272-0b4e-7cc3-8c4f-071c81275657",
    Some(json!({"createdAtMs": since + 10, "cwd": format!("{wt}/other")})),
  );
  cursor_chat(&config, &wt, "0199a272-0b4e-7cc3-8c4f-071c81275658", None);
  cursor_chat(
    &config,
    &wt,
    "not-a-chat-id",
    Some(json!({"createdAtMs": since + 10, "cwd": wt})),
  );
  assert_eq!(
    capture_session(Cursor, &wt, since, &env),
    Ok(Some(FIRST.to_owned()))
  );
  cursor_chat(
    &config,
    &wt,
    "0199a272-0b4e-7cc3-8c4f-071c81275659",
    Some(json!({"createdAtMs": since + 20, "cwd": wt})),
  );
  assert_eq!(capture_session(Cursor, &wt, since, &env), Ok(None));
}

#[test]
fn codex_metadata_attests_one_session_and_never_guesses() {
  let tmp = tempfile::tempdir().expect("temp");
  let (since, wt) = (since(), tmp.path().join("project").display().to_string());
  let root = tmp.path().join("codex");
  codex_session(&root, &wt, since, FIRST);
  let env = env(&[("CODEX_HOME", root.display().to_string())]);
  assert_eq!(
    capture_session(Codex, &wt, since, &env),
    Ok(Some(FIRST.to_owned()))
  );
  let dir = codex_day(&root, since);
  std::os::unix::fs::symlink(root.join("rotated.jsonl"), dir.join("rotated.jsonl"))
    .expect("dangling");
  assert_eq!(
    capture_session(Codex, &wt, since, &env),
    Ok(Some(FIRST.to_owned()))
  );
  codex_session(&root, &wt, since, SECOND);
  assert_eq!(capture_session(Codex, &wt, since, &env), Ok(None));
}

/// Codex files a session under its local date, which is the UTC date one day
/// before (west of UTC late in the day) or after (far east) the launch.
#[test]
fn codex_capture_finds_a_session_filed_under_a_local_date() {
  const DAY: u64 = 86_400_000;
  for at in [since() - DAY, since() + DAY] {
    let tmp = tempfile::tempdir().expect("temp");
    let (since, wt) = (since(), tmp.path().join("project").display().to_string());
    let root = tmp.path().join("codex");
    let env = env(&[("CODEX_HOME", root.display().to_string())]);
    codex_session(&root, &wt, at, FIRST);
    assert_eq!(
      capture_session(Codex, &wt, since, &env),
      Ok(Some(FIRST.to_owned()))
    );
    codex_session(&root, &wt, since, SECOND);
    assert_eq!(capture_session(Codex, &wt, since, &env), Ok(None));
  }
}

/// Older rollouts across the day window never count toward the file cap.
#[test]
fn codex_capture_ignores_older_rollouts_past_the_file_cap() {
  let tmp = tempfile::tempdir().expect("temp");
  let (since, wt) = (since(), tmp.path().join("project").display().to_string());
  let root = tmp.path().join("codex");
  let env = env(&[("CODEX_HOME", root.display().to_string())]);
  let old = SystemTime::UNIX_EPOCH + Duration::from_millis(since - 3_600_000);
  for n in 0..=MAX_FILES {
    let id = format!("0199a272-0b4e-7cc3-8c4f-{n:012}");
    codex_session(&root, &wt, since, &id);
    std::fs::File::options()
      .write(true)
      .open(codex_day(&root, since).join(format!("{id}.jsonl")))
      .and_then(|file| file.set_modified(old))
      .expect("age");
  }
  assert_eq!(capture_session(Codex, &wt, since, &env), Ok(None));
  codex_session(&root, &wt, since, FIRST);
  assert_eq!(
    capture_session(Codex, &wt, since, &env),
    Ok(Some(FIRST.to_owned()))
  );
}

#[test]
fn claude_transcripts_name_the_worktree() {
  let tmp = tempfile::tempdir().expect("temp");
  let (since, wt) = (since(), tmp.path().join("project").display().to_string());
  let claude = tmp.path().join("claude");
  let slug: String = wt
    .chars()
    .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
    .collect();
  let dir = claude.join("projects").join(slug);
  std::fs::create_dir_all(&dir).expect("dir");
  std::fs::write(
    dir.join("s.jsonl"),
    format!("not json\n{}\n", json!({"cwd": wt, "sessionId": FIRST})),
  )
  .expect("transcript");
  let env = env(&[("CLAUDE_CONFIG_DIR", claude.display().to_string())]);
  assert_eq!(
    capture_session(Claude, &wt, since, &env),
    Ok(Some(FIRST.to_owned()))
  );
  assert_eq!(
    capture_session(Claude, &wt, since + 3_600_000, &env),
    Ok(None)
  );
  let none = await_session(
    Claude,
    &wt,
    since + 3_600_000,
    &env,
    Duration::from_millis(10),
  )
  .expect_err("none");
  assert_eq!(
    none.message,
    "cannot attest claude session identity; inspect the owned pane before retry"
  );
}

#[test]
fn a_session_file_head_reads_only_its_first_64_kib() {
  let tmp = tempfile::tempdir().expect("temp");
  let path = tmp.path().join("big.jsonl");
  std::fs::write(&path, "a".repeat(200_000)).expect("write");
  assert_eq!(
    super::head(&path).map(|text| text.map(|text| text.len())),
    Ok(Some(65_536))
  );
  assert_eq!(super::head(&tmp.path().join("gone.jsonl")), Ok(None));
}
