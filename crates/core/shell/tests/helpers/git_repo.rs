//! Real git repositories laid out the way the bats suites built them, and the
//! `push_target_root` / `push_target_branch` questions replayed on them
//! (`packages/toolu-core/src/detect/detect-git.ts`, `parity-helpers.ts`).

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_shell::analysis::ShellAnalysis;
use toolu_shell::git::push_targets;

use crate::cases::{Res, field};
use crate::lists::strings;

/// A fresh temp tree with its own clean git environment (`cleanEnv`).
pub(crate) struct Layout {
  _dir: TempDir,
  root: PathBuf,
  env: Vec<(String, String)>,
}

fn text(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

impl Layout {
  /// A layout whose environment also holds `extra` (`$ROOT` stands for the tree).
  pub(crate) fn new(extra: Option<&Value>) -> Res<Layout> {
    let dir = tempfile::Builder::new()
      .prefix("shell-fixture-")
      .tempdir()
      .map_err(|err| format!("temp dir: {err}"))?;
    let root = fs::canonicalize(dir.path()).map_err(|err| format!("temp dir: {err}"))?;
    let path = std::env::var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/bin"));
    let mut env = vec![
      ("PATH".to_owned(), path.to_string_lossy().into_owned()),
      ("HOME".to_owned(), text(&root.join("home"))),
      ("LC_ALL".to_owned(), "C".to_owned()),
      ("GIT_CONFIG_NOSYSTEM".to_owned(), "1".to_owned()),
      ("GIT_CEILING_DIRECTORIES".to_owned(), text(&root)),
    ];
    for (key, value) in extra.and_then(Value::as_object).into_iter().flatten() {
      let value = value.as_str().unwrap_or_default();
      env.push((key.clone(), value.replace("$ROOT", &text(&root))));
    }
    Ok(Layout {
      _dir: dir,
      root,
      env,
    })
  }

  /// `git args` in `cwd` with only this layout's environment.
  fn command(&self, cwd: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
      .args(args)
      .current_dir(cwd)
      .env_clear()
      .envs(self.env.iter().map(|(key, value)| (key, value)));
    command
  }

  /// The trimmed stdout of `git args` in `cwd`, or `None` when it cannot start,
  /// fails or prints nothing.
  fn git_out(&self, cwd: &Path, args: &[&str]) -> Option<String> {
    let output = self.command(cwd, args).output().ok()?;
    let out = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (output.status.success() && !out.is_empty()).then_some(out)
  }

  /// `git args` in `dir`, which must succeed.
  fn git(&self, dir: &Path, args: &[&str]) -> Res<()> {
    let mut full = vec!["-c", "user.email=t@t", "-c", "user.name=t"];
    full.extend_from_slice(args);
    let output = self
      .command(dir, &full)
      .output()
      .map_err(|err| format!("git {args:?}: {err}"))?;
    if output.status.success() {
      return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!("git {args:?} in {}: {stderr}", dir.display()))
  }

  /// A repository at `rel` on `branch` with one empty commit (`initRepo`).
  fn init_repo(&self, rel: &str, branch: &str) -> Res<()> {
    let dir = self.dir(rel)?;
    self.git(&dir, &["init", "-q", "-b", branch])?;
    self.git(&dir, &["commit", "-q", "--allow-empty", "-m", "init"])
  }

  /// The directory `rel` under the tree, created.
  fn dir(&self, rel: &str) -> Res<PathBuf> {
    let dir = self.root.join(rel);
    fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    Ok(dir)
  }

  /// `command` with `$MKTEMP` and `$TMP` standing for `mk` and `tmp` of the tree.
  fn expand(&self, command: &str) -> String {
    command
      .replace("$MKTEMP", &text(&self.root.join("mk")))
      .replace("$TMP", &text(&self.root.join("tmp")))
  }

  /// `path` relative to the tree after resolving links, `"."` for the tree (`relativeTo`).
  fn relative(&self, path: &Path) -> Res<String> {
    let resolved = fs::canonicalize(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let within = resolved.strip_prefix(&self.root).map_err(|err| {
      format!(
        "{} is outside {}: {err}",
        resolved.display(),
        self.root.display()
      )
    })?;
    Ok(if within.as_os_str().is_empty() {
      ".".to_owned()
    } else {
      text(within)
    })
  }

  /// The project directory the environment names, if any.
  fn project_dir(&self) -> Option<PathBuf> {
    let var = self.env.iter().find(|(key, _)| key == "TOOLU_PROJECT_DIR");
    var
      .filter(|(_, value)| !value.is_empty())
      .map(|(_, value)| PathBuf::from(value))
  }

  /// `pushTargetRoot`: the toplevel of the repository the first push targets,
  /// replaying its whole `-C` chain, then the cwd's toplevel, the project
  /// directory, and the cwd.
  fn push_target_root(&self, analysis: &ShellAnalysis, cwd: &Path) -> PathBuf {
    let pushes = push_targets(analysis);
    let chain = pushes.first().map(|push| &push.invocation.c_chain);
    let dirs: Vec<&str> = chain.into_iter().flatten().flatten().copied().collect();
    let whole = chain.is_some_and(|chain| !chain.is_empty() && chain.len() == dirs.len());
    let via_chain = whole.then(|| {
      let mut args: Vec<&str> = dirs.iter().flat_map(|dir| ["-C", *dir]).collect();
      args.extend(["rev-parse", "--show-toplevel"]);
      self.git_out(cwd, &args)
    });
    via_chain
      .flatten()
      .or_else(|| self.git_out(cwd, &["rev-parse", "--show-toplevel"]))
      .map(PathBuf::from)
      .or_else(|| self.project_dir())
      .unwrap_or_else(|| cwd.to_path_buf())
  }

  /// `pushTargetBranch`: the checked-out branch of `root`; on a detached HEAD,
  /// the branch the first push's refspec names, else `""`.
  fn push_target_branch(&self, analysis: &ShellAnalysis, root: &Path) -> String {
    match self.git_out(root, &["rev-parse", "--abbrev-ref", "HEAD"]) {
      Some(branch) if branch != "HEAD" => branch,
      _ => push_targets(analysis)
        .first()
        .and_then(|push| push.destination.clone())
        .unwrap_or_default(),
    }
  }
}

/// A `push_target_root` case: the repositories it names, its cwd and its environment.
pub(crate) fn push_root_answer(case: &Value) -> Res<Value> {
  let layout = Layout::new(case.get("env"))?;
  for repo in strings(case, "repos")? {
    layout.init_repo(&repo, "main")?;
  }
  for dir in ["outside", "codex-project"] {
    layout.dir(dir)?;
  }
  let analysis = toolu_shell::analyze(&layout.expand(field(case, "command")?));
  let cwd = layout.root.join(field(case, "cwd")?);
  let root = layout.push_target_root(&analysis, &cwd);
  Ok(json!(layout.relative(&root)?))
}

/// A `push_target_branch` case, with its repository attached on `feat/x` and then detached.
pub(crate) fn push_branch_answer(case: &Value) -> Res<Value> {
  let layout = Layout::new(None)?;
  layout.init_repo("tmp", "feat/x")?;
  let repo = layout.root.join("tmp");
  let analysis = toolu_shell::analyze(&layout.expand(field(case, "command")?));
  let attached = layout.push_target_branch(&analysis, &repo);
  layout.git(&repo, &["checkout", "-q", "--detach"])?;
  let detached = layout.push_target_branch(&analysis, &repo);
  Ok(json!({ "attached": attached, "detached": detached }))
}
