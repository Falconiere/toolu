//! Registry files for tests: raw files, ESM modules, manifests and install records.

use toolu_engine::Phase;
use toolu_protocol::host::Host;

use super::hook::Hook;
use super::sandbox::{Res, write};

/// A file in the event directory, as written.
pub(crate) fn file(hook: &Hook, phase: Phase, file: &str, body: &str) -> Res<()> {
  write(&hook.dir(phase).join(file), body)
}

/// An ESM module whose `run(event, ctx)` body is `body`.
pub(crate) fn js(hook: &Hook, phase: Phase, spec: &str, name: &str, body: &str) -> Res<()> {
  let event = if phase == Phase::Pre {
    "tool/pre"
  } else {
    "tool/post"
  };
  let source = format!(
    "export default {{ spec: {spec:?}, name: {name:?}, event: {event:?}, async run(event, ctx) {{ {body} }} }};\n"
  );
  file(hook, phase, &format!("{spec}__{name}.js"), &source)
}

/// A manifest for `spec__name` with `matcher`.
pub(crate) fn manifest(
  hook: &Hook,
  phase: Phase,
  spec: &str,
  name: &str,
  matcher: &str,
) -> Res<()> {
  let event = if phase == Phase::Pre {
    "tool/pre"
  } else {
    "tool/post"
  };
  let body = format!(
    r#"{{"version":1,"spec":"{spec}","name":"{name}","event":"{event}","matcher":"{matcher}"}}"#
  );
  file(hook, phase, &format!("{spec}__{name}.json"), &body)
}

/// Record `specs` as installed: Claude's install record or a ready Codex snapshot.
pub(crate) fn install(hook: &Hook, specs: &[&str]) -> Res<()> {
  let list: Vec<String> = specs.iter().map(|spec| format!("{spec:?}")).collect();
  match hook.host {
    Host::Codex => write(
      &hook.sb.path("codex/toolu/codex-plugins.json"),
      &format!(
        r#"{{"version":1,"status":"ready","plugins":[{}]}}"#,
        list.join(",")
      ),
    ),
    Host::Claude | Host::Cursor | Host::Hermes | Host::Opencode => {
      let plugins: Vec<String> = list
        .iter()
        .map(|spec| format!("{spec}:[{{\"scope\":\"user\"}}]"))
        .collect();
      write(
        &hook.sb.path("home/.claude/plugins/installed_plugins.json"),
        &format!(r#"{{"version":2,"plugins":{{{}}}}}"#, plugins.join(",")),
      )
    }
  }
}
