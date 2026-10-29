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
  let pending = b.path().join(".git/toolu-transfer/run");
  std::fs::create_dir_all(&pending).unwrap();
  std::fs::write(pending.join("snapshot.bundle"), &bytes[..19]).unwrap();
  import_recover(&env, dst, "run", &sha, &bytes).unwrap();
  assert!(std::fs::read_dir(&pending).unwrap().flatten().any(|entry| {
    std::fs::read(entry.path().join("previous")).is_ok_and(|prior| prior == bytes[..19])
  }));
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

#[test]
fn interrupted_export_build_preserves_old_artifacts_and_resumes() {
  let repo = tempfile::tempdir().unwrap();
  let root = repo.path().to_str().unwrap();
  git(root, &["init", "-q"]);
  git(root, &["config", "user.name", "Test"]);
  git(root, &["config", "user.email", "t@example.invalid"]);
  std::fs::write(repo.path().join("file"), b"snapshot").unwrap();
  git(root, &["add", "file"]);
  git(root, &["commit", "-qm", "snapshot"]);
  let sha = git(root, &["rev-parse", "HEAD"]);
  let old = repo.path().join(".git/toolu-transfer/staged/build");
  std::fs::create_dir_all(&old).unwrap();
  std::fs::write(old.join("bundle"), b"interrupted").unwrap();
  let bytes = export_recover(&Env::process(), root, "staged", &sha).unwrap();
  assert_ne!(bytes, [] as [u8; 0]);
  assert_eq!(std::fs::read(old.join("bundle")).unwrap(), b"interrupted");
  assert_eq!(
    export_recover(&Env::process(), root, "staged", &sha).unwrap(),
    bytes
  );
  let bundle = repo
    .path()
    .join(".git/toolu-transfer/staged/snapshot.bundle");
  let corrupt = &bytes[..bytes.len() - 8];
  std::fs::write(&bundle, corrupt).unwrap();
  assert!(export_recover(&Env::process(), root, "staged", &sha).is_err());
  assert_eq!(std::fs::read(bundle).unwrap(), corrupt);
}
