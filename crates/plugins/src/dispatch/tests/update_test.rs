use std::time::{Duration, Instant};

use serde_json::json;
use toolu_runtime::term::Style;

use crate::args::{Approval, Request, Scope, Verb};
use crate::dispatch::{Context, Report, dispatch};
use crate::fail::{Fail, Res};
use crate::prompt::{Ask, Choice};
use crate::support::{Sandbox, catalog, names};

/// A person who must never be asked: update without `--host` decides alone.
struct Nobody;

impl Ask for Nobody {
  fn many(&mut self, message: &str, _options: &[Choice]) -> Res<Vec<usize>> {
    Err(Fail::failed(format!("update asked: {message}")))
  }

  fn one(&mut self, message: &str, _options: &[Choice]) -> Res<usize> {
    Err(Fail::failed(format!("update asked: {message}")))
  }
}

fn update(
  sb: &Sandbox,
  plugins: &[&str],
  scope: Option<Scope>,
  style: Style,
) -> Result<Report, Fail> {
  let request = Request {
    verb: Verb::Update,
    names: names(plugins),
    scope,
    config: None,
    yes: false,
    dry_run: false,
    no_input: false,
    catalog: None,
    approval: Approval::Report,
  };
  let mut nobody = Nobody;
  let mut cx = Context {
    env: sb.env(),
    host: None,
    ask: Some(&mut nobody),
    timeout: Duration::from_secs(10),
    version: "7.11.0",
    origin: String::new(),
    style,
    started: Instant::now(),
    intro_shown: false,
    progress: None,
  };
  dispatch(&request, &catalog(), &mut cx)
}

/// claude and codex with toolu plugins, muse with none.
fn three_hosts(sb: &Sandbox, codex_fails: &str) {
  sb.host("claude", "[]", "never");
  sb.write(
    "claude-installed.json",
    r#"[{"id":"jev@toolu","version":"7.11.0"},{"id":"cloudflare@cloudflare","version":"1.0.1"}]"#,
  );
  sb.host("codex", r#"{"installed":[]}"#, codex_fails);
  sb.write(
    "codex-installed.json",
    r#"{"installed":[{"name":"ast-grep","version":"7.11.0","marketplaceName":"toolu"}]}"#,
  );
  sb.muse("never");
}

#[test]
fn every_host_with_toolu_plugins_is_updated_without_asking() {
  let sb = Sandbox::new();
  three_hosts(&sb, "never");
  let report = update(&sb, &[], None, Style::Plain).expect("update");
  assert!(report.text.contains("claude:\n  + jev "), "{}", report.text);
  assert!(report.text.contains("codex:\n"), "{}", report.text);
  assert!(
    report
      .text
      .contains("muse: no toolu plugins installed; skipped"),
    "{}",
    report.text
  );
  let hosts: Vec<&str> = report.json["hosts"]
    .as_array()
    .expect("hosts")
    .iter()
    .filter_map(|host| host["host"].as_str())
    .collect();
  assert_eq!(hosts, ["claude", "codex"]);
  assert_eq!(report.json["dryRun"], json!(false));
  assert!(
    sb.log("claude")
      .contains(&"plugin update jev@toolu --json".to_owned())
  );
  assert!(
    !sb
      .log("claude")
      .iter()
      .any(|line| line.contains("cloudflare"))
  );
  assert_eq!(sb.log("muse"), ["plugins list --json"]);
}

#[test]
fn names_go_only_where_they_are_installed_and_nowhere_is_an_error() {
  let sb = Sandbox::new();
  three_hosts(&sb, "never");
  let report = update(&sb, &["jev"], None, Style::Plain).expect("update");
  assert!(
    report.text.contains("codex: jev not installed; skipped"),
    "{}",
    report.text
  );
  assert!(!sb.log("codex").iter().any(|line| line.contains("jev")));
  let Err(fail) = update(&sb, &["statusline"], None, Style::Plain) else {
    panic!("a plugin installed nowhere was updated");
  };
  assert_eq!(
    fail.message,
    "statusline is not installed from toolu on any detected host"
  );
}

#[test]
fn a_failing_host_is_its_own_section_and_the_others_still_run() {
  let sb = Sandbox::new();
  three_hosts(&sb, "\"plugin marketplace add\"*");
  let report = update(&sb, &[], Some(Scope::User), Style::Plain).expect("update");
  assert!(report.failed);
  let codex = &report.json["hosts"][1];
  assert_eq!(codex["host"], json!("codex"));
  assert_eq!(codex["steps"][0]["name"], json!("codex"));
  assert_eq!(codex["steps"][0]["outcome"], json!("failed"));
  assert!(
    sb.log("claude")
      .contains(&"plugin update jev@toolu --json".to_owned())
  );
}

#[test]
fn nothing_to_update_anywhere_says_so() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  sb.muse("never");
  let Err(fail) = update(&sb, &[], None, Style::Plain) else {
    panic!("an empty update reported success");
  };
  assert_eq!(
    fail.message,
    "no detected host has toolu plugins installed, so there is nothing to update"
  );
}

#[test]
fn the_rails_report_notes_the_hosts_left_out() {
  let sb = Sandbox::new();
  three_hosts(&sb, "never");
  let report = update(&sb, &[], None, Style::Mono).expect("update");
  assert!(
    report
      .text
      .contains("│\n◇  muse\n│  ○ no toolu plugins installed\n└  "),
    "{}",
    report.text
  );
}
