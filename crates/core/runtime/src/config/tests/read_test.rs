use serde_json::{Value, json};
use toolu_protocol::host::Host;

use super::{
  CodexModel, MODEL_CLASSES, codex_model, config_string, enabled, enabled_explicit, flag_false,
  flag_true, model,
};
use crate::config::load::LoadedConfig;

fn config(data: Value) -> LoadedConfig {
  let Value::Object(map) = data else {
    panic!("not an object")
  };
  LoadedConfig::from_data(map, Host::Claude)
}

#[test]
fn enabled_defaults_on_and_turns_off_for_false_or_the_string_false() {
  let c =
    config(json!({ "hooks": { "a": false, "b": "false", "c": true, "d": 0 }, "skills": [false] }));
  let got: Vec<bool> = ["a", "b", "c", "d", "z"]
    .iter()
    .map(|name| enabled(&c, "hooks", name))
    .collect();
  assert_eq!(got, [false, false, true, true, true]);
  assert!(enabled(&c, "skills", "0"));
}

#[test]
fn flags_and_explicit_enables_are_default_off() {
  let c = config(json!({ "p": { "t": true, "f": false, "st": "true", "sf": "false" } }));
  assert_eq!(
    [
      flag_true(&c, "p", "t"),
      flag_true(&c, "p", "st"),
      flag_true(&c, "p", "x")
    ],
    [true, false, false]
  );
  assert_eq!(
    [
      flag_false(&c, "p", "f"),
      flag_false(&c, "p", "sf"),
      flag_false(&c, "p", "x")
    ],
    [true, false, false]
  );
  assert_eq!(
    [
      enabled_explicit(&c, "p", "t"),
      enabled_explicit(&c, "p", "st"),
      enabled_explicit(&c, "p", "sf")
    ],
    [true, true, false]
  );
}

#[test]
fn config_string_is_silent_when_absent_and_warns_when_unqualified() {
  let c =
    config(json!({ "a": { "ok": "block", "bad": "maybe", "num": 3, "nil": null }, "s": "flat" }));
  let modes = ["block", "advise", "off"];
  assert_eq!(config_string(&c, "a.ok", "advise", &modes), "block");
  for path in ["a.missing", "s.deeper", "a.nil", "nowhere"] {
    assert_eq!(config_string(&c, path, "advise", &modes), "advise");
  }
  assert_eq!(c.take_warnings(), Vec::<String>::new());
  assert_eq!(config_string(&c, "a.bad", "advise", &modes), "advise");
  assert_eq!(config_string(&c, "a.num", "advise", &modes), "advise");
  assert_eq!(
    c.take_warnings(),
    [
      "a.bad: 'maybe' is not an allowed value (block advise off); using advise",
      "a.num: value is not a string; using advise",
    ]
  );
}

#[test]
fn model_passes_aliases_warns_on_junk_and_reads_false_and_empty_as_unset() {
  let c = config(
    json!({ "models": { "review": "opus", "synthesis": "gpt", "mechanical": false, "exploration": "", "architecture": 3 } }),
  );
  assert_eq!(MODEL_CLASSES.len(), 6);
  let got: Vec<String> = [
    "review",
    "mechanical",
    "exploration",
    "synthesis",
    "architecture",
    "implementation",
  ]
  .iter()
  .map(|class| model(&c, class).unwrap())
  .collect();
  assert_eq!(got, ["opus", "haiku", "sonnet", "opus", "opus", "sonnet"]);
  assert_eq!(
    c.take_warnings(),
    [
      "models.synthesis: 'gpt' is not a model alias (haiku sonnet opus fable inherit); using opus",
      "models.architecture: '3' is not a model alias (haiku sonnet opus fable inherit); using opus",
    ]
  );
  let error = model(&c, "reviewer").unwrap_err().0;
  assert!(
    error.starts_with("unknown model class 'reviewer'"),
    "{error}"
  );
}

#[test]
fn codex_model_and_effort_fall_back_independently() {
  let c = config(json!({ "models": { "codex": {
    "review": { "model": "", "reasoningEffort": "extreme" },
    "synthesis": { "model": "gpt-x", "reasoningEffort": 3 },
    "architecture": { "model": "gpt-arch", "reasoningEffort": "max" },
    "exploration": { "model": false, "reasoningEffort": false }
  } } }));
  let pair = |model: &str, effort: &str| CodexModel {
    model: model.to_owned(),
    reasoning_effort: effort.to_owned(),
  };
  assert_eq!(
    codex_model(&c, "architecture").unwrap(),
    pair("gpt-arch", "max")
  );
  assert_eq!(
    codex_model(&c, "mechanical").unwrap(),
    pair("gpt-5.6-luna", "medium")
  );
  assert_eq!(
    codex_model(&c, "exploration").unwrap(),
    pair("gpt-5.6-terra", "medium")
  );
  assert_eq!(
    codex_model(&c, "review").unwrap(),
    pair("gpt-5.6-terra", "high")
  );
  assert_eq!(codex_model(&c, "synthesis").unwrap(), pair("gpt-x", "high"));
  assert_eq!(
    c.take_warnings(),
    [
      "models.codex.review.model: value is not a non-empty string; using gpt-5.6-terra",
      "models.codex.review.reasoningEffort: 'extreme' is not supported (low medium high xhigh max ultra); using high",
      "models.codex.synthesis.reasoningEffort: value is not a string; using high",
    ]
  );
  assert!(codex_model(&c, "nope").is_err());
}
