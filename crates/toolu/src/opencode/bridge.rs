//! One `OpenCode` hook call: the plugin's `hooks.json` entries for the call's
//! Claude event whose matcher fits, each run as the binary's own
//! `toolu [<plugin>] hook <name>` with the call as a Claude-shaped payload, and
//! their output folded into the shim's answer. A hook runs with
//! `TOOLU_HOST_OVERRIDE=opencode` and `CLAUDE_PLUGIN_ROOT` set, in the call's
//! directory, under its entry's timeout. A matcher is a regular expression
//! searched in the tool name, session source or compaction trigger, as Claude
//! Code applies it; `""` and `*` match everything. The first refusal before a
//! tool ends the call, so a later hook never runs on a denied action.

use std::path::{Path, PathBuf};
use std::time::Duration;

use regex::Regex;
use serde_json::Value;
use toolu_protocol::launcher::{
  DEFAULT_TIMEOUT, MAX_TIMEOUT, Target, hook_label, hook_name, hook_plugin,
};
use toolu_protocol::opencode::Hook;
use toolu_protocol::opencode::event::{Call, matcher_subject};
use toolu_protocol::opencode::reply::{Ran, Reply};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

/// One bridge call.
pub(crate) struct Bridge<'a> {
  /// The `OpenCode` hook the call came from.
  pub(crate) hook: Hook,
  /// The plugin directory.
  pub(crate) root: &'a Path,
  /// The `toolu` binary each hook runs.
  pub(crate) exe: PathBuf,
  /// The environment each hook starts from.
  pub(crate) env: Env,
}

/// One `hooks.json` entry to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Planned {
  /// The argv after the binary.
  pub(crate) args: Vec<String>,
  /// How a failure names the hook.
  pub(crate) label: String,
  /// The entry's timeout.
  pub(crate) timeout: Duration,
}

impl Bridge<'_> {
  /// The answer for the call in `stdin`.
  pub(crate) fn run(&self, stdin: Option<&str>) -> String {
    let mut reply = Reply::default();
    if let Err(err) = self.fold(stdin, &mut reply) {
      reply.fail(
        self.hook,
        format!("toolu opencode bridge on {}: {err}", self.hook.name()),
      );
    }
    reply.json()
  }

  fn fold(&self, stdin: Option<&str>, reply: &mut Reply) -> Result<(), String> {
    let call = Call::parse(stdin.ok_or("the hook call could not be read")?)?;
    let payload = call.payload(self.hook);
    let planned = plan(
      self.hook,
      &read_hooks(self.root)?,
      matcher_subject(self.hook, &payload),
      self.root,
    )?;
    if planned.is_empty() {
      return Ok(());
    }
    let input = Value::Object(payload).to_string();
    let cwd = hook_cwd(&call)?;
    let root = self.root.display().to_string();
    let env = self
      .env
      .clone()
      .with("TOOLU_HOST_OVERRIDE", "opencode")
      .with("CLAUDE_PLUGIN_ROOT", &root);
    for one in planned {
      let mut spec =
        Spec::new(std::iter::once(self.exe.display().to_string()).chain(one.args.iter().cloned()));
      spec.cwd = Some(cwd.clone());
      spec.env = Some(env.clone());
      spec.stdin = input.clone().into_bytes();
      spec.timeout = one.timeout;
      let out = run(&spec).map_err(|err| format!("{} could not run: {err:?}", one.label))?;
      reply.add(
        self.hook,
        &Ran {
          label: &one.label,
          exit_code: out.exit_code,
          stdout: &out.stdout,
          stderr: &out.stderr,
          timed_out: out.timed_out,
        },
      );
      if reply.denied() {
        break;
      }
    }
    Ok(())
  }
}

/// Where the hooks run: a shell call's `workdir` when it exists, else the call's
/// directory. A call naming neither is refused, so a hook never judges the
/// repository the bridge happened to start in.
fn hook_cwd(call: &Call) -> Result<PathBuf, String> {
  [call.cwd(), call.directory.clone()]
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .find(|dir| dir.is_dir())
    .ok_or_else(|| "the OpenCode call names no existing directory".to_owned())
}

/// `<root>/hooks/hooks.json`; a plugin without one has no hooks.
fn read_hooks(root: &Path) -> Result<Value, String> {
  let path = root.join("hooks/hooks.json");
  match std::fs::read_to_string(&path) {
    Ok(text) => {
      serde_json::from_str(&text).map_err(|err| format!("{} is not JSON: {err}", path.display()))
    }
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
    Err(err) => Err(format!("cannot read {}: {err}", path.display())),
  }
}

/// The entries of `hooks_json` that `hook` runs for `subject`, in file order. No
/// document (`null`) or no `hooks` has no entries.
///
/// # Errors
/// When the document has the wrong shape, so a malformed file refuses a tool call
/// rather than running none of its hooks, or an entry is not a native `toolu`
/// launcher.
pub(crate) fn plan(
  hook: Hook,
  hooks_json: &Value,
  subject: Option<&str>,
  root: &Path,
) -> Result<Vec<Planned>, String> {
  let event = hook.claude_event();
  let hooks = match hooks_json {
    Value::Null => return Ok(Vec::new()),
    Value::Object(doc) => match doc.get("hooks") {
      None => return Ok(Vec::new()),
      Some(Value::Object(hooks)) => hooks,
      Some(_) => return Err("hooks.json: `hooks` must be an object".to_owned()),
    },
    Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
      return Err("hooks.json must be a JSON object".to_owned());
    }
  };
  let mut planned = Vec::new();
  for group in array(hooks.get(event), &format!("hooks.{event}"))? {
    let group = group
      .as_object()
      .ok_or_else(|| format!("hooks.json: a hooks.{event} group must be an object"))?;
    let matcher = match group.get("matcher") {
      None => "",
      Some(Value::String(matcher)) => matcher,
      Some(_) => {
        return Err(format!(
          "hooks.json: a hooks.{event} matcher must be a string"
        ));
      }
    };
    if !matches(matcher, subject) {
      continue;
    }
    for entry in array(
      group.get("hooks"),
      &format!("a hooks.{event} group's hooks"),
    )? {
      planned.push(planned_entry(entry, event, root)?);
    }
  }
  Ok(planned)
}

/// `value` as an array; absent is empty, anything else is malformed.
fn array<'v>(value: Option<&'v Value>, what: &str) -> Result<&'v [Value], String> {
  match value {
    None => Ok(&[]),
    Some(Value::Array(items)) => Ok(items),
    Some(_) => Err(format!("hooks.json: {what} must be an array")),
  }
}

fn planned_entry(entry: &Value, event: &str, root: &Path) -> Result<Planned, String> {
  let command = entry.get("command").and_then(Value::as_str).unwrap_or("");
  let (Some(plugin), Some(name)) = (hook_plugin(command), hook_name(command)) else {
    return Err(format!(
      "a {event} entry is not a native toolu launcher: {command:?}"
    ));
  };
  let seconds = entry
    .get("timeout")
    .and_then(Value::as_u64)
    .map_or(DEFAULT_TIMEOUT, |secs| {
      u32::try_from(secs)
        .unwrap_or(MAX_TIMEOUT)
        .clamp(1, MAX_TIMEOUT)
    });
  let mut args: Vec<String> = Vec::new();
  if plugin != "toolu" {
    args.push(plugin.to_owned());
  }
  args.extend(
    [
      "hook",
      name,
      "--event",
      event,
      "--plugin-root",
      &root.display().to_string(),
    ]
    .map(str::to_owned),
  );
  let target = Target {
    plugin,
    event,
    name,
  };
  Ok(Planned {
    args,
    label: hook_label(&target),
    timeout: Duration::from_secs(u64::from(seconds)),
  })
}

/// Whether `matcher` selects `subject`: `""` and `*` always, an event without a
/// subject always, else a regular-expression search (an invalid one compares
/// as text).
pub(crate) fn matches(matcher: &str, subject: Option<&str>) -> bool {
  if matcher.is_empty() || matcher == "*" {
    return true;
  }
  let Some(subject) = subject else {
    return true;
  };
  Regex::new(matcher).map_or(matcher == subject, |regex| regex.is_match(subject))
}

#[cfg(test)]
#[path = "tests/bridge_test.rs"]
mod tests;
