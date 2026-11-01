use super::support::{Repo, git};
use super::{summary, sweep};

#[test]
fn a_full_sweep_reports_each_section_and_its_summary_counts_them() {
  let repo = Repo::new().unwrap();
  let herdr = repo.tmp.join(".herdr/worktrees/repo");
  repo.worktree(&herdr.join("k1"), "feat/k1").unwrap();
  repo.record("k1", "merged", &[]).unwrap();
  let head = git(&repo.env, &repo.root, &["rev-parse", "HEAD"]).unwrap();
  git(
    &repo.env,
    &repo.root,
    &["update-ref", "refs/epic-wip/k1", head.trim()],
  )
  .unwrap();

  let report = sweep(&repo.request(false));
  assert_eq!(report["apply"], false);
  assert_eq!(report["repos"][0]["root"], repo.root.display().to_string());
  assert_eq!(report["repos"][0]["worktrees"][0]["action"], "would-remove");
  let text = summary(&report);
  assert!(
    text.starts_with("would: worktrees=1 branches=0 wip_refs=1"),
    "{text}"
  );

  let applied = sweep(&repo.request(true));
  let text = summary(&applied);
  assert!(
    text
      .starts_with("worktrees=1 branches=1 wip_refs=1 job_logs=0 sandbox=skipped colima=no-colima"),
    "{text}"
  );
  assert!(!herdr.join("k1").exists());
}
