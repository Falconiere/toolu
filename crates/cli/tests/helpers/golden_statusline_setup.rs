//! `toolu statusline setup` against the shared settings cases: one runner per scenario.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::golden_statusline_actions::{path_with_bin, put};
use crate::golden_statusline_proc::{EnvPatch, Ran, Spec, exec, set, strs};
use crate::golden_statusline_render::KEY;
use crate::golden_statusline_sandbox::Sandbox;
use crate::golden_statusline_value::{TOOLU, TestResult};
use crate::golden_statusline_value::{arr_of, bad, field, int_of, str_of};

/// One setup case over its sandbox, with the explicit config dir `<project>/cfg`.
pub(crate) struct Ctx<'a> {
  pub(crate) sb: &'a Sandbox,
  pub(crate) case: &'a Value,
  pub(crate) cfg: PathBuf,
  pub(crate) settings: PathBuf,
}

impl Ctx<'_> {
  /// The shared environment of a setup, hook or wired-command run.
  pub(crate) fn env(&self, default_dir: bool) -> TestResult<EnvPatch> {
    let mut env: EnvPatch = Vec::new();
    set(
      &mut env,
      "HOME",
      Some(self.sb.home.to_string_lossy().into_owned()),
    );
    set(&mut env, "PATH", Some(path_with_bin()?));
    set(&mut env, "TYPESAFE_API_KEY", Some(KEY.to_owned()));
    let dir = (!default_dir).then(|| self.cfg.to_string_lossy().into_owned());
    set(&mut env, "CLAUDE_CONFIG_DIR", dir);
    Ok(env)
  }

  /// Run `toolu statusline setup` with `extra` arguments.
  pub(crate) fn setup(&self, extra: &[&str], default_dir: bool) -> TestResult<Ran> {
    let mut args = strs(&["statusline", "setup"]);
    args.extend(strs(extra));
    exec(&Spec {
      program: TOOLU,
      args,
      cwd: &self.sb.root,
      env: self.env(default_dir)?,
      stdin: "",
    })
  }

  pub(crate) fn write(&self, body: &str) -> TestResult<()> {
    put(&self.settings, body)
  }

  /// The case's tagged string field `key`.
  pub(crate) fn tagged(&self, key: &str) -> TestResult<String> {
    self.sb.string(field(self.case, key)?)
  }

  /// The run exited as `expectedExit` says.
  pub(crate) fn exit_ok(&self, ran: &Ran) -> TestResult<()> {
    let want = int_of(self.case, "expectedExit")?;
    if ran.code.map(i64::from) == Some(want) {
      return Ok(());
    }
    Err(bad(format!(
      "exit {:?}, want {want}; stdout {:?} stderr {:?}",
      ran.code, ran.stdout, ran.stderr
    )))
  }

  /// Stdout starts with `stdoutPrefix` when the case has one.
  pub(crate) fn prefix_ok(&self, ran: &Ran) -> TestResult<()> {
    if self.case.get("stdoutPrefix").is_none() {
      return Ok(());
    }
    let prefix = self.tagged("stdoutPrefix")?;
    if ran.stdout.starts_with(&prefix) {
      return Ok(());
    }
    Err(bad(format!(
      "stdout {:?} does not start with {prefix:?}",
      ran.stdout
    )))
  }

  /// Stdout equals `expectedStdout` when the case has one.
  pub(crate) fn stdout_ok(&self, ran: &Ran) -> TestResult<()> {
    if self.case.get("expectedStdout").is_none() {
      return Ok(());
    }
    let want = self.tagged("expectedStdout")?;
    if ran.stdout == want {
      return Ok(());
    }
    Err(bad(format!("stdout {:?}, want {want:?}", ran.stdout)))
  }

  /// Exit, prefix and exact stdout together.
  pub(crate) fn outcome_ok(&self, ran: &Ran) -> TestResult<()> {
    self.exit_ok(ran)?;
    self.prefix_ok(ran)?;
    self.stdout_ok(ran)
  }

  /// `statusLine.command` of `expectedCommand` in the settings file at `path`.
  pub(crate) fn command_ok(&self, path: &Path) -> TestResult<()> {
    let want = self.tagged("expectedCommand")?;
    let got = command_of(path)?;
    if got.as_deref() == Some(want.as_str()) {
      return Ok(());
    }
    Err(bad(format!("statusLine.command {got:?}, want {want:?}")))
  }

  /// The settings file still holds exactly `body`.
  pub(crate) fn unchanged(&self, body: &str) -> TestResult<()> {
    let now = std::fs::read_to_string(&self.settings)?;
    if now == body {
      return Ok(());
    }
    Err(bad(format!(
      "settings.json changed to {now:?}, want {body:?}"
    )))
  }
}

/// `statusLine.command` of a settings file, when it has one.
pub(crate) fn command_of(path: &Path) -> TestResult<Option<String>> {
  let doc: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
  Ok(
    doc
      .get("statusLine")
      .and_then(|line| line.get("command"))
      .and_then(Value::as_str)
      .map(str::to_owned),
  )
}

fn simple(ctx: &Ctx<'_>) -> TestResult<()> {
  let ran = ctx.setup(&[], false)?;
  ctx.outcome_ok(&ran)?;
  ctx.command_ok(&ctx.settings)
}

fn preserve(ctx: &Ctx<'_>) -> TestResult<()> {
  ctx.write(str_of(ctx.case, "initial")?)?;
  let ran = ctx.setup(&[], false)?;
  ctx.exit_ok(&ran)?;
  ctx.prefix_ok(&ran)?;
  let doc: Value = serde_json::from_str(&std::fs::read_to_string(&ctx.settings)?)?;
  let want = ctx.sb.materialize(field(ctx.case, "expectedDoc")?)?;
  if doc != want {
    return Err(bad(format!("settings {doc}, want {want}")));
  }
  let backup = std::fs::read_to_string(ctx.settings.with_extension("json.bak"))?;
  if backup != str_of(ctx.case, "backup")? {
    return Err(bad(format!("backup {backup:?}")));
  }
  Ok(())
}

fn idempotent(ctx: &Ctx<'_>) -> TestResult<()> {
  ctx.setup(&[], false)?;
  let before = std::fs::read_to_string(&ctx.settings)?;
  let ran = ctx.setup(&[], false)?;
  ctx.exit_ok(&ran)?;
  ctx.stdout_ok(&ran)?;
  ctx.unchanged(&before)
}

fn refuse_custom(ctx: &Ctx<'_>) -> TestResult<()> {
  let initial = str_of(ctx.case, "initial")?;
  ctx.write(initial)?;
  let ran = ctx.setup(&[], false)?;
  ctx.exit_ok(&ran)?;
  ctx.prefix_ok(&ran)?;
  ctx.unchanged(initial)?;
  let absent = ctx.case.get("backupAbsent") == Some(&Value::Bool(true));
  if absent && ctx.settings.with_extension("json.bak").exists() {
    return Err(bad("a backup was written for a refused setup"));
  }
  Ok(())
}

fn force_custom(ctx: &Ctx<'_>) -> TestResult<()> {
  for flag in arr_of(ctx.case, "flags")? {
    let flag = flag.as_str().ok_or_else(|| bad("flag is not a string"))?;
    ctx.write(str_of(ctx.case, "initial")?)?;
    let ran = ctx.setup(&[flag], false)?;
    ctx.exit_ok(&ran)?;
    ctx.prefix_ok(&ran)?;
    ctx.command_ok(&ctx.settings)?;
    let backed = command_of(&ctx.settings.with_extension("json.bak"))?;
    if backed.as_deref() != Some(str_of(ctx.case, "backupCommand")?) {
      return Err(bad(format!("flag {flag}: backup command {backed:?}")));
    }
  }
  Ok(())
}

/// Replay the scenarios that share the explicit config dir; `None` when `scenario` is not one.
pub(crate) fn simple_scenario(scenario: &str, ctx: &Ctx<'_>) -> Option<TestResult<()>> {
  Some(match scenario {
    "create" | "explicit" => simple(ctx),
    "preserve" => preserve(ctx),
    "idempotent" => idempotent(ctx),
    "refuse-custom" => refuse_custom(ctx),
    "force-custom" => force_custom(ctx),
    _ => return None,
  })
}
