//! `cargo xtask host-smoke [--bin PATH] [--host claude|codex|muse|both]`: a live
//! smoke of the native toolu hooks in real Claude Code, Codex and Muse sessions
//! (`both`, the default, means all three), with no `bun` on the children's `PATH`.
//! Claude Code and Codex run on macOS only; Muse on any Unix (#552). Per host it
//! checks that a protected `.env` write
//! is denied, an allowed `cat` of the project's nonce file runs, a post-edit quality
//! failure is recorded, a `git commit` is denied by the quality gate and a `git push` by
//! push review.
//!
//! The sessions spend model credits, so this is never a gate step. The evidence
//! directory is kept (and printed) on failure; on success it is deleted unless
//! `TOOLU_HOST_SMOKE_KEEP=1`.

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use crate::host_smoke_claude::Claude;
use crate::host_smoke_codex::{Codex, private_home};
use crate::host_smoke_env::ChildEnv;
use crate::host_smoke_muse::{self as muse, Muse};
use crate::host_smoke_project::{create, create_push};
use crate::host_smoke_steps::{self as steps, Driver, Run};
use crate::launcher_e2e::SYSTEM_DIRS;
use crate::options::Options;
use crate::scratch;
use crate::{Verdict, output};

/// A host the smoke can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Host {
  Claude,
  Codex,
  Muse,
}

impl Host {
  fn name(self) -> &'static str {
    match self {
      Host::Claude => "claude",
      Host::Codex => "codex",
      Host::Muse => "muse",
    }
  }
}

/// The hosts `--host` names; all three for `both` or when it is absent.
pub(crate) fn hosts(arg: Option<&str>) -> Result<Vec<Host>, String> {
  match arg {
    None | Some("both") => Ok(vec![Host::Claude, Host::Codex, Host::Muse]),
    Some("claude") => Ok(vec![Host::Claude]),
    Some("codex") => Ok(vec![Host::Codex]),
    Some("muse") => Ok(vec![Host::Muse]),
    Some(other) => Err(format!(
      "--host must be claude, codex, muse or both, not {other}"
    )),
  }
}

/// `Ok` when every host in `hosts` runs on `os`: Claude Code and Codex need macOS.
pub(crate) fn require_platform(hosts: &[Host], os: &str) -> Result<(), String> {
  let needs_macos = hosts.iter().any(|host| *host != Host::Muse);
  if needs_macos && os != "macos" {
    return Err("live Claude Code and Codex smoke requires macOS".to_owned());
  }
  Ok(())
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
  bin: PathBuf,
  env: ChildEnv,
  claude: Option<PathBuf>,
  codex: Option<(PathBuf, PathBuf)>,
  muse: Option<(PathBuf, PathBuf)>,
}

/// The Muse CLI and credential, when Muse is planned and no fixed-directory
/// `toolu` would answer its hooks instead of `bin`.
fn muse_plan(env: &ChildEnv, bin: &Path) -> Result<(PathBuf, PathBuf), String> {
  let dirs: Vec<PathBuf> = SYSTEM_DIRS.iter().map(PathBuf::from).collect();
  muse::require_unshadowed(&dirs, bin)?;
  Ok((
    env.tool("muse")?,
    muse::require_auth(&|key| std::env::var_os(key))?,
  ))
}

fn plan(options: &Options, root: &Path) -> Result<Plan, String> {
  let hosts = hosts(options.host.as_deref())?;
  require_platform(&hosts, std::env::consts::OS)?;
  let default = options.root.join("target/debug/toolu");
  let bin = require_bin(options.bin.as_deref().unwrap_or(&default))?;
  let path = std::env::var_os("PATH").ok_or("PATH is not set")?;
  let env = ChildEnv::new(&bin, &path, &root.join("shims"))?;
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
  let muse = hosts
    .contains(&Host::Muse)
    .then(|| muse_plan(&env, &bin))
    .transpose()?;
  Ok(Plan {
    hosts,
    repo: options.root.clone(),
    bin,
    env,
    claude,
    codex,
    muse,
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

/// One host's sweep with its own CLI and profile.
fn drive_host(plan: &Plan, host: Host, root: &Path) -> Vec<String> {
  let run = |driver: &dyn Driver, env: &ChildEnv| sweep(driver, &Run { root, env });
  let repo = plan.repo.clone();
  match (host, &plan.claude, &plan.codex, &plan.muse) {
    (Host::Claude, Some(exe), _, _) => run(
      &Claude {
        exe: exe.clone(),
        repo,
      },
      &plan.env,
    ),
    (Host::Codex, _, Some((exe, auth)), _) => match private_home(root, auth) {
      Ok(home) => run(
        &Codex {
          exe: exe.clone(),
          repo,
        },
        &plan.env.with_codex_home(&home),
      ),
      Err(why) => vec![format!("codex: {why}")],
    },
    (Host::Muse, _, _, Some((exe, auth))) => match muse::private_home(root, auth, &plan.bin) {
      Ok(home) => run(
        &Muse {
          exe: exe.clone(),
          repo,
        },
        &plan.env.with_muse_home(&home),
      ),
      Err(why) => vec![format!("muse: {why}")],
    },
    _ => vec!["host-smoke: host not planned".to_owned()],
  }
}

fn drive(plan: &Plan, root: &Path) -> Vec<String> {
  let mut found = Vec::new();
  for host in &plan.hosts {
    let host_found = drive_host(plan, *host, root);
    if host_found.is_empty() {
      output::say(&format!(
        "{}: native deny, post-edit quality state, commit gate and push review passed",
        host.name()
      ));
    }
    found.extend(host_found);
  }
  found
}

fn evidence_dir() -> Result<PathBuf, String> {
  scratch::create("toolu-native-host")
}

/// The credential links a private profile holds, relative to the evidence directory.
const AUTH_LINKS: [&str; 2] = ["codex-home/auth.json", "muse-home/.config/muse/auth.json"];

/// Drop the Codex and Muse auth links, then the evidence unless something failed or `keep`.
pub(crate) fn settle(root: &Path, found: &[String], keep: bool) -> Result<Verdict, String> {
  for link in AUTH_LINKS {
    let auth = root.join(link);
    match std::fs::remove_file(&auth) {
      Ok(()) => {}
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
      Err(err) => return Err(format!("cannot remove {}: {err}", auth.display())),
    }
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
  let root = evidence_dir()?;
  let plan = match plan(options, &root) {
    Ok(plan) => plan,
    Err(why) => {
      let cleanup = std::fs::remove_dir_all(&root);
      return Err(match cleanup {
        Ok(()) => why,
        Err(err) => format!("{why}; cannot remove {}: {err}", root.display()),
      });
    }
  };
  let found = drive(&plan, &root);
  let keep = std::env::var_os("TOOLU_HOST_SMOKE_KEEP") == Some(OsString::from("1"));
  settle(&root, &found, keep)
}

#[cfg(test)]
#[path = "tests/host_smoke_test.rs"]
mod tests;
