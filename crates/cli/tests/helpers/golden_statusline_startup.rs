//! `SessionStart` over the five host cases: migrate the settings command, then retire the old
//! `statusline/statusline.sh` link and the generated shim (#440) unless settings still name it.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::golden_statusline_actions::{path_with_bin, put};
use crate::golden_statusline_proc::{EnvPatch, Ran, Spec, exec, set, strs};
use crate::golden_statusline_sandbox::Sandbox;
use crate::golden_statusline_setup::command_of;
use crate::golden_statusline_value::{TOOLU, TestResult};
use crate::golden_statusline_value::{arr_of, bad, str_of};

/// What the generated fallback shim held before #440.
const SHIM: &str = "#!/bin/sh\nexec toolu statusline render \"$@\"\n";
const RENDER: &str = "toolu statusline render";
const OLD_PATH: &str = "statusline/statusline.sh";

const PUBLISHED: &str = "asserted the published statusline.sh link exists; #440 retires it";

/// Startup cases whose published-link expectation became the retire behaviour.
pub(crate) const ADAPTED: [(&str, &str); 5] = [
  (
    "statusline session-start: a bash statusLine on Claude gets one notice",
    PUBLISHED,
  ),
  (
    "statusline session-start: an interpreter path counts as a shell",
    PUBLISHED,
  ),
  (
    "statusline session-start: a published path migrates to native render",
    PUBLISHED,
  ),
  (
    "statusline session-start: a custom statusLine on Claude is silent",
    "also asserted the link exists; the shell -c pipeline still names the old path, so SessionStart keeps the link and prints a notice",
  ),
  (
    "statusline session-start: Codex never reads Claude's settings",
    PUBLISHED,
  ),
];

/// What `SessionStart` does for a command, under #440.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Effect {
  Migrated,
  Retired,
  Kept,
}

struct Old {
  link: PathBuf,
  shim: PathBuf,
}

fn plant(cfg: &Path) -> TestResult<Old> {
  let shim = cfg.join("toolu/statusline-render.sh");
  put(&shim, SHIM)?;
  let link = cfg.join(OLD_PATH);
  std::fs::create_dir_all(cfg.join("statusline"))?;
  std::os::unix::fs::symlink(&shim, &link)?;
  Ok(Old { link, shim })
}

fn effect(case: &Value, command: &str) -> TestResult<Effect> {
  Ok(if str_of(case, "expected")? == "migrated" {
    Effect::Migrated
  } else if str_of(case, "host")? == "claude" && command.contains(OLD_PATH) {
    Effect::Kept
  } else {
    Effect::Retired
  })
}

fn session_start(env: EnvPatch, cwd: &Path) -> TestResult<Ran> {
  let ran = exec(&Spec {
    program: TOOLU,
    args: strs(&[
      "statusline",
      "hook",
      "session-start",
      "--event",
      "SessionStart",
    ]),
    cwd,
    env,
    stdin: "{}",
  })?;
  if ran.code == Some(0) && ran.stderr.is_empty() {
    return Ok(ran);
  }
  Err(bad(format!(
    "session-start exited {:?} with stderr {:?}",
    ran.code, ran.stderr
  )))
}

fn present(old: &Old) -> (bool, bool) {
  let exists = |path: &Path| std::fs::symlink_metadata(path).is_ok();
  (exists(&old.link), exists(&old.shim))
}

fn verify(effect: Effect, ran: &Ran, old: &Old, first: bool) -> TestResult<()> {
  let (link, shim) = present(old);
  let text = &ran.stdout;
  let (said, keeps) = match effect {
    Effect::Migrated => (
      if first {
        text.contains("migrated")
      } else {
        text.is_empty()
      },
      false,
    ),
    Effect::Retired => (text.is_empty(), false),
    Effect::Kept => (text.contains(RENDER), true),
  };
  if said && link == keeps && shim == keeps {
    return Ok(());
  }
  Err(bad(format!(
    "{effect:?} run {}: stdout {text:?}, link kept {link}, shim kept {shim}",
    if first { "one" } else { "two" }
  )))
}

fn settings_ok(effect: Effect, settings: &Path, initial: &str, command: &str) -> TestResult<()> {
  let now = command_of(settings)?;
  let want = if effect == Effect::Migrated {
    RENDER
  } else {
    command
  };
  if now.as_deref() != Some(want) {
    return Err(bad(format!("statusLine.command {now:?}, want {want:?}")));
  }
  let backup = settings.with_extension("json.bak");
  if effect == Effect::Migrated && std::fs::read_to_string(&backup)? != initial {
    return Err(bad("the backup is not the original settings"));
  }
  Ok(())
}

fn replay_command(case: &Value, command: &str) -> TestResult<()> {
  let sb = Sandbox::new()?;
  let host = str_of(case, "host")?;
  let claude = sb.path("claude-cfg")?;
  let cfg = if host == "claude" {
    claude.clone()
  } else {
    sb.path("codex-cfg")?
  };
  let settings = claude.join("settings.json");
  let initial = json!({"statusLine": {"type": "command", "command": command}}).to_string();
  put(&settings, &initial)?;
  let old = plant(&cfg)?;
  let mut env: EnvPatch = Vec::new();
  set(
    &mut env,
    "HOME",
    Some(sb.home.to_string_lossy().into_owned()),
  );
  set(
    &mut env,
    "CLAUDE_CONFIG_DIR",
    Some(claude.to_string_lossy().into_owned()),
  );
  set(
    &mut env,
    "CODEX_HOME",
    Some(cfg.to_string_lossy().into_owned()),
  );
  set(&mut env, "TOOLU_HOST_OVERRIDE", Some(host.to_owned()));
  set(&mut env, "PATH", Some(path_with_bin()?));
  let want = effect(case, command)?;
  verify(want, &session_start(env.clone(), &sb.root)?, &old, true)?;
  settings_ok(want, &settings, &initial, command)?;
  verify(want, &session_start(env, &sb.root)?, &old, false)
}

/// Replay one startup case: every command of the case, each in its own sandbox.
pub(crate) fn replay_case(case: &Value) -> TestResult<()> {
  for command in arr_of(case, "commands")? {
    let command = command
      .as_str()
      .ok_or_else(|| bad("command is not a string"))?;
    replay_command(case, command).map_err(|error| bad(format!("command {command:?}: {error}")))?;
  }
  Ok(())
}
