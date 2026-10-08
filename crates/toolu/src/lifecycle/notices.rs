//! Once-per-machine notices (`session-notices.ts`). A failed sentinel write
//! still returns the text, so the next session shows it again.

use std::path::Path;

use serde_json::{Map, Value};
use toolu_protocol::host::Host;

const GATE_NOTICE: &str = "toolu gates no longer prompt: the `balanced` preset advises on push-review and denylist hits, and the quality gate still blocks only `git commit`/`git push`. Pin a prompt with `gates.<name>.mode: ask`, or the old hard denies with `{\"gates\":{\"preset\":\"strict\"}}`.";

const DELIVERY_PREFIX: &str = "WARN: toolu workflow skills moved to delivery-flow (brainstorm, spec, spec-review, plan, plan-review, execution, test).";

const CLAUDE_INSTALL: &str =
  "Install with `/plugin install delivery-flow@toolu`, then invoke `/delivery-flow:delivery-flow`.";

const CODEX_INSTALL: &str = "Install with `npx @toolu/plugins install delivery-flow --host codex`, then invoke `$delivery-flow:delivery-flow`.";

const OPENCODE_INSTALL: &str = "Add `delivery-flow` to `enabled` in `.opencode/toolu/plugins.json`, then load `skill({ name: \"delivery-flow-delivery-flow\" })`.";

const NEUTRAL_INSTALL: &str =
  "Install the `delivery-flow` plugin, then load its `delivery-flow` skill.";

/// The gate-preset notice, the first time, unless the user already pinned delivery.
pub(crate) fn gate_preset_notice(
  config_root: &Path,
  config: &Map<String, Value>,
) -> Option<String> {
  once(
    &config_root.join("toolu").join(".gate-preset-notice-v6"),
    GATE_NOTICE,
    !delivery_pinned(config),
  )
}

/// The delivery-flow move, the first time, for this host's install command.
pub(crate) fn delivery_flow_notice(config_root: &Path, host: Host) -> Option<String> {
  let text = format!("{DELIVERY_PREFIX} {}", delivery_install(host));
  once(
    &config_root
      .join("toolu")
      .join(".delivery-flow-migration-v7"),
    &text,
    true,
  )
}

fn delivery_install(host: Host) -> &'static str {
  match host {
    Host::Claude => CLAUDE_INSTALL,
    Host::Codex => CODEX_INSTALL,
    Host::Opencode => OPENCODE_INSTALL,
    Host::Cursor | Host::Hermes => NEUTRAL_INSTALL,
  }
}

fn delivery_pinned(config: &Map<String, Value>) -> bool {
  let Some(gates) = config.get("gates").and_then(Value::as_object) else {
    return false;
  };
  if gates.get("preset").is_some_and(|value| !value.is_null()) {
    return true;
  }
  gates.values().any(|gate| {
    gate
      .as_object()
      .is_some_and(|gate| gate.contains_key("mode"))
  })
}

fn once(sentinel: &Path, text: &str, show: bool) -> Option<String> {
  if !show || sentinel.exists() {
    return None;
  }
  if let Some(parent) = sentinel.parent() {
    let _dir = std::fs::create_dir_all(parent);
  }
  let _file = std::fs::write(sentinel, "");
  Some(text.to_owned())
}

#[cfg(test)]
#[path = "tests/notices_test.rs"]
mod tests;
