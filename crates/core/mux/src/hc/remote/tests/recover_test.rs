use super::super::local::local_git;
use super::{export_recover, import_recover};
use toolu_runtime::env::Env;

fn git(repo: &str, args: &[&str]) -> String {
  local_git(&Env::process(), repo, args).unwrap()
}

#[test]
fn retries_reuse_only_matching_git_refs_and_bytes() {
  let a = tempfile::tempdir().unwrap();
  let b = tempfile::tempdir().unwrap();
  let src = a.path().to_str().unwrap();
  let dst = b.path().to_str().unwrap();
  for repo in [src, dst] {
    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.name", "Test"]);
    git(repo, &["config", "user.email", "t@example.invalid"]);
  }
  std::fs::write(a.path().join("file"), b"snapshot").unwrap();
  git(src, &["add", "file"]);
  git(src, &["commit", "-qm", "one"]);
  let sha = git(src, &["rev-parse", "HEAD"]);
  let env = Env::process();
  let bytes = export_recover(&env, src, "run", &sha).unwrap();
  assert_eq!(export_recover(&env, src, "run", &sha).unwrap(), bytes);
  git(
    src,
    &["update-ref", "-d", "refs/toolu-transfer/run/snapshot"],
  );
  assert_eq!(export_recover(&env, src, "run", &sha).unwrap(), bytes);
  import_recover(&env, dst, "run", &sha, &bytes).unwrap();
  import_recover(&env, dst, "run", &sha, &bytes).unwrap();
  git(
    dst,
    &["update-ref", "-d", "refs/toolu-transfer/run/snapshot"],
  );
  import_recover(&env, dst, "run", &sha, &bytes).unwrap();
  let mut changed = bytes.clone();
  changed.push(0);
  assert!(import_recover(&env, dst, "run", &sha, &changed).is_err());
  let wrong = "a".repeat(40);
  assert!(export_recover(&env, src, "run", &wrong).is_err());
  assert!(import_recover(&env, dst, "run", &wrong, &bytes).is_err());
  assert_eq!(
    git(dst, &["rev-parse", "refs/toolu-transfer/run/snapshot"]),
    sha
  );
}

#[test]
fn large_binary_bundle_round_trips_and_recovers() {
  let source = tempfile::tempdir().unwrap();
  let destination = tempfile::tempdir().unwrap();
  let src = source.path().to_str().unwrap();
  let dst = destination.path().to_str().unwrap();
  for repo in [src, dst] {
    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.name", "Test"]);
    git(repo, &["config", "user.email", "t@example.invalid"]);
  }
  let mut seed = 0x1234_5678_u32;
  let content: Vec<u8> = (0..10 * 1024 * 1024)
    .map(|_| {
      seed ^= seed << 13;
      seed ^= seed >> 17;
      seed ^= seed << 5;
      seed.to_le_bytes()[0]
    })
    .collect();
  std::fs::write(source.path().join("binary"), content).unwrap();
  git(src, &["add", "binary"]);
  git(src, &["commit", "-qm", "large"]);
  let sha = git(src, &["rev-parse", "HEAD"]);
  let env = Env::process();
  let direct = super::super::local::export(&env, src, "direct", &sha).unwrap();
  super::super::local::import(&env, dst, "direct", &sha, &direct).unwrap();
  assert!(direct.len() > 8 * 1024 * 1024);
  let bytes = export_recover(&env, src, "large", &sha).unwrap();
  assert!(bytes.len() > 8 * 1024 * 1024);
  assert_eq!(export_recover(&env, src, "large", &sha).unwrap(), bytes);
  import_recover(&env, dst, "large", &sha, &bytes).unwrap();
  import_recover(&env, dst, "large", &sha, &bytes).unwrap();
  assert_eq!(
    git(dst, &["rev-parse", "refs/toolu-transfer/large/snapshot"]),
    sha
  );
}
