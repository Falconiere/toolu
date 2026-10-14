//! The bounded sandbox actions and payload kinds the statusline cases are written in.

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

use serde_json::Value;

use crate::golden_statusline_proc::{EnvPatch, Spec, exec, git, repo, set, strs, with_remote};
use crate::golden_statusline_sandbox::Sandbox;
use crate::golden_statusline_value::{TOOLU, TestResult};
use crate::golden_statusline_value::{arr_of, bad, field, int_of, str_of};

const JEV_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/jev");

/// `jev.sh` no longer ships (#440); any file stands in for a published wrapper's contents.
const WRAPPER: &str = "#!/bin/sh\nexec toolu jev \"$@\"\n";

/// The `$path` of a tagged path field.
fn tagged<'a>(value: &'a Value, key: &str) -> TestResult<&'a str> {
  let inner = field(value, key)?;
  str_of(inner, "$path")
}

fn at(sb: &Sandbox, action: &Value, key: &str) -> TestResult<PathBuf> {
  sb.mutation_path(tagged(action, key)?, false)
}

/// The directory holding `toolu` first, then the inherited `PATH`.
pub(crate) fn path_with_bin() -> TestResult<String> {
  let bin = std::path::Path::new(TOOLU)
    .parent()
    .ok_or_else(|| bad("binary has no parent"))?;
  let inherited = std::env::var("PATH").unwrap_or_default();
  Ok(format!("{}:{inherited}", bin.display()))
}

fn publish_jev(sb: &Sandbox, action: &Value) -> TestResult<()> {
  let root = sb.mutation_path(tagged(action, "root")?, false)?;
  let host = str_of(action, "host")?;
  let key = if host == "claude" {
    "CLAUDE_CONFIG_DIR"
  } else {
    "CODEX_HOME"
  };
  let mut env: EnvPatch = Vec::new();
  set(
    &mut env,
    "HOME",
    Some(sb.home.to_string_lossy().into_owned()),
  );
  set(&mut env, "PATH", Some(path_with_bin()?));
  set(&mut env, key, Some(root.to_string_lossy().into_owned()));
  set(&mut env, "TOOLU_HOST_OVERRIDE", Some(host.to_owned()));
  if let Some(Value::Object(extra)) = action.get("env") {
    for (name, value) in extra {
      set(&mut env, name, Some(sb.string(value)?));
    }
  }
  let ran = exec(&Spec {
    program: TOOLU,
    args: strs(&[
      "jev",
      "hook",
      "session-start",
      "--event",
      "SessionStart",
      "--plugin-root",
      JEV_ROOT,
    ]),
    cwd: &sb.project,
    env,
    stdin: "",
  })?;
  if ran.code == Some(0) {
    return Ok(());
  }
  Err(bad(format!(
    "jev session-start exited {:?}: {}",
    ran.code, ran.stderr
  )))
}

fn git_action(sb: &Sandbox, action: &Value) -> TestResult<()> {
  let cwd = at(sb, action, "cwd")?;
  let args = arr_of(action, "args")?
    .iter()
    .map(|arg| sb.string(arg))
    .collect::<TestResult<Vec<_>>>()?;
  git(sb, &cwd, &args)
}

fn link_tool(sb: &Sandbox, action: &Value) -> TestResult<()> {
  let tool = str_of(action, "tool")?;
  let path = at(sb, action, "path")?;
  let inherited = std::env::var("PATH").unwrap_or_default();
  let found = inherited
    .split(':')
    .map(|dir| std::path::Path::new(dir).join(tool))
    .find(|candidate| candidate.is_file());
  match found {
    Some(found) => Ok(std::os::unix::fs::symlink(found, path)?),
    None => Ok(()),
  }
}

fn apply(sb: &Sandbox, action: &Value) -> TestResult<()> {
  match str_of(action, "op")? {
    "repo" => repo(sb, &at(sb, action, "path")?, "main"),
    "remote" => with_remote(sb, str_of(action, "branch")?),
    "write" => put(&at(sb, action, "path")?, str_of(action, "body")?),
    "mkdir" => Ok(std::fs::create_dir_all(at(sb, action, "path")?)?),
    "git" => git_action(sb, action),
    "publish-jev" => publish_jev(sb, action),
    "remove" => remove(sb, action),
    "symlink" => {
      let target = sb.string(field(action, "target")?)?;
      Ok(std::os::unix::fs::symlink(target, at(sb, action, "path")?)?)
    }
    "copy" => put(&at(sb, action, "path")?, WRAPPER),
    "chmod" => chmod(sb, action),
    "link-tool" => link_tool(sb, action),
    other => Err(bad(format!("unknown fixture op: {other}"))),
  }
}

/// Write `body`, creating its parent directories.
pub(crate) fn put(path: &std::path::Path, body: &str) -> TestResult<()> {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent)?;
  }
  Ok(std::fs::write(path, body)?)
}

fn remove(sb: &Sandbox, action: &Value) -> TestResult<()> {
  let path = sb.mutation_path(tagged(action, "path")?, true)?;
  match std::fs::symlink_metadata(&path) {
    Ok(meta) if meta.is_dir() => Ok(std::fs::remove_dir_all(path)?),
    Ok(_) => Ok(std::fs::remove_file(path)?),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
    Err(error) => Err(error.into()),
  }
}

fn chmod(sb: &Sandbox, action: &Value) -> TestResult<()> {
  let mode = u32::try_from(int_of(action, "mode")?)?;
  let path = at(sb, action, "path")?;
  Ok(std::fs::set_permissions(
    path,
    std::fs::Permissions::from_mode(mode),
  )?)
}

/// Apply fixture actions in order against one sandbox.
pub(crate) fn apply_actions(sb: &Sandbox, actions: &[Value]) -> TestResult<()> {
  for action in actions {
    let op = str_of(action, "op").unwrap_or("?").to_owned();
    apply(sb, action).map_err(|error| bad(format!("action {op}: {error}")))?;
  }
  Ok(())
}

/// The status-line payload most cases send.
pub(crate) fn payload(cwd: &str, extra: &str) -> TestResult<String> {
  let dir = serde_json::to_string(cwd)?;
  Ok(format!(
    "{{\"model\":{{\"display_name\":\"Opus\"}},\"workspace\":{{\"current_dir\":{dir}}},\"context_window\":{{\"context_window_size\":200000,\"total_input_tokens\":1000}}{extra}}}"
  ))
}

/// The payload a case run names, or `None` for the default.
pub(crate) fn fixture_payload(sb: &Sandbox, value: Option<&Value>) -> TestResult<Option<String>> {
  let Some(value) = value else {
    return Ok(None);
  };
  let body = match str_of(value, "kind")? {
    "ctx" => format!(
      "{{\"model\":{{\"display_name\":\"Opus\"}},\"context_window\":{{\"context_window_size\":200000,\"total_input_tokens\":{},\"used_percentage\":{}}}}}",
      field(value, "tokens")?,
      field(value, "pct")?
    ),
    "workspace" => payload(&sb.string(field(value, "cwd")?)?, str_of(value, "extra")?)?,
    "raw" => str_of(value, "value")?.to_owned(),
    "object" => serde_json::to_string(&sb.materialize(field(value, "value")?)?)?,
    other => return Err(bad(format!("unknown payload kind: {other}"))),
  };
  Ok(Some(body))
}
