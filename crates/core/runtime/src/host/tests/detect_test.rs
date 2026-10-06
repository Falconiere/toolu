use toolu_protocol::host::Host;

use super::{Detected, detect};
use crate::env::Env;

fn host(pairs: &[(&str, &str)], event: Option<&str>) -> Host {
  detect(&Env::from_pairs(pairs.iter().copied()), event).host
}

#[test]
fn claude_is_the_default_without_a_host_signal() {
  let detected = detect(&Env::from_pairs([("HOME", "/home/u")]), None);
  assert_eq!(
    detected,
    Detected {
      host: Host::Claude,
      warning: None
    }
  );
}

#[test]
fn codex_plugin_root_wins_over_the_claude_compatibility_variables() {
  assert_eq!(
    host(
      &[("PLUGIN_ROOT", "/codex"), ("CLAUDE_PLUGIN_ROOT", "/c")],
      None
    ),
    Host::Codex
  );
}

#[test]
fn cursor_variables_win_over_plugin_root() {
  assert_eq!(
    host(&[("CURSOR_VERSION", "1.7.0"), ("PLUGIN_ROOT", "/p")], None),
    Host::Cursor
  );
  let both = [
    ("CURSOR_PROJECT_DIR", "/repo"),
    ("CLAUDE_PROJECT_DIR", "/repo"),
  ];
  assert_eq!(host(&both, None), Host::Cursor);
}

#[test]
fn hermes_home_alone_never_selects_hermes() {
  assert_eq!(
    host(&[("HERMES_HOME", "/home/u/.hermes")], None),
    Host::Claude
  );
}

#[test]
fn a_host_unique_event_name_selects_its_host() {
  assert_eq!(host(&[], Some("pre_tool_call")), Host::Hermes);
  assert_eq!(
    host(&[("PLUGIN_ROOT", "/p")], Some("beforeShellExecution")),
    Host::Cursor
  );
}

#[test]
fn a_shared_or_unknown_event_name_falls_through_to_the_environment() {
  assert_eq!(
    host(&[("PLUGIN_ROOT", "/p")], Some("PreToolUse")),
    Host::Codex
  );
  assert_eq!(host(&[], Some("PreToolUse")), Host::Claude);
  assert_eq!(host(&[], Some("made_up_event")), Host::Claude);
  assert_eq!(host(&[("PLUGIN_ROOT", "/p")], Some("")), Host::Codex);
}

#[test]
fn the_override_accepts_every_host_and_wins_over_all_signals() {
  for expected in Host::ALL {
    let env = [
      ("TOOLU_HOST_OVERRIDE", expected.name()),
      ("PLUGIN_ROOT", "/p"),
      ("CURSOR_VERSION", "1"),
    ];
    let detected = detect(&Env::from_pairs(env), Some("pre_tool_call"));
    assert_eq!(
      detected,
      Detected {
        host: expected,
        warning: None
      }
    );
  }
}

#[test]
fn an_invalid_override_warns_and_falls_back_to_detection() {
  let env = Env::from_pairs([("TOOLU_HOST_OVERRIDE", "gemini"), ("PLUGIN_ROOT", "/p")]);
  let warning = "toolu-host: invalid TOOLU_HOST_OVERRIDE 'gemini' (using environment detection)";
  let expected = Detected {
    host: Host::Codex,
    warning: Some(warning.to_owned()),
  };
  assert_eq!(detect(&env, None), expected);
}

#[test]
fn empty_variables_count_as_unset() {
  let env = [
    ("TOOLU_HOST_OVERRIDE", ""),
    ("PLUGIN_ROOT", ""),
    ("CURSOR_VERSION", ""),
  ];
  assert_eq!(
    detect(&Env::from_pairs(env), None),
    Detected {
      host: Host::Claude,
      warning: None
    }
  );
}
