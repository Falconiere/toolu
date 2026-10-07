//! Git facts and `command -v` for session-start (`project-detect.ts`, `bash-compat.ts`).

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use toolu_runtime::config::load::is_file;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Output, Spec, Wait, run};

use crate::lifecycle::text::strip_trailing_newlines;

const LOCKFILES: &[(&str, &str)] = &[
  ("bun.lock", "bun"),
  ("bun.lockb", "bun"),
  ("pnpm-lock.yaml", "pnpm"),
  ("yarn.lock", "yarn"),
  ("package-lock.json", "npm"),
];

const PYTHON_MARKERS: &[&str] = &[
  "pyproject.toml",
  "setup.py",
  "setup.cfg",
  "requirements.txt",
];

/// What session-start names about a git toplevel. All empty outside a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectFacts {
  /// `basename` of the toplevel.
  pub name: String,
  /// The first lockfile's package manager, or empty.
  pub node_pm: String,
  /// A `Cargo.toml` sits at the toplevel.
  pub rust: bool,
  /// `git ls-files` lists a tsconfig. Only looked up when verbose.
  pub ts: bool,
  /// A Python project marker sits at the toplevel.
  pub python: bool,
}

impl ProjectFacts {
  fn empty() -> ProjectFacts {
    ProjectFacts {
      name: String::new(),
      node_pm: String::new(),
      rust: false,
      ts: false,
      python: false,
    }
  }
}

/// `command -v name` over a `PATH` list. A name containing `/` is not a command.
pub(crate) fn on_path(name: &str, path: &str) -> bool {
  if name.is_empty() || name.contains('/') {
    return false;
  }
  path.split(':').filter(|dir| !dir.is_empty()).any(|dir| {
    let candidate = Path::new(dir).join(name);
    std::fs::metadata(&candidate)
      .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
  })
}

/// `git rev-parse --show-toplevel` in `cwd`. Empty when git is missing or fails.
pub(crate) fn git_toplevel(env: &Env, cwd: &Path) -> String {
  let Some(output) = git_output(env, cwd, &["rev-parse", "--show-toplevel"]) else {
    return String::new();
  };
  if output.exit_code != 0 {
    return String::new();
  }
  strip_trailing_newlines(&output.stdout).to_owned()
}

/// `Branch: <name>` from `git rev-parse --abbrev-ref HEAD`. An unborn branch is `HEAD`.
pub(crate) fn branch_line(env: &Env, cwd: &Path) -> Option<String> {
  if !on_path("git", env.get("PATH").unwrap_or("")) {
    return None;
  }
  let output = git_output(env, cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?;
  let branch = strip_trailing_newlines(&output.stdout);
  (!branch.is_empty()).then(|| format!("Branch: {branch}"))
}

/// Facts for git toplevel `root`. `None` and `""` are outside a repository.
pub(crate) fn project_facts(root: Option<&Path>, env: &Env, with_ts: bool) -> ProjectFacts {
  let Some(root) = root.filter(|root| !root.as_os_str().is_empty()) else {
    return ProjectFacts::empty();
  };
  ProjectFacts {
    name: base_name(root),
    node_pm: node_pm(root),
    rust: is_file(&root.join("Cargo.toml")),
    ts: with_ts && tracks_tsconfig(root, env),
    python: PYTHON_MARKERS.iter().any(|file| is_file(&root.join(file))),
  }
}

fn base_name(root: &Path) -> String {
  root
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_default()
}

fn node_pm(root: &Path) -> String {
  LOCKFILES
    .iter()
    .find(|(file, _)| is_file(&root.join(file)))
    .map(|(_, manager)| (*manager).to_owned())
    .unwrap_or_default()
}

/// `git ls-files` printed anything. The bytes are the check, not the stripped text.
fn tracks_tsconfig(root: &Path, env: &Env) -> bool {
  let mut spec = Spec::new(["git", "-C"]);
  spec.argv.push(root.display().to_string());
  spec.argv.extend([
    "ls-files".to_owned(),
    "**/tsconfig*.json".to_owned(),
    "tsconfig*.json".to_owned(),
  ]);
  spec.cwd = Some(root.to_path_buf());
  spec.env = Some(env.clone());
  spec.wait = Wait::Streams;
  run(&spec)
    .is_ok_and(|output| output.exit_code == 0 && !output.timed_out && !output.stdout.is_empty())
}

fn git_output(env: &Env, cwd: &Path, git_args: &[&str]) -> Option<Output> {
  let mut argv = Vec::with_capacity(git_args.len() + 1);
  argv.push("git".to_owned());
  argv.extend(git_args.iter().map(|arg| (*arg).to_owned()));
  let mut spec = Spec::new(argv);
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone());
  spec.wait = Wait::Streams;
  run(&spec).ok().filter(|output| !output.timed_out)
}

#[cfg(test)]
#[path = "tests/project_test.rs"]
mod tests;
