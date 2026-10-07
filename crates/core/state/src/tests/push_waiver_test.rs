use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{push_waiver_dir, push_waiver_path, push_waiver_pending_path, push_waiver_promote};
use crate::ctx::StateCtx;

/// 2026-10-07T12:00:00Z.
const NOW: u64 = 1_791_374_400;
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn ctx(env: Env, host: Host) -> StateCtx {
  let mut ctx = StateCtx::new(Roots::new(env, Some(host)));
  ctx.now = Some(UNIX_EPOCH + Duration::from_secs(NOW));
  ctx
}

fn claude() -> StateCtx {
  ctx(Env::from_pairs([("HOME", "/nonexistent")]), Host::Claude)
}

fn pending(root: &Path, body: &str) -> PathBuf {
  let path = root.join(".claude/tmp/push-review/feat_x.pending-waiver.json");
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(&path, body).unwrap();
  path
}

/// A marker as `pushWaiverPend` writes it: `jq -c` keys in `WaiverFile` order.
fn marker(sha: &str) -> String {
  format!(
    "{{\"version\":1,\"branch\":\"feat_x\",\"diff_sha\":\"{sha}\",\"base_branch\":\"main\",\"reason_code\":\"no-state\",\"asked_at\":\"2026-10-07T11:00:00Z\"}}\n"
  )
}

#[test]
fn the_dir_is_state_dir_else_the_push_review_dir_of_the_root() {
  let root = Path::new("/p");
  let claude = claude();
  assert_eq!(
    push_waiver_dir(&claude.roots, root),
    Some(PathBuf::from("/p/.claude/tmp/push-review"))
  );
  let codex = ctx(Env::from_pairs([("HOME", "/h")]), Host::Codex);
  assert_eq!(
    push_waiver_path(&codex.roots, root, "feat_x"),
    Some(PathBuf::from(
      "/p/.codex/tmp/push-review/feat_x.waiver.json"
    ))
  );
  let overridden = ctx(Env::from_pairs([("STATE_DIR", "/s")]), Host::Claude);
  assert_eq!(
    push_waiver_pending_path(&overridden.roots, root, "feat_x"),
    Some(PathBuf::from("/s/feat_x.pending-waiver.json"))
  );
  let empty = ctx(Env::from_pairs([("STATE_DIR", "")]), Host::Claude);
  assert_eq!(
    push_waiver_dir(&empty.roots, root),
    Some(PathBuf::from("/p/.claude/tmp/push-review")),
    "an empty STATE_DIR is unset"
  );
}

#[test]
fn a_matching_marker_becomes_a_waiver_byte_for_byte() {
  let dir = tempfile::tempdir().unwrap();
  let marker_path = pending(dir.path(), &marker(SHA));
  assert!(push_waiver_promote(&claude(), dir.path(), "feat_x", SHA));
  let waiver = dir
    .path()
    .join(".claude/tmp/push-review/feat_x.waiver.json");
  assert_eq!(
    std::fs::read_to_string(&waiver).unwrap(),
    format!(
      "{{\"version\":1,\"branch\":\"feat_x\",\"diff_sha\":\"{SHA}\",\"base_branch\":\"main\",\"reason_code\":\"no-state\",\"waived_at\":\"2026-10-07T12:00:00Z\"}}\n"
    )
  );
  let mode = std::fs::metadata(&waiver).unwrap().permissions().mode();
  assert_eq!(mode & 0o777, 0o600);
  assert!(!marker_path.exists(), "the pending marker is cashed in");
}

#[test]
fn keys_keep_javascript_order_and_an_existing_waived_at_keeps_its_place() {
  let dir = tempfile::tempdir().unwrap();
  pending(
    dir.path(),
    &format!(
      "{{\"waived_at\":\"old\",\"diff_sha\":\"{SHA}\",\"2\":\"b\",\"version\":\"1\",\"note\":\"\u{7f}\",\"1\":\"a\"}}"
    ),
  );
  assert!(push_waiver_promote(&claude(), dir.path(), "feat_x", SHA));
  let text = std::fs::read_to_string(
    dir
      .path()
      .join(".claude/tmp/push-review/feat_x.waiver.json"),
  );
  assert_eq!(
    text.unwrap(),
    format!(
      "{{\"1\":\"a\",\"2\":\"b\",\"waived_at\":\"2026-10-07T12:00:00Z\",\"diff_sha\":\"{SHA}\",\"version\":\"1\",\"note\":\"\\u007f\"}}\n"
    ),
    "array-index keys first, a string version \"1\" is v1, DEL is escaped as jq does"
  );
}

#[test]
fn only_a_v1_marker_naming_exactly_this_sha_promotes() {
  let other = "ffffffffffffffffffffffffffffffffffffffff";
  let cases = [
    marker(other),
    marker(""),
    marker(SHA).replace("\"version\":1", "\"version\":2"),
    marker(SHA).replace("\"version\":1,", ""),
    marker(SHA).replace("\"version\":1", "\"version\":false"),
    "[1]".to_owned(),
    "not json".to_owned(),
    String::new(),
  ];
  for body in cases {
    let dir = tempfile::tempdir().unwrap();
    let marker_path = pending(dir.path(), &body);
    assert!(
      !push_waiver_promote(&claude(), dir.path(), "feat_x", SHA),
      "{body}"
    );
    assert!(marker_path.exists(), "{body}: the marker stays");
    assert!(
      !dir
        .path()
        .join(".claude/tmp/push-review/feat_x.waiver.json")
        .exists()
    );
  }
}

#[test]
fn nothing_is_promoted_without_a_sha_a_marker_or_a_writable_dir() {
  let dir = tempfile::tempdir().unwrap();
  assert!(
    !push_waiver_promote(&claude(), dir.path(), "feat_x", SHA),
    "no marker"
  );
  pending(dir.path(), &marker(""));
  assert!(
    !push_waiver_promote(&claude(), dir.path(), "feat_x", ""),
    "no sha"
  );
  let marker_dir = dir.path().join("marker");
  std::fs::create_dir_all(marker_dir.join("feat_x.pending-waiver.json")).unwrap();
  let overridden = ctx(
    Env::from_pairs([("STATE_DIR", marker_dir.to_string_lossy().into_owned())]),
    Host::Claude,
  );
  assert!(
    !push_waiver_promote(&overridden, dir.path(), "feat_x", SHA),
    "a directory is no marker"
  );
}

#[test]
fn a_failed_waiver_write_keeps_the_marker() {
  let dir = tempfile::tempdir().unwrap();
  let marker_path = pending(dir.path(), &marker(SHA));
  let waiver = dir
    .path()
    .join(".claude/tmp/push-review/feat_x.waiver.json");
  std::fs::create_dir_all(waiver.join("blocker")).unwrap();
  assert!(!push_waiver_promote(&claude(), dir.path(), "feat_x", SHA));
  assert!(marker_path.exists());
}
