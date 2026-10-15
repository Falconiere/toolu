//! A case's sandbox: a real git repository, config and install-record files, and stub
//! executables on a `PATH` holding nothing else but `git` and `bash`.

use std::collections::BTreeMap;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::RwLock;

use crate::golden_lifecycle_cases::{Case, Res};

/// Writers of executables hold it exclusively, spawners shared: a child forked while an
/// executable is open for writing would fail to `exec` it with `ETXTBSY` (Linux).
pub(crate) static EXECUTABLES: RwLock<()> = RwLock::new(());

/// Environment overrides; `None` unsets the key in the child.
pub(crate) type EnvPatch = BTreeMap<String, Option<String>>;

/// A case's directories and the environment its hook runs with.
pub(crate) struct Prepared {
  pub(crate) _dir: tempfile::TempDir,
  pub(crate) root: PathBuf,
  pub(crate) cwd: PathBuf,
  pub(crate) env: EnvPatch,
}

fn write_file(path: &Path, body: &str) -> Res<()> {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent)?;
  }
  let _exclusive = EXECUTABLES.write().map_err(|err| err.to_string())?;
  std::fs::write(path, body)?;
  if path.extension().is_some_and(|ext| ext == "sh") {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
  }
  Ok(())
}

fn stub(bin: &Path, name: &str, body: &str) -> Res<()> {
  let path = bin.join(name);
  write_file(&path, &format!("#!/bin/sh\n{body}\n"))?;
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
  Ok(())
}

fn git(dir: &Path, home: &Path, args: &[&str]) -> Res<()> {
  let out = Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .env("HOME", home)
    .env_remove("GIT_DIR")
    .env_remove("GIT_WORK_TREE")
    .env_remove("GIT_INDEX_FILE")
    .output()?;
  if !out.status.success() {
    let stderr = String::from_utf8_lossy(&out.stderr);
    return Err(format!("git {}: {stderr}", args.join(" ")).into());
  }
  Ok(())
}

fn init_git(dir: &Path, home: &Path, branch: &str) -> Res<()> {
  git(dir, home, &["init", "-q", "-b", branch])?;
  git(dir, home, &["config", "user.email", "t@t"])?;
  git(dir, home, &["config", "user.name", "t"])?;
  git(dir, home, &["config", "commit.gpgsign", "false"])?;
  git(dir, home, &["add", "-A"])?;
  git(dir, home, &["commit", "-q", "--allow-empty", "-m", "init"])
}

fn which(tool: &str) -> Option<PathBuf> {
  let path = std::env::var_os("PATH")?;
  std::env::split_paths(&path)
    .map(|dir| dir.join(tool))
    .find(|candidate| candidate.is_file())
}

fn bin_dir(case: &Case, root: &Path) -> Res<PathBuf> {
  let bin = root.join("bin");
  std::fs::create_dir_all(&bin)?;
  for tool in ["git", "bash"] {
    if let Some(found) = which(tool) {
      symlink(found, bin.join(tool))?;
    }
  }
  if case.ast_grep == Some(true) {
    stub(&bin, "ast-grep", "exit 0")?;
  }
  if let Some(list) = &case.codex_list {
    stub(&bin, "codex", &format!("printf '%s\\n' '{list}'"))?;
  }
  Ok(bin)
}

fn json_line(value: &serde_json::Value) -> Res<String> {
  Ok(format!("{}\n", serde_json::to_string(value)?))
}

fn synthetic_plugin(case: &Case, root: &Path) -> Res<Option<PathBuf>> {
  let Some(manifest) = &case.manifest else {
    return Ok(None);
  };
  let plug = root.join("plug");
  write_file(
    &plug.join(".claude-plugin/plugin.json"),
    &json_line(manifest)?,
  )?;
  if let Some(codex) = &case.codex_manifest {
    write_file(&plug.join(".codex-plugin/plugin.json"), &json_line(codex)?)?;
  }
  Ok(Some(plug))
}

fn write_tree(base: &Path, files: Option<&BTreeMap<String, String>>) -> Res<()> {
  for (rel, body) in files.into_iter().flatten() {
    write_file(&base.join(rel), body)?;
  }
  Ok(())
}

fn mark_seen(config: &Path, cwd: &Path, project_dir: &str) -> Res<()> {
  write_file(&config.join("toolu/.gate-preset-notice-v6"), "")?;
  write_file(&config.join("toolu/.delivery-flow-migration-v7"), "")?;
  write_file(&cwd.join(project_dir).join("tmp/.permissions-written"), "")
}

fn host_env(case: &Case, home: &Path, cwd: &Path, plugin: &str) -> Vec<(&'static str, String)> {
  if case.host.as_deref() == Some("codex") {
    vec![
      ("TOOLU_HOST_OVERRIDE", "codex".to_owned()),
      ("CODEX_HOME", home.join(".codex").display().to_string()),
      ("PLUGIN_ROOT", plugin.to_owned()),
    ]
  } else {
    vec![("CLAUDE_PROJECT_DIR", cwd.display().to_string())]
  }
}

/// Build `case`'s sandbox; `plugin` is the real plugin root.
pub(crate) fn prepare(case: &Case, plugin: &Path) -> Res<Prepared> {
  let dir = tempfile::Builder::new()
    .prefix("toolu-harness-")
    .tempdir()?;
  let root = dir.path().canonicalize()?;
  let home = root.join("home");
  let project = root.join("project");
  let cwd = match case.subdir.as_deref() {
    Some(sub) => project.join(sub),
    None => project,
  };
  std::fs::create_dir_all(&cwd)?;
  std::fs::create_dir_all(home.join(".codex"))?;
  write_tree(&cwd, case.files.as_ref())?;
  if case.git != Some(false) {
    init_git(&cwd, &home, case.branch.as_deref().unwrap_or("main"))?;
  }
  write_tree(&cwd, case.untracked.as_ref())?;
  let codex = case.host.as_deref() == Some("codex");
  let config = if codex {
    home.join(".codex")
  } else {
    home.join(".claude")
  };
  if case.first_run != Some(true) {
    mark_seen(&config, &cwd, if codex { ".codex" } else { ".claude" })?;
  }
  if let Some(user) = &case.user_config {
    write_file(&config.join("toolu.config.json"), &json_line(user)?)?;
  }
  for name in case.wrappers.iter().flatten() {
    write_file(&config.join(name).join("search.sh"), "exit 0\n")?;
  }
  if let Some(value) = &case.registry {
    write_file(
      &root.join("installed_plugins.json"),
      &serde_json::to_string(value)?,
    )?;
  }
  let plug = synthetic_plugin(case, &root)?.unwrap_or_else(|| plugin.to_path_buf());
  let env = build_env(case, &root, &plug, &cwd)?;
  Ok(Prepared {
    _dir: dir,
    root,
    cwd,
    env,
  })
}

fn build_env(case: &Case, root: &Path, plug: &Path, cwd: &Path) -> Res<EnvPatch> {
  let home = root.join("home");
  let plug = plug.display().to_string();
  let bin = bin_dir(case, root)?;
  let base = [
    ("PATH", bin.display().to_string()),
    ("HOME", home.display().to_string()),
    ("LANG", "C".to_owned()),
    ("LC_ALL", "C".to_owned()),
    ("CLAUDE_PLUGIN_ROOT", plug.clone()),
    (
      "CLAUDE_PLUGINS_REGISTRY",
      root.join("installed_plugins.json").display().to_string(),
    ),
  ];
  let mut env = EnvPatch::new();
  for (key, value) in base.into_iter().chain(host_env(case, &home, cwd, &plug)) {
    env.insert(key.to_owned(), Some(value));
  }
  env.insert("EXA_API_KEY".to_owned(), None);
  for (key, value) in case.env.iter().flatten() {
    env.insert(key.clone(), Some(value.clone()));
  }
  Ok(env)
}
