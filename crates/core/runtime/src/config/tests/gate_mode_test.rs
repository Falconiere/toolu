use serde_json::{Value, json};
use toolu_protocol::decision::Decision;
use toolu_protocol::event::HostEvent;
use toolu_protocol::host::Host;
use toolu_protocol::text::Text;

use super::{
  GATE_GUARDRAILS, GATE_NAMES, GateMode, gate_decision, gate_mode, gate_preset, guardrail_warning,
};
use crate::config::load::LoadedConfig;

fn config(data: Value, host: Host) -> LoadedConfig {
  let Value::Object(map) = data else {
    panic!("not an object")
  };
  LoadedConfig::from_data(map, host)
}

fn modes(c: &LoadedConfig) -> Vec<&'static str> {
  GATE_NAMES
    .iter()
    .map(|name| gate_mode(c, name, None, None).name())
    .collect()
}

#[test]
fn balanced_blocks_quality_asks_on_guardrails_and_advises_the_rest() {
  let c = config(json!({}), Host::Claude);
  assert_eq!(gate_preset(&c), "balanced");
  assert_eq!(
    modes(&c),
    [
      "advise", "block", "advise", "ask", "advise", "advise", "advise", "ask", "ask"
    ]
  );
}

#[test]
fn strict_blocks_everything_and_relaxed_keeps_guardrails_asking() {
  let strict = config(json!({ "gates": { "preset": "strict" } }), Host::Claude);
  assert!(modes(&strict).iter().all(|mode| *mode == "block"));
  let relaxed = config(json!({ "gates": { "preset": "relaxed" } }), Host::Claude);
  assert_eq!(
    modes(&relaxed),
    [
      "advise", "advise", "off", "ask", "off", "off", "off", "ask", "ask"
    ]
  );
}

#[test]
fn the_gate_key_beats_the_legacy_key_which_beats_the_preset() {
  let c = config(
    json!({
      "docsSync": { "mode": "block" }, "agentTier": { "mode": "off" },
      "gates": { "docsSync": { "mode": "ask" }, "commitGate": { "mode": "off" }, "pushReview": { "mode": "maybe" } }
    }),
    Host::Claude,
  );
  assert_eq!(gate_mode(&c, "docsSync", None, None), GateMode::Ask);
  assert_eq!(gate_mode(&c, "agentTier", None, None), GateMode::Off);
  assert_eq!(gate_mode(&c, "commitGate", None, None), GateMode::Off);
  assert_eq!(gate_mode(&c, "pushReview", None, None), GateMode::Advise);
  assert_eq!(
    c.take_warnings(),
    [
      "gates.pushReview.mode: 'maybe' is not an allowed value (block ask advise off); using __unset__",
    ]
  );
}

#[test]
fn ask_degrades_by_class_where_the_host_cannot_prompt() {
  let all_ask: serde_json::Map<String, Value> = GATE_NAMES
    .iter()
    .map(|name| ((*name).to_owned(), json!({ "mode": "ask" })))
    .collect();
  let codex = config(json!({ "gates": all_ask }), Host::Codex);
  for name in GATE_NAMES {
    let expected = if GATE_GUARDRAILS.contains(&name) {
      GateMode::Block
    } else {
      GateMode::Advise
    };
    assert_eq!(gate_mode(&codex, name, None, None), expected, "{name}");
  }
  let claude = config(
    json!({ "gates": codex.data["gates"].clone() }),
    Host::Claude,
  );
  assert!(modes(&claude).iter().all(|mode| *mode == "ask"));
  assert_eq!(
    gate_mode(&claude, "pushReview", Some(Host::Codex), None),
    GateMode::Advise
  );
  let permission = gate_mode(
    &claude,
    "pushReview",
    Some(Host::Codex),
    Some(HostEvent::PermissionEvaluate),
  );
  assert_eq!(permission, GateMode::Ask);
}

#[test]
fn an_invalid_config_blocks_every_gate_and_an_unknown_gate_warns_and_blocks() {
  let mut c = config(json!({ "gates": { "preset": "relaxed" } }), Host::Claude);
  c.invalid = Some("/p/toolu.config.json: unknown top-level key 'nope'".to_owned());
  assert!(modes(&c).iter().all(|mode| *mode == "block"));
  assert_eq!(gate_mode(&c, "pushReveiw", None, None), GateMode::Block);
  let known = GATE_NAMES.join(" ");
  assert_eq!(
    c.take_warnings(),
    [format!(
      "unknown gate 'pushReveiw' (known: {known}); enforcing block"
    )]
  );
}

#[test]
fn decisions_follow_the_mode_and_off_is_silent() {
  let reason = Text::new("why").unwrap();
  assert_eq!(gate_decision(GateMode::Off, reason.clone()), None);
  assert_eq!(
    gate_decision(GateMode::Advise, reason.clone()),
    Some(Decision::Advisory {
      message: reason.clone()
    })
  );
  assert_eq!(
    gate_decision(GateMode::Ask, reason.clone()),
    Some(Decision::Ask {
      reason: reason.clone()
    })
  );
  assert_eq!(
    gate_decision(GateMode::Block, reason.clone()),
    Some(Decision::Deny { reason })
  );
  let warning = guardrail_warning("Editing .env", "Secrets live there.");
  assert!(
    warning.starts_with("####")
      && warning.contains("\nEditing .env\n")
      && warning.contains("WHY THIS IS GUARDED\nSecrets live there.\n")
  );
  assert!(warning.ends_with("the answer is no."));
}
