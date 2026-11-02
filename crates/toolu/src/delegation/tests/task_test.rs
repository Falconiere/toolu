use super::*;
use crate::delegation::delegate_command;

#[test]
fn worker_reports_require_a_clean_bound_commit_and_replay_identically() {
  let dir = tempfile::tempdir().unwrap();
  let repo = dir.path().join("repo");
  std::fs::create_dir(&repo).unwrap();
  let env = Env::process();
  let repo = repo.to_str().unwrap();
  git(&env, repo, &["init", "--quiet"]).unwrap();
  git(
    &env,
    repo,
    &[
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "--allow-empty",
      "-m",
      "input",
    ],
  )
  .unwrap();
  let head = git(&env, repo, &["rev-parse", "HEAD"]).unwrap();
  let file = dir.path().join("report.json");
  let matches = delegate_command()
    .try_get_matches_from([
      "delegate",
      "report",
      "--file",
      file.to_str().unwrap(),
      "--repo",
      repo,
      "--attempt",
      "unique-attempt",
      "--base",
      &head,
      "--commit",
      &head,
      "--status",
      "done",
    ])
    .unwrap();
  let (_, args) = matches.subcommand().unwrap();
  let first = report(args, &env).unwrap();
  assert_eq!(report(args, &env).unwrap(), first);
  std::fs::write(std::path::Path::new(repo).join("dirty"), "work").unwrap();
  assert!(report(args, &env).is_err());
  assert_eq!(
    serde_json::from_slice::<Value>(&std::fs::read(file).unwrap()).unwrap(),
    first
  );
}
