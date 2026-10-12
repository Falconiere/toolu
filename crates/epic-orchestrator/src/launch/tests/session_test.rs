use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde_json::json;
use toolu_runtime::env::Env;

use super::{await_session, capture_session, require_captured_session};
use crate::hosts::HostKind::{Claude, Codex, Cursor, Opencode};
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

fn codex_session(root: &Path, worktree: &str, at: u64, id: &str) {
  let day = toolu_state::time::iso_seconds(SystemTime::UNIX_EPOCH + Duration::from_millis(at));
  let dir = day
    .get(..10)
    .expect("date")
    .split('-')
    .fold(root.join("sessions"), |dir, part| dir.join(part));
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
  let day = toolu_state::time::iso_seconds(SystemTime::UNIX_EPOCH + Duration::from_millis(since));
  let dir = day
    .get(..10)
    .expect("date")
    .split('-')
    .fold(root.join("sessions"), |dir, part| dir.join(part));
  std::os::unix::fs::symlink(root.join("rotated.jsonl"), dir.join("rotated.jsonl"))
    .expect("dangling");
  assert_eq!(
    capture_session(Codex, &wt, since, &env),
    Ok(Some(FIRST.to_owned()))
  );
  codex_session(&root, &wt, since, SECOND);
  assert_eq!(capture_session(Codex, &wt, since, &env), Ok(None));
}

#[test]
fn claude_transcripts_and_opencode_sessions_name_the_worktree() {
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
  let bin = tmp.path().join("bin");
  std::fs::create_dir_all(&bin).expect("bin");
  std::fs::create_dir_all(&wt).expect("worktree");
  let list = json!([{"id": "ses_abc", "directory": wt, "created": since + 5}, {"id": "ses_old", "directory": wt, "created": since - 5}]);
  let script = bin.join("opencode");
  std::fs::write(&script, format!("#!/bin/sh\necho '{list}'\n")).expect("script");
  std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
  let path = env
    .clone()
    .with("PATH", &format!("{}:/usr/bin:/bin", bin.display()));
  assert_eq!(
    capture_session(Opencode, &wt, since, &path),
    Ok(Some("ses_abc".to_owned()))
  );
  let none =
    await_session(Opencode, &wt, since, &env, Duration::from_millis(10)).expect_err("none");
  assert_eq!(
    none.message,
    "cannot attest opencode session identity; inspect the owned pane before retry"
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
