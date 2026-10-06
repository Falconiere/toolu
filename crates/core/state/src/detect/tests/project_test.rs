use std::path::{Path, PathBuf};
use std::process::Command;

use toolu_runtime::env::Env;

use super::{
  NodePackageManager, PythonLinter, TsLinter, detect_clippy, detect_python, detect_rust, detect_ts,
  node_package_manager, project_name, project_toplevel, python_linter, to_relative_path, ts_linter,
};

/// A scratch directory holding a git repository named `project`.
struct Repo {
  _dir: tempfile::TempDir,
  root: PathBuf,
  env: Env,
}

impl Repo {
  /// An empty repository, probed with a `PATH` that finds git.
  fn new() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap().join("project");
    std::fs::create_dir(&root).unwrap();
    let path = std::env::var("PATH").unwrap();
    let env = Env::from_pairs([
      ("PATH", path.as_str()),
      ("HOME", dir.path().to_str().unwrap()),
    ]);
    let repo = Repo {
      _dir: dir,
      root,
      env,
    };
    repo.git(&["init", "-q"]);
    repo
  }

  fn git(&self, args: &[&str]) {
    let status = Command::new("git")
      .args(args)
      .current_dir(&self.root)
      .status()
      .unwrap();
    assert!(status.success(), "git {args:?}");
  }

  fn write(&self, rel: &str, body: &str) {
    let path = self.root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
  }

  /// Every probe from `cwd`, as printable words.
  fn probe(&self, cwd: &Path) -> Vec<String> {
    let env = &self.env;
    vec![
      project_toplevel(env, cwd)
        .map(|root| root.display().to_string())
        .unwrap_or_default(),
      project_name(env, cwd).unwrap_or_default(),
      node_package_manager(env, cwd)
        .map(|pm| pm.name().to_owned())
        .unwrap_or_default(),
      detect_rust(env, cwd).to_string(),
      detect_python(env, cwd).to_string(),
      detect_ts(env, cwd).to_string(),
      ts_linter(env, cwd)
        .map(|linter| linter.name().to_owned())
        .unwrap_or_default(),
      python_linter(env, cwd)
        .map(|linter| linter.name().to_owned())
        .unwrap_or_default(),
      detect_clippy(env, cwd).to_string(),
    ]
  }
}

#[test]
fn an_empty_repository_has_a_name_and_no_markers() {
  let repo = Repo::new();
  let root = repo.root.display().to_string();
  let expected = [
    root.as_str(),
    "project",
    "",
    "false",
    "false",
    "false",
    "",
    "",
    "false",
  ];
  assert_eq!(repo.probe(&repo.root), expected);
}

#[test]
fn markers_and_linters_are_read_from_the_root_from_any_subdirectory() {
  let repo = Repo::new();
  for (name, body) in [
    ("bun.lock", ""),
    ("Cargo.toml", "[package]\n"),
    ("setup.cfg", ""),
  ] {
    repo.write(name, body);
  }
  repo.write("biome.json", "{}");
  repo.write("pyproject.toml", "[project]\n[tool.ruff.lint]\n");
  repo.write(".clippy.toml", "");
  repo.write("packages/a/tsconfig.base.json", "{}");
  repo.git(&["add", "-A"]);
  let sub = repo.root.join("sub/dir");
  std::fs::create_dir_all(&sub).unwrap();
  let root = repo.root.display().to_string();
  let expected = [
    root.as_str(),
    "project",
    "bun",
    "true",
    "true",
    "true",
    "biome",
    "ruff",
    "true",
  ];
  assert_eq!(repo.probe(&repo.root), expected);
  assert_eq!(repo.probe(&sub), expected);
}

#[test]
fn an_untracked_tsconfig_is_not_typescript_and_git_failing_is_not_either() {
  let repo = Repo::new();
  repo.write("tsconfig.json", "{}");
  assert!(!detect_ts(&repo.env, &repo.root));
  repo.git(&["add", "tsconfig.json"]);
  assert!(detect_ts(&repo.env, &repo.root));
  // The toplevel comes from `.git`, so only `ls-files` needs a git on PATH.
  let no_git = Env::from_pairs([("PATH", "")]);
  assert!(!detect_ts(&no_git, &repo.root));
}

#[test]
fn lock_files_linters_and_ruff_sections_follow_bashs_precedence() {
  let repo = Repo::new();
  for name in [
    "yarn.lock",
    "package-lock.json",
    ".oxlintrc.json",
    "eslint.config.mjs",
  ] {
    repo.write(name, "");
  }
  repo.write("pyproject.toml", "  [tool.ruff]\nx = 1\r[tool.ruff]\n");
  let (env, root) = (&repo.env, repo.root.as_path());
  assert_eq!(
    node_package_manager(env, root),
    Some(NodePackageManager::Yarn)
  );
  assert_eq!(ts_linter(env, root), Some(TsLinter::Oxc));
  assert_eq!(python_linter(env, root), None);
  std::fs::remove_file(repo.root.join(".oxlintrc.json")).unwrap();
  assert_eq!(ts_linter(env, root), Some(TsLinter::Eslint));
  repo.write("pyproject.toml", "[project]\n[tool.ruff]\n");
  assert_eq!(python_linter(env, root), Some(PythonLinter::Ruff));
  assert_eq!(PythonLinter::Ruff.name(), "ruff");
  let managers = [
    NodePackageManager::Bun,
    NodePackageManager::Pnpm,
    NodePackageManager::Npm,
  ];
  assert_eq!(
    managers.map(NodePackageManager::name),
    ["bun", "pnpm", "npm"]
  );
}

#[test]
fn outside_a_repository_every_probe_is_empty() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
  let env = Env::from_pairs([("PATH", std::env::var("PATH").unwrap())]);
  let repo = Repo {
    _dir: tempfile::tempdir().unwrap(),
    root: dir.path().to_path_buf(),
    env,
  };
  assert_eq!(repo.probe(dir.path()).concat(), "falsefalsefalsefalse");
}

#[test]
fn a_path_is_relative_to_the_toplevel_only_when_under_it() {
  let repo = Repo::new();
  let root = repo.root.display().to_string();
  let inputs = [
    format!("{root}/src/a.ts"),
    format!("{root}/deep/x/y.rs"),
    "src/a.ts".to_owned(),
    String::new(),
    root.clone(),
    format!("{root}/"),
    format!("{root}-sibling/a.ts"),
  ];
  let relative: Vec<String> = inputs
    .iter()
    .map(|path| to_relative_path(path, &repo.env, &repo.root))
    .collect();
  assert_eq!(relative[..2], ["src/a.ts", "deep/x/y.rs"]);
  assert_eq!(
    relative[2..],
    ["src/a.ts", "", root.as_str(), "", inputs[6].as_str()]
  );
  let outside = tempfile::tempdir().unwrap();
  let moved = to_relative_path(&inputs[0], &repo.env, outside.path());
  assert_eq!(moved, inputs[0]);
}
