//! The temporary git projects `host-smoke` drives a live session in.

use std::path::{Path, PathBuf};

use crate::host_smoke_env::ChildEnv;
use crate::host_smoke_exec::run;

/// Every gate in block mode and a three-line TypeScript limit.
const LIMITS: &str = r#"{"gates":{"protectedFiles":{"mode":"block"},"commitGate":{"mode":"block"},"qualityGate":{"mode":"block"},"pushReview":{"mode":"block"}},"lang":{"ts":{"maxFileLines":3}}}"#;
/// The branch the push-review project is on, ahead of `main`.
const PUSH_BRANCH: &str = "feat/push-smoke";

/// A project whose `origin` is a local bare repository.
pub(crate) struct PushProject {
  pub(crate) project: PathBuf,
  pub(crate) remote: PathBuf,
}

/// `git args...` in `cwd`, returning its output.
pub(crate) fn git(cwd: &Path, env: &ChildEnv, args: &[&str]) -> Result<String, String> {
  let mut words = vec!["git".to_owned()];
  words.extend(args.iter().map(|arg| (*arg).to_owned()));
  run(&words, cwd, env, None)
}

/// The commit `HEAD` names in `project`.
pub(crate) fn head(project: &Path, env: &ChildEnv) -> Result<String, String> {
  Ok(git(project, env, &["rev-parse", "HEAD"])?.trim().to_owned())
}

fn write(path: &Path, text: &str) -> Result<(), String> {
  std::fs::write(path, text).map_err(|err| format!("cannot write {}: {err}", path.display()))
}

/// A committed project called `name` for `host` (`claude` or `codex`).
pub(crate) fn create(
  root: &Path,
  name: &str,
  host: &str,
  env: &ChildEnv,
) -> Result<PathBuf, String> {
  let path = root.join(name);
  let host_dir = path.join(format!(".{host}"));
  std::fs::create_dir_all(&host_dir)
    .map_err(|err| format!("cannot create {}: {err}", host_dir.display()))?;
  write(&path.join(".env"), "KEEP=1\n")?;
  write(&path.join("sample.ts"), "export const n = 1;\n")?;
  write(&path.join("tsconfig.json"), "{}\n")?;
  write(
    &path.join("package.json"),
    "{\"name\":\"native-host-smoke\",\"private\":true}\n",
  )?;
  write(&path.join("bun.lock"), "")?;
  write(&host_dir.join("toolu.config.json"), &format!("{LIMITS}\n"))?;
  git(&path, env, &["init", "-q", "-b", "main"])?;
  for (key, value) in [
    ("user.name", "Toolu Smoke"),
    ("user.email", "smoke@toolu.test"),
    ("commit.gpgsign", "false"),
  ] {
    git(&path, env, &["config", key, value])?;
  }
  git(&path, env, &["add", "-A"])?;
  git(&path, env, &["commit", "-qm", "fixture: initial"])?;
  Ok(path)
}

/// A project on `feat/push-smoke`, one clean commit ahead of `main`, with a bare `origin`.
pub(crate) fn create_push(root: &Path, host: &str, env: &ChildEnv) -> Result<PushProject, String> {
  let project = create(root, &format!("{host} push project with spaces"), host, env)?;
  let remote = root.join(format!("{host}-remote.git"));
  let remote_text = remote.display().to_string();
  git(root, env, &["init", "-q", "--bare", &remote_text])?;
  git(&project, env, &["remote", "add", "origin", &remote_text])?;
  git(&project, env, &["switch", "-q", "-c", PUSH_BRANCH])?;
  write(&project.join("note.txt"), "a clean change to push\n")?;
  git(&project, env, &["add", "note.txt"])?;
  git(&project, env, &["commit", "-qm", "docs: a clean change"])?;
  Ok(PushProject { project, remote })
}

/// Whether the bare `remote` has the branch ref `reference`.
pub(crate) fn remote_has(remote: &Path, env: &ChildEnv, reference: &str) -> Result<bool, String> {
  let refs = git(
    remote,
    env,
    &["for-each-ref", "--format=%(refname)", reference],
  )?;
  Ok(!refs.trim().is_empty())
}

#[cfg(test)]
#[path = "tests/host_smoke_project_test.rs"]
mod tests;
