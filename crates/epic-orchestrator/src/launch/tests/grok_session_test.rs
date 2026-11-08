use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use toolu_runtime::env::Env;

use super::super::session::{capture_session, require_captured_session};
use super::{grok_home, group};
use crate::hosts::HostKind::Grok;

/// The probe sessions the fixtures were trimmed from: a headless session and
/// the subagent it spawned, both in `WORKSPACE`, and an interactive session
/// herdr started in `PROBE`.
const ID: &str = "01a1bd47-c3a6-7f12-8ea9-93ee7d8e2e1f";
const CHILD: &str = "01a1bd47-d8a4-7a30-ae61-170f1a6f55cd";
const WORKSPACE: &str = "/private/tmp/grok-subagent-846";
const PROBE_ID: &str = "01a1bd42-396e-7cb3-9e06-4c45a5eda666";
const PROBE: &str = "/private/tmp/grok-probe-846";
const OTHER: &str = "01a1bd45-5cfe-7592-be63-e8ccca2c0baf";

fn real_summary(cwd: &str, id: &str) -> String {
  let encoded = cwd.replace('/', "%2F");
  let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../fixtures/epic/tokens/grok/sessions")
    .join(encoded)
    .join(id)
    .join("summary.json");
  std::fs::read_to_string(path).expect("fixture")
}

fn since() -> u64 {
  toolu_state::time::epoch_millis(SystemTime::now()) - 1_000
}

fn env(home: &Path) -> Env {
  Env::from_pairs([
    ("HOME", "/nonexistent".to_owned()),
    ("GROK_HOME", home.display().to_string()),
  ])
}

/// Lay `text` out as the summary of session directory `dir` of `cwd`.
fn session(env: &Env, cwd: &str, dir: &str, text: &str) -> PathBuf {
  let path = group(env, cwd).join(dir).join("summary.json");
  std::fs::create_dir_all(path.parent().expect("dir")).expect("mkdir");
  std::fs::write(&path, text).expect("summary");
  path
}

#[test]
fn the_top_level_session_is_captured_and_its_subagent_ignored() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env(tmp.path());
  let since = since();
  session(&env, WORKSPACE, ID, &real_summary(WORKSPACE, ID));
  session(&env, WORKSPACE, CHILD, &real_summary(WORKSPACE, CHILD));
  assert_eq!(
    capture_session(Grok, WORKSPACE, since, &env),
    Ok(Some(ID.to_owned()))
  );
  assert_eq!(
    require_captured_session(Grok, WORKSPACE, since, ID, &env),
    Ok(())
  );
  assert_eq!(
    require_captured_session(Grok, WORKSPACE, since, CHILD, &env),
    Err("started agent did not attach the recorded session".to_owned())
  );
  assert_eq!(
    capture_session(Grok, "/private/tmp/elsewhere", since, &env),
    Ok(None)
  );
}

#[test]
fn an_interactive_session_herdr_started_is_captured() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env(tmp.path());
  session(&env, PROBE, PROBE_ID, &real_summary(PROBE, PROBE_ID));
  assert_eq!(
    capture_session(Grok, PROBE, since(), &env),
    Ok(Some(PROBE_ID.to_owned()))
  );
}

#[test]
fn older_foreign_misfiled_and_ambiguous_sessions_are_no_answer() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env(tmp.path());
  let since = since();
  assert_eq!(capture_session(Grok, PROBE, since, &env), Ok(None));
  let old = session(&env, PROBE, PROBE_ID, &real_summary(PROBE, PROBE_ID));
  let before = SystemTime::now() - Duration::from_secs(600);
  File::options()
    .write(true)
    .open(&old)
    .and_then(|file| file.set_modified(before))
    .expect("mtime");
  assert_eq!(capture_session(Grok, PROBE, since, &env), Ok(None));
  // A summary naming another session, one of another cwd, a non-UUID
  // directory and an unparseable summary.
  session(&env, PROBE, OTHER, &real_summary(PROBE, PROBE_ID));
  session(&env, PROBE, ID, &real_summary(WORKSPACE, ID));
  session(&env, PROBE, "not-a-session", &real_summary(PROBE, PROBE_ID));
  session(&env, PROBE, CHILD, "{");
  assert_eq!(capture_session(Grok, PROBE, since, &env), Ok(None));
  session(&env, PROBE, PROBE_ID, &real_summary(PROBE, PROBE_ID));
  assert_eq!(
    capture_session(Grok, PROBE, since, &env),
    Ok(Some(PROBE_ID.to_owned()))
  );
  session(
    &env,
    PROBE,
    OTHER,
    &real_summary(PROBE, PROBE_ID).replace(PROBE_ID, OTHER),
  );
  assert_eq!(capture_session(Grok, PROBE, since, &env), Ok(None));
}

#[test]
fn the_group_is_the_cwd_grok_encoded() {
  let env = Env::from_pairs([("HOME", "/home/user"), ("GROK_HOME", " ")]);
  assert_eq!(grok_home(&env), PathBuf::from("/home/user/.grok"));
  // Grok 1.0.77 named this directory for the cwd beside it.
  assert_eq!(
    group(&env, "/private/tmp/grok enc_~+@,=#%x.y-z"),
    PathBuf::from(
      "/home/user/.grok/sessions/%2Fprivate%2Ftmp%2Fgrok%20enc_~%2B%40%2C%3D%23%25x.y-z"
    )
  );
  let set = Env::from_pairs([("HOME", "/home/user"), ("GROK_HOME", "/g")]);
  assert_eq!(group(&set, "/a"), PathBuf::from("/g/sessions/%2Fa"));
}
