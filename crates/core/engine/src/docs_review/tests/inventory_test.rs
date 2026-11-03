use crate::docs_review::{
  inspect,
  tests::{fixture, git},
};
use toolu_protocol::host::Host;
use toolu_runtime::{env::Env, host::roots::Roots};

#[cfg(unix)]
#[test]
fn inventory_uses_target_config_and_only_committed_regular_documentation() {
  let (dir, _) = fixture();
  let repo = dir.path();
  std::fs::create_dir_all(repo.join(".claude")).unwrap();
  std::fs::write(
    repo.join(".claude/toolu.config.json"),
    r#"{"docsSync":{"surfaceExcludes":["private/*"]}}"#,
  )
  .unwrap();
  for path in [
    "private/hidden.md",
    "docs/toolu/plans/p.md",
    "docs/toolu/brainstorms/b.md",
    "docs/toolu/specs/s.md",
    "docs/toolu/decisions/d.md",
    "skills/worker/SKILL.md",
    "CAPS.MD",
  ] {
    let file = repo.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, "documentation\n").unwrap();
  }
  std::os::unix::fs::symlink("GUIDE.md", repo.join("link.md")).unwrap();
  git(repo, &["add", "."]);
  git(repo, &["commit", "-m", "add documentation"]);
  std::fs::write(repo.join("untracked.md"), "untracked\n").unwrap();
  let elsewhere = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(elsewhere.path().join(".claude")).unwrap();
  std::fs::write(
    elsewhere.path().join(".claude/toolu.config.json"),
    r#"{"docsSync":{"surfaceExcludes":["CAPS.MD"]}}"#,
  )
  .unwrap();
  let roots = Roots::new(
    Env::from_pairs([
      ("HOME", repo.to_str().unwrap()),
      ("DOCS_SYNC_BASE", "main"),
      ("TOOLU_PROJECT_DIR", elsewhere.path().to_str().unwrap()),
    ]),
    Some(Host::Claude),
  );
  let inspection = inspect(&roots, repo, None).unwrap();
  let inventory = inspection["documentation_files"]
    .as_object()
    .expect("inventory");
  for path in ["README.md", "GUIDE.md", "CAPS.MD", "skills/worker/SKILL.md"] {
    assert!(inventory.contains_key(path), "missing {path}");
  }
  for path in [
    "private/hidden.md",
    "docs/toolu/plans/p.md",
    "docs/toolu/brainstorms/b.md",
    "docs/toolu/specs/s.md",
    "docs/toolu/decisions/d.md",
    "link.md",
    "untracked.md",
  ] {
    assert!(!inventory.contains_key(path), "unexpected {path}");
  }
}
