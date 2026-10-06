//! Gate enforcement modes (`gate-mode.ts`). A gate owns its check; this decides
//! how firmly a failed check reaches the user. Precedence: `gates.<name>.mode`,
//! the legacy top-level key (`docsSync`, `agentTier`), the `gates.preset`
//! table, then `balanced`. Where the host cannot prompt, `ask` degrades: a
//! security guardrail to `block`, a judgement gate to `advise`. An invalid
//! config envelope fails closed: every gate blocks.

use toolu_protocol::decision::Decision;
use toolu_protocol::encode::supports_ask;
use toolu_protocol::event::HostEvent;
use toolu_protocol::host::Host;
use toolu_protocol::text::Text;

use super::load::LoadedConfig;
use super::read::config_string;

/// How firmly a failed check reaches the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateMode {
  /// Deny the action.
  Block,
  /// Ask the user.
  Ask,
  /// Tell the model, stop nothing.
  Advise,
  /// Say nothing.
  Off,
}

/// The mode names, in [`GateMode`] order.
pub const GATE_MODES: [&str; 4] = ["block", "ask", "advise", "off"];

impl GateMode {
  /// The config name.
  pub fn name(self) -> &'static str {
    match self {
      GateMode::Block => "block",
      GateMode::Ask => "ask",
      GateMode::Advise => "advise",
      GateMode::Off => "off",
    }
  }

  fn parse(name: &str) -> Option<GateMode> {
    [
      GateMode::Block,
      GateMode::Ask,
      GateMode::Advise,
      GateMode::Off,
    ]
    .into_iter()
    .find(|mode| mode.name() == name)
  }
}

/// The preset names; `balanced` is what ships.
pub const GATE_PRESETS: [&str; 3] = ["strict", "balanced", "relaxed"];

/// The gates a mode can be set for.
pub const GATE_NAMES: [&str; 9] = [
  "pushReview",
  "qualityGate",
  "commitGate",
  "bashCommands",
  "planLedger",
  "docsSync",
  "agentTier",
  "protectedFiles",
  "mcpBlocker",
];

/// Security guardrails: `ask` at every preset but strict, and `ask` degrades to block.
pub const GATE_GUARDRAILS: [&str; 3] = ["protectedFiles", "mcpBlocker", "bashCommands"];

/// Marks an absent or rejected mode key; never a valid mode.
const UNSET: &str = "__unset__";

/// `toolu_gate_preset`: the configured preset, or `balanced`.
pub fn gate_preset(config: &LoadedConfig) -> String {
  config_string(config, "gates.preset", "balanced", &GATE_PRESETS)
}

fn read_mode(config: &LoadedConfig, path: &str) -> Option<GateMode> {
  GateMode::parse(&config_string(config, path, UNSET, &GATE_MODES))
}

/// The preset table: the whole policy.
fn preset_mode(preset: &str, name: &str, guardrail: bool) -> GateMode {
  match (preset, guardrail) {
    ("strict", _) => GateMode::Block,
    (_, true) => GateMode::Ask,
    ("relaxed", false) if name == "pushReview" || name == "qualityGate" => GateMode::Advise,
    ("relaxed", false) => GateMode::Off,
    (_, false) if name == "qualityGate" => GateMode::Block,
    (_, false) => GateMode::Advise,
  }
}

/// `toolu_gate_mode NAME` for `host` (the config's when `None`) at `event`
/// (`tool/pre` when `None`). An unknown gate name is a caller typo: it warns
/// and blocks, so a misspelled gate keeps enforcing.
pub fn gate_mode(
  config: &LoadedConfig,
  name: &str,
  host: Option<Host>,
  event: Option<HostEvent>,
) -> GateMode {
  if !GATE_NAMES.contains(&name) {
    let known = GATE_NAMES.join(" ");
    config.warn(format!(
      "unknown gate '{name}' (known: {known}); enforcing block"
    ));
    return GateMode::Block;
  }
  if config.invalid.is_some() {
    return GateMode::Block;
  }
  let guardrail = GATE_GUARDRAILS.contains(&name);
  let legacy = match name {
    "docsSync" => Some("docsSync.mode"),
    "agentTier" => Some("agentTier.mode"),
    _ => None,
  };
  let mode = read_mode(config, &format!("gates.{name}.mode"))
    .or_else(|| legacy.and_then(|path| read_mode(config, path)))
    .unwrap_or_else(|| preset_mode(&gate_preset(config), name, guardrail));
  let event = event.unwrap_or(HostEvent::ToolPre);
  if mode == GateMode::Ask && !supports_ask(host.unwrap_or(config.host), event) {
    return if guardrail {
      GateMode::Block
    } else {
      GateMode::Advise
    };
  }
  mode
}

/// `toolu_gate_emit MODE REASON` as a decision; `off` is none.
pub fn gate_decision(mode: GateMode, reason: Text) -> Option<Decision> {
  match mode {
    GateMode::Off => None,
    GateMode::Advise => Some(Decision::Advisory { message: reason }),
    GateMode::Ask => Some(Decision::Ask { reason }),
    GateMode::Block => Some(Decision::Deny { reason }),
  }
}

/// `toolu_gate_guardrail_warning HEADLINE DETAIL`: the prompt of a guardrail's
/// `ask`. Loud on purpose: approving overrides a protection.
pub fn guardrail_warning(headline: &str, detail: &str) -> String {
  format!(
    "############################################################
##  ⚠️  SECURITY GUARDRAIL — OVERRIDE REQUESTED  ⚠️        ##
############################################################

{headline}

WHY THIS IS GUARDED
{detail}

Approving covers THIS ONE CALL. Nothing is remembered and the next attempt
asks again. If you did not just ask for this, the answer is no."
  )
}

#[cfg(test)]
#[path = "tests/gate_mode_test.rs"]
mod tests;
