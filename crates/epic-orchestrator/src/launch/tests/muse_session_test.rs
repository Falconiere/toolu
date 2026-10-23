use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use toolu_runtime::env::Env;

use super::super::session::{capture_session, require_captured_session};
use super::data_root;
use crate::hosts::HostKind::Muse;

/// The probe session the fixture was trimmed from, and its workspace.
const ID: &str = "01a16abb-7eef-7340-8cf2-2e0378dcf66c";
const WORKSPACE: &str = "/private/tmp/muse-probe-603";
const OTHER: &str = "01a16abe-bf40-7a42-a923-df25d8809d3d";

fn real_log() -> String {
  let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/muse/session.jsonl");
  std::fs::read_to_string(path).expect("fixture")
}

fn since() -> u64 {
  toolu_state::time::epoch_millis(SystemTime::now()) - 1_000
}

/// `<data>/sessions/<yyyy>/<mm>/<dd>` of `at` (UTC).
fn day(data: &Path, at: u64) -> PathBuf {
  let iso = toolu_state::time::iso_seconds(SystemTime::UNIX_EPOCH + Duration::from_millis(at));
  iso
    .get(..10)
    .expect("date")
    .split('-')
    .fold(data.join("sessions"), |dir, part| dir.join(part))
}

/// Lay `text` out as session `dir` of the day of `at`.
fn session(data: &Path, at: u64, dir: &str, text: &str) -> PathBuf {
  let path = day(data, at).join(dir).join("session.jsonl");
  std::fs::create_dir_all(path.parent().expect("dir")).expect("mkdir");
  std::fs::write(&path, text).expect("log");
  path
}

fn env(xdg: &Path) -> Env {
  Env::from_pairs([
    ("HOME", "/nonexistent".to_owned()),
    ("XDG_DATA_HOME", xdg.display().to_string()),
  ])
}

#[test]
fn a_real_muse_session_is_captured_by_its_metadata() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env(tmp.path());
  let since = since();
  session(&data_root(&env), since, ID, &real_log());
  assert_eq!(
    capture_session(Muse, WORKSPACE, since, &env),
    Ok(Some(ID.to_owned()))
  );
  assert_eq!(
    require_captured_session(Muse, WORKSPACE, since, ID, &env),
    Ok(())
  );
  assert_eq!(
    require_captured_session(Muse, WORKSPACE, since, OTHER, &env),
    Err("started agent did not attach the recorded session".to_owned())
  );
  assert_eq!(
    capture_session(Muse, "/private/tmp/elsewhere", since, &env),
    Ok(None)
  );
}

#[test]
fn older_misfiled_and_ambiguous_sessions_are_no_answer() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env(tmp.path());
  let data = data_root(&env);
  let since = since();
  assert_eq!(capture_session(Muse, WORKSPACE, since, &env), Ok(None));
  let old = session(&data, since, ID, &real_log());
  let before = SystemTime::now() - Duration::from_secs(600);
  File::options()
    .write(true)
    .open(&old)
    .and_then(|file| file.set_modified(before))
    .expect("mtime");
  assert_eq!(capture_session(Muse, WORKSPACE, since, &env), Ok(None));
  // A log whose metadata names another session, and a non-UUID directory.
  session(&data, since, OTHER, &real_log());
  session(&data, since, "not-a-session", &real_log());
  assert_eq!(capture_session(Muse, WORKSPACE, since, &env), Ok(None));
  session(&data, since, ID, &real_log());
  assert_eq!(
    capture_session(Muse, WORKSPACE, since, &env),
    Ok(Some(ID.to_owned()))
  );
  session(&data, since, OTHER, &real_log().replace(ID, OTHER));
  assert_eq!(capture_session(Muse, WORKSPACE, since, &env), Ok(None));
}

#[test]
fn a_session_filed_under_the_next_local_day_is_found() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env(tmp.path());
  let since = since();
  session(&data_root(&env), since + 86_400_000, ID, &real_log());
  assert_eq!(
    capture_session(Muse, WORKSPACE, since, &env),
    Ok(Some(ID.to_owned()))
  );
}

#[test]
fn the_data_root_falls_back_to_the_home_share_directory() {
  let home = Env::from_pairs([("HOME", "/home/user"), ("XDG_DATA_HOME", " ")]);
  assert_eq!(
    data_root(&home),
    PathBuf::from("/home/user/.local/share/muse")
  );
  let xdg = Env::from_pairs([("HOME", "/home/user"), ("XDG_DATA_HOME", "/data")]);
  assert_eq!(data_root(&xdg), PathBuf::from("/data/muse"));
}
