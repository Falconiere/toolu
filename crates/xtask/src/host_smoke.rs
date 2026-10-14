//! `cargo xtask host-smoke [--bin PATH] [--host claude|codex|both]`: a live macOS
//! smoke of the native toolu hooks in real Claude Code and Codex sessions, with no
//! `bun` on the children's `PATH`. Per host it checks that a protected `.env` write
//! is denied, an allowed `pwd` runs, a post-edit quality failure is recorded, a
//! `git commit` is denied by the quality gate and a `git push` by push review.
//!
//! The sessions spend model credits, so this is never a gate step. The evidence
//! directory is kept (and printed) on failure; on success it is deleted unless
//! `TOOLU_HOST_SMOKE_KEEP=1`.

use std::ffi::OsString;
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::host_smoke_claude::Claude;
use crate::host_smoke_codex::{Codex, private_home};
use crate::host_smoke_env::ChildEnv;
use crate::host_smoke_project::{create, create_push};
use crate::host_smoke_steps::{self as steps, Driver, Run};
use crate::options::Options;
use crate::{Verdict, output};

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

/// A host the smoke can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Host {
  Claude,
  Codex,
}

/// The hosts `--host` names; both when it is absent.
pub(crate) fn hosts(arg: Option<&str>) -> Result<Vec<Host>, String> {
  match arg {
    None | Some("both") => Ok(vec![Host::Claude, Host::Codex]),
    Some("claude") => Ok(vec![Host::Claude]),
    Some("codex") => Ok(vec![Host::Codex]),
    Some(other) => Err(format!("--host must be claude, codex or both, not {other}")),
  }
}

/// `bin` as an absolute path, if it is an executable file.
pub(crate) fn require_bin(bin: &Path) -> Result<PathBuf, String> {
  let real = std::fs::canonicalize(bin).map_err(|err| {
    format!(
      "native binary missing: {}: {err} (cargo build -p toolu-cli)",
      bin.display()
    )
  })?;
  let executable = std::fs::metadata(&real)
    .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0);
  if executable {
    Ok(real)
  } else {
    Err(format!("{} is not an executable file", real.display()))
  }
}

/// The user's Codex `auth.json` under `home`.
pub(crate) fn require_auth(home: &Path) -> Result<PathBuf, String> {
  let auth = home.join(".codex/auth.json");
  if auth.is_file() {
    Ok(auth)
  } else {
    Err(format!(
      "no Codex auth at {}; run `codex login`",
      auth.display()
    ))
  }
}

struct Plan {
  hosts: Vec<Host>,
  repo: PathBuf,
  env: ChildEnv,
  claude: Option<PathBuf>,
  codex: Option<(PathBuf, PathBuf)>,
}

fn plan(options: &Options) -> Result<Plan, String> {
  let hosts = hosts(options.host.as_deref())?;
  if !cfg!(target_os = "macos") {
    return Err("live host smoke requires macOS".to_owned());
  }
  let default = options.root.join("target/debug/toolu");
  let bin = require_bin(options.bin.as_deref().unwrap_or(&default))?;
  let path = std::env::var_os("PATH").ok_or("PATH is not set")?;
  let env = ChildEnv::new(&bin, &path)?;
  let home = std::env::var_os("HOME")
    .map(PathBuf::from)
    .ok_or("HOME is not set")?;
  let claude = hosts
    .contains(&Host::Claude)
    .then(|| env.tool("claude"))
    .transpose()?;
  let codex = if hosts.contains(&Host::Codex) {
    Some((env.tool("codex")?, require_auth(&home)?))
  } else {
    None
  };
  Ok(Plan {
    hosts,
    repo: options.root.clone(),
    env,
    claude,
    codex,
  })
}

fn finding(name: &str, step: &str, result: Result<(), String>) -> Option<String> {
  result.err().map(|why| format!("{name}: {step}: {why}"))
}

/// The deny, control, edit and commit steps in one project.
fn project_steps(driver: &dyn Driver, run: &Run<'_>) -> Vec<String> {
  let name = driver.name();
  let project = match create(
    run.root,
    &format!("{name} project with spaces"),
    name,
    run.env,
  ) {
    Ok(project) => project,
    Err(why) => return vec![format!("{name}: project: {why}")],
  };
  if let Err(why) = driver.prepare(run, &project) {
    return vec![format!("{name}: prepare: {why}")];
  }
  [
    ("native deny", steps::deny(driver, run, &project)),
    ("allowed control", steps::allowed(driver, run, &project)),
    (
      "post-edit quality state",
      steps::edit(driver, run, &project),
    ),
    ("commit gate", steps::commit(driver, run, &project)),
  ]
  .into_iter()
  .filter_map(|(step, result)| finding(name, step, result))
  .collect()
}

/// The push-review step in a project with a bare remote.
fn push_steps(driver: &dyn Driver, run: &Run<'_>) -> Vec<String> {
  let name = driver.name();
  match create_push(run.root, name, run.env) {
    Ok(pushed) => finding(name, "push review", steps::push(driver, run, &pushed))
      .into_iter()
      .collect(),
    Err(why) => vec![format!("{name}: push project: {why}")],
  }
}

/// Every step of one host, one finding per failed step. A host that cannot be set
/// up stops there: its later sessions would only spend credits on a broken profile.
fn sweep(driver: &dyn Driver, run: &Run<'_>) -> Vec<String> {
  let mut found = project_steps(driver, run);
  found.extend(push_steps(driver, run));
  found
}

fn drive(plan: &Plan, root: &Path) -> Vec<String> {
  let mut found = Vec::new();
  for host in &plan.hosts {
    let host_found = match (host, &plan.claude, &plan.codex) {
      (Host::Claude, Some(exe), _) => {
        let claude = Claude {
          exe: exe.clone(),
          repo: plan.repo.clone(),
        };
        sweep(
          &claude,
          &Run {
            root,
            env: &plan.env,
          },
        )
      }
      (Host::Codex, _, Some((exe, auth))) => match private_home(root, auth) {
        Ok(home) => {
          let env = plan.env.with_codex_home(&home);
          let codex = Codex {
            exe: exe.clone(),
            repo: plan.repo.clone(),
          };
          sweep(&codex, &Run { root, env: &env })
        }
        Err(why) => vec![format!("codex: {why}")],
      },
      _ => vec!["host-smoke: host not planned".to_owned()],
    };
    if host_found.is_empty() {
      let name = if *host == Host::Claude {
        "claude"
      } else {
        "codex"
      };
      output::say(&format!(
        "{name}: native deny, post-edit quality state, commit gate and push review passed"
      ));
    }
    found.extend(host_found);
  }
  found
}

fn evidence_dir() -> Result<PathBuf, String> {
  let tick = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
  let root = std::env::temp_dir().join(format!("toolu-native-host-{}-{tick}", std::process::id()));
  std::fs::DirBuilder::new()
    .mode(0o700)
    .create(&root)
    .map_err(|err| format!("cannot create {}: {err}", root.display()))?;
  Ok(root)
}

/// Drop the Codex auth link, then the evidence unless something failed or `keep`.
pub(crate) fn settle(root: &Path, found: &[String], keep: bool) -> Result<Verdict, String> {
  let auth = root.join("codex-home/auth.json");
  match std::fs::remove_file(&auth) {
    Ok(()) => {}
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
    Err(err) => return Err(format!("cannot remove {}: {err}", auth.display())),
  }
  if found.is_empty() && !keep {
    std::fs::remove_dir_all(root)
      .map_err(|err| format!("cannot remove {}: {err}", root.display()))?;
  } else {
    output::error(&format!("host-smoke evidence: {}", root.display()));
  }
  Ok(output::findings("host-smoke", found))
}

/// Run the live smoke for `--host` with `--bin`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let plan = plan(options)?;
  let root = evidence_dir()?;
  let found = drive(&plan, &root);
  let keep = std::env::var_os("TOOLU_HOST_SMOKE_KEEP") == Some(OsString::from("1"));
  settle(&root, &found, keep)
}

#[cfg(test)]
#[path = "tests/host_smoke_test.rs"]
mod tests;
