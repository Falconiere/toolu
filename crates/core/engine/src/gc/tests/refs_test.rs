use super::super::repos::records;
use super::super::support::{Repo, git};
use super::{keep_reason, sweep};

#[test]
fn snapshots_of_finished_issues_go_and_the_rest_stay_with_a_reason() {
  let repo = Repo::new().unwrap();
  let head = git(&repo.env, &repo.root, &["rev-parse", "HEAD"]).unwrap();
  for key in ["done", "busy", "dirty", "orphan"] {
    let name = format!("refs/epic-wip/{key}");
    git(&repo.env, &repo.root, &["update-ref", &name, head.trim()]).unwrap();
  }
  repo.record("done", "merged", &[]).unwrap();
  repo.record("busy", "running", &[]).unwrap();
  repo.record("dirty", "abandoned", &["notes.txt"]).unwrap();
  let found = records(&[repo.tmp.join("epic")]);
  assert_eq!(keep_reason("done", &found), None);
  assert_eq!(
    keep_reason("busy", &found).as_deref(),
    Some("the issue is running")
  );
  assert_eq!(
    keep_reason("dirty", &found).as_deref(),
    Some("the snapshot holds 1 leftover files")
  );
  assert_eq!(
    keep_reason("orphan", &found),
    None,
    "no record: the sweep decides"
  );

  let report = sweep(&repo.request(false), &repo.root, &found);
  assert_eq!(report["removed"], serde_json::json!(["refs/epic-wip/done"]));
  assert_eq!(report["kept"].as_array().unwrap().len(), 3);
  let applied = sweep(&repo.request(true), &repo.root, &found);
  assert_eq!(
    applied["removed"],
    serde_json::json!(["refs/epic-wip/done"])
  );
  let left = git(&repo.env, &repo.root, &["for-each-ref", "refs/epic-wip/"]).unwrap();
  assert!(!left.contains("/done") && left.contains("/orphan") && left.contains("/busy"));
}
