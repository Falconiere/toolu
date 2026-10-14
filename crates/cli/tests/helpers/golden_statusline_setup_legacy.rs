//! The setup scenarios about odd documents, the default dir, old commands and the wired command.

use serde_json::{Value, json};

use crate::golden_statusline_actions::put;
use crate::golden_statusline_proc::{Ran, Spec, exec, set, strs};
use crate::golden_statusline_sandbox::Sandbox;
use crate::golden_statusline_setup::{Ctx, command_of, simple_scenario};
use crate::golden_statusline_value::{TOOLU, TestResult};
use crate::golden_statusline_value::{arr_of, bad, field, str_of};

fn odd_document(ctx: &Ctx<'_>) -> TestResult<()> {
  let initial = str_of(ctx.case, "initial")?;
  ctx.write(initial)?;
  let ran = ctx.setup(&[], false)?;
  ctx.outcome_ok(&ran)?;
  if str_of(ctx.case, "scenario")? == "empty" {
    return Ok(());
  }
  ctx.unchanged(initial)
}

fn default_dir(ctx: &Ctx<'_>) -> TestResult<()> {
  let ran = ctx.setup(&[], true)?;
  ctx.exit_ok(&ran)?;
  ctx.command_ok(&ctx.sb.home.join(".claude/settings.json"))
}

fn legacy_default(ctx: &Ctx<'_>) -> TestResult<()> {
  let initial = str_of(ctx.case, "initial")?;
  let settings = ctx.sb.home.join(".claude/settings.json");
  put(&settings, initial)?;
  let ran = ctx.setup(&[], true)?;
  ctx.exit_ok(&ran)?;
  ctx.stdout_ok(&ran)?;
  let doc: Value = serde_json::from_str(&std::fs::read_to_string(&settings)?)?;
  if &doc != field(ctx.case, "expectedDoc")? {
    return Err(bad(format!("settings {doc}")));
  }
  if std::fs::read_to_string(settings.with_extension("json.bak"))? != initial {
    return Err(bad("the backup is not the original document"));
  }
  let again = ctx.setup(&[], true)?;
  if !again.stdout.starts_with(str_of(ctx.case, "alreadyPrefix")?) {
    return Err(bad(format!("second run said {:?}", again.stdout)));
  }
  Ok(())
}

fn legacy_explicit(ctx: &Ctx<'_>) -> TestResult<()> {
  let doc = ctx.sb.materialize(field(ctx.case, "initialDoc")?)?;
  ctx.write(&doc.to_string())?;
  let ran = ctx.setup(&[], false)?;
  ctx.exit_ok(&ran)?;
  ctx.prefix_ok(&ran)?;
  ctx.command_ok(&ctx.settings)
}

fn shell_prefixes(ctx: &Ctx<'_>) -> TestResult<()> {
  let published = ctx.cfg.join("statusline/statusline.sh");
  for shell in arr_of(ctx.case, "shells")? {
    let shell = shell.as_str().ok_or_else(|| bad("shell is not a string"))?;
    let command = format!("{shell} \"{}\"", published.display());
    ctx.write(&json!({"statusLine": {"command": command}}).to_string())?;
    let ran = ctx.setup(&[], false)?;
    ctx.exit_ok(&ran)?;
    ctx.prefix_ok(&ran)?;
    ctx
      .command_ok(&ctx.settings)
      .map_err(|error| bad(format!("shell {shell:?}: {error}")))?;
  }
  Ok(())
}

fn shell_pipelines(ctx: &Ctx<'_>) -> TestResult<()> {
  for custom in arr_of(ctx.case, "commands")? {
    let body = json!({"statusLine": {"command": custom}}).to_string();
    ctx.write(&body)?;
    let ran = ctx.setup(&[], false)?;
    ctx
      .exit_ok(&ran)
      .map_err(|error| bad(format!("{custom}: {error}")))?;
    ctx.prefix_ok(&ran)?;
    ctx
      .unchanged(&body)
      .map_err(|error| bad(format!("{custom}: {error}")))?;
  }
  Ok(())
}

/// A `toMatchObject` over the keys the case lists: `exitCode`, `stdout` and `stderr`.
fn matches_object(ran: &Ran, want: &Value, what: &str) -> TestResult<()> {
  if let Some(code) = want.get("exitCode").and_then(Value::as_i64)
    && ran.code.map(i64::from) != Some(code)
  {
    return Err(bad(format!("{what}: exit {:?}, want {code}", ran.code)));
  }
  for (key, got) in [("stdout", &ran.stdout), ("stderr", &ran.stderr)] {
    if let Some(Value::String(wanted)) = want.get(key)
      && wanted != got
    {
      return Err(bad(format!("{what}: {key} {got:?}, want {wanted:?}")));
    }
  }
  Ok(())
}

fn wired_command(ctx: &Ctx<'_>) -> TestResult<()> {
  ctx.setup(&[], false)?;
  let mut hook_env = ctx.env(false)?;
  set(
    &mut hook_env,
    "TOOLU_HOST_OVERRIDE",
    Some("claude".to_owned()),
  );
  let hook = exec(&Spec {
    program: TOOLU,
    args: strs(&[
      "statusline",
      "hook",
      "session-start",
      "--event",
      "SessionStart",
    ]),
    cwd: &ctx.sb.root,
    env: hook_env,
    stdin: "{}",
  })?;
  matches_object(&hook, field(ctx.case, "hookExpected")?, "session-start")?;
  let wired = command_of(&ctx.settings)?.ok_or_else(|| bad("setup wired no command"))?;
  let rendered = exec(&Spec {
    program: "sh",
    args: strs(&["-c", &wired]),
    cwd: &ctx.sb.root,
    env: ctx.env(false)?,
    stdin: "{}",
  })?;
  matches_object(
    &rendered,
    field(ctx.case, "rendererExpected")?,
    "wired command",
  )
}

fn rest(scenario: &str, ctx: &Ctx<'_>) -> TestResult<()> {
  match scenario {
    "malformed" | "empty" | "not-object" => odd_document(ctx),
    "default" => default_dir(ctx),
    "legacy-default" => legacy_default(ctx),
    "legacy-explicit" => legacy_explicit(ctx),
    "shell-prefixes" => shell_prefixes(ctx),
    "shell-pipelines" => shell_pipelines(ctx),
    "wired-command" => wired_command(ctx),
    other => Err(bad(format!("unknown setup scenario: {other}"))),
  }
}

/// Replay one setup case in its own sandbox.
pub(crate) fn replay_case(case: &Value) -> TestResult<()> {
  let sb = Sandbox::new()?;
  let cfg = sb.path("cfg")?;
  std::fs::create_dir_all(&cfg)?;
  let ctx = Ctx {
    settings: cfg.join("settings.json"),
    cfg,
    sb: &sb,
    case,
  };
  let scenario = str_of(case, "scenario")?;
  simple_scenario(scenario, &ctx).unwrap_or_else(|| rest(scenario, &ctx))
}
