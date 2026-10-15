//! `toolu statusline render` the way Claude Code runs it, and the render cases.

use std::path::PathBuf;

use serde_json::Value;

use crate::golden_statusline_actions::{apply_actions, fixture_payload, path_with_bin, payload};
use crate::golden_statusline_checks::{plain, run_checks};
use crate::golden_statusline_proc::{EnvPatch, Spec, exec, set, strs};
use crate::golden_statusline_sandbox::Sandbox;
use crate::golden_statusline_value::{TOOLU, TestResult};
use crate::golden_statusline_value::{arr_of, bad, str_of};

/// The test key every renderer run carries.
pub(crate) const KEY: &str = "statusline-test-key";

const JEV_SUFFIX: &str = "\u{1b}[2m | \u{1b}[0m\u{1b}[1m\u{1b}[32m[JEV:READY]\u{1b}[0m";
const ALL_SEGMENTS: &str = "statusline: every segment in order, colours included";

/// How one render is run.
#[derive(Default)]
pub(crate) struct RenderOpts {
  pub(crate) payload: Option<String>,
  pub(crate) cwd: Option<PathBuf>,
  pub(crate) env: EnvPatch,
}

/// A case's `env` object as an ordered patch: `null` unsets, anything else is a string.
pub(crate) fn env_patch(sb: &Sandbox, raw: Option<&Value>) -> TestResult<EnvPatch> {
  let Some(Value::Object(map)) = raw else {
    return Ok(Vec::new());
  };
  let mut env = Vec::new();
  for (key, value) in map {
    let text = if value.is_null() {
      None
    } else {
      Some(sb.string(value)?)
    };
    set(&mut env, key, text);
  }
  Ok(env)
}

fn workspace_dir(body: &str, fallback: &str) -> String {
  serde_json::from_str::<Value>(body)
    .ok()
    .and_then(|doc| {
      doc
        .get("workspace")?
        .get("current_dir")?
        .as_str()
        .map(str::to_owned)
    })
    .unwrap_or_else(|| fallback.to_owned())
}

fn is_git_worktree(sb: &Sandbox, dir: &str) -> TestResult<bool> {
  let ran = exec(&Spec {
    program: "git",
    args: strs(&["-C", dir, "rev-parse", "--show-toplevel"]),
    cwd: &sb.root,
    env: vec![(
      "HOME".to_owned(),
      Some(sb.home.to_string_lossy().into_owned()),
    )],
    stdin: "",
  })?;
  Ok(ran.code == Some(0))
}

fn render_env(sb: &Sandbox, extra: &EnvPatch) -> TestResult<EnvPatch> {
  let mut env: EnvPatch = Vec::new();
  set(
    &mut env,
    "HOME",
    Some(sb.home.to_string_lossy().into_owned()),
  );
  set(
    &mut env,
    "CLAUDE_CONFIG_DIR",
    Some(sb.root.join("cfg").to_string_lossy().into_owned()),
  );
  set(&mut env, "TOOLU_HOST_OVERRIDE", Some("claude".to_owned()));
  set(&mut env, "TYPESAFE_API_KEY", Some(KEY.to_owned()));
  set(&mut env, "PATH", Some(path_with_bin()?));
  for (key, value) in extra {
    set(&mut env, key, value.clone());
  }
  Ok(env)
}

fn clean_run(spec: &Spec<'_>, what: &str) -> TestResult<String> {
  let ran = exec(spec)?;
  if ran.code != Some(0) || !ran.stderr.is_empty() {
    return Err(bad(format!(
      "{what} exited {:?} with stderr {:?}",
      ran.code, ran.stderr
    )));
  }
  Ok(ran.stdout)
}

/// Run the renderer; the payload defaults to one for the sandbox project.
pub(crate) fn render(sb: &Sandbox, opts: &RenderOpts) -> TestResult<String> {
  let env = render_env(sb, &opts.env)?;
  let project = sb.project.to_string_lossy().into_owned();
  let body = match &opts.payload {
    Some(body) => body.clone(),
    None => payload(&project, "")?,
  };
  let cwd = opts.cwd.clone().unwrap_or_else(|| sb.project.clone());
  let worktree = workspace_dir(&body, &project);
  if is_git_worktree(sb, &worktree)? {
    let args = strs(&["statusline", "refresh-cache", &worktree]);
    clean_run(
      &Spec {
        program: TOOLU,
        args,
        cwd: &cwd,
        env: env.clone(),
        stdin: "",
      },
      "refresh-cache",
    )?;
  }
  let args = strs(&["statusline", "render"]);
  clean_run(
    &Spec {
      program: TOOLU,
      args,
      cwd: &cwd,
      env,
      stdin: &body,
    },
    "render",
  )
}

fn replay_run(sb: &Sandbox, name: &str, run: &Value) -> TestResult<()> {
  let opts = RenderOpts {
    payload: fixture_payload(sb, run.get("payload"))?,
    cwd: None,
    env: env_patch(sb, run.get("env"))?,
  };
  let raw = render(sb, &opts)?;
  let legacy = match raw.strip_suffix(JEV_SUFFIX) {
    Some(_) if name == ALL_SEGMENTS => raw.as_str(),
    Some(rest) => rest,
    None => {
      return Err(bad(format!(
        "output does not end with the Jev suffix: {raw:?}"
      )));
    }
  };
  let out = if run.get("plain") == Some(&Value::Bool(true)) {
    plain(legacy)?
  } else {
    legacy.to_owned()
  };
  run_checks(sb, &out, arr_of(run, "checks")?)
}

/// Replay one render case in its own sandbox.
pub(crate) fn replay_case(case: &Value) -> TestResult<()> {
  let sb = Sandbox::new()?;
  apply_actions(&sb, arr_of(case, "actions")?)?;
  let name = str_of(case, "name")?;
  for run in arr_of(case, "runs")? {
    replay_run(&sb, name, run)?;
  }
  Ok(())
}
