use super::super::repos::records;
use super::super::support::Repo;
use super::{stale, sweep};

#[test]
fn busy_records_whose_worktree_is_gone_are_reported_with_their_fix() {
  let repo = Repo::new().unwrap();
  repo.record("gone", "running", &[]).unwrap();
  repo.record("alive", "running", &[]).unwrap();
  std::fs::create_dir_all(repo.tmp.join(".herdr/worktrees/repo/alive")).unwrap();
  repo.record("over", "merged", &[]).unwrap();
  let found = records(&[repo.tmp.join("epic")]);
  let rows = stale(&found);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0]["key"], "gone");
  assert_eq!(
    rows[0]["fix"],
    format!(
      "toolu epic finish --abandon {} gone",
      repo.tmp.join("epic").display()
    )
  );
  let report = sweep(&repo.request(true), &found);
  assert_eq!(report["job_logs"], 0);
  assert_eq!(report["job_logs_removed"], 0);
  assert_eq!(report["stale_records"].as_array().unwrap().len(), 1);
}
