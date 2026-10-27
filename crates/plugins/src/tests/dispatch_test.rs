use std::time::Duration;

use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;

use super::{Context, Report, dispatch};
use crate::args::{Approval, Request, Scope, Verb};
use crate::fail::Fail;
use crate::prompt::Ask;
use crate::support::{Answer, Sandbox, catalog, names};

fn request(verb: Verb, plugins: &[&str]) -> Request {
  Request {
    verb,
    names: names(plugins),
    scope: None,
    config: None,
    yes: false,
    dry_run: false,
    no_input: false,
    catalog: None,
    approval: Approval::Report,
  }
}

fn go(
  sb: &Sandbox,
  request: &Request,
  host: Option<Host>,
  ask: Option<&mut dyn Ask>,
) -> Result<Report, Fail> {
  let mut cx = Context {
    env: sb.env(),
    host,
    ask,
    timeout: Duration::from_secs(10),
    version: "7.11.0",
    origin: String::new(),
    style: toolu_runtime::term::Style::Plain,
    started: std::time::Instant::now(),
    intro_shown: false,
    progress: None,
  };
  dispatch(request, &catalog(), &mut cx)
}

#[test]
fn a_multi_host_dry_run_sections_the_report_per_chosen_host() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  sb.host("codex", r#"{"installed":[]}"#, "never");
  let mut dry = request(Verb::Install, &["ast-grep"]);
  dry.dry_run = true;
  let mut both = Answer(vec![0, 1]);
  let Ok(report) = go(&sb, &dry, None, Some(&mut both)) else {
    panic!("the dry run failed");
  };
  assert_eq!(
    report.text,
    "claude:\nPlanned commands (nothing was run):\n  claude plugin install ast-grep@toolu\n\ncodex:\nPlanned commands (nothing was run):\n  codex plugin add ast-grep@toolu\n"
  );
  assert!(!report.failed);
  let refused = go(&sb, &dry, None, None).map_err(|fail| fail.exit);
  assert_eq!(refused, Err(Exit::Usage));
}

#[test]
fn the_plugin_picker_runs_only_without_names() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  let mut pick_jev = Answer(vec![
    catalog()
      .plugins
      .iter()
      .position(|entry| entry.name == "jev")
      .unwrap_or(0),
  ]);
  let mut bare = request(Verb::Install, &[]);
  bare.dry_run = true;
  let Ok(picked) = go(&sb, &bare, Some(Host::Claude), Some(&mut pick_jev)) else {
    panic!("the picked install failed");
  };
  assert_eq!(
    picked.text,
    "Planned commands (nothing was run):\n  claude plugin install jev@toolu\n"
  );
  let mut named = request(Verb::Install, &["brainstorm"]);
  named.dry_run = true;
  let mut unused = Answer(vec![0]);
  let Ok(report) = go(&sb, &named, Some(Host::Claude), Some(&mut unused)) else {
    panic!("the named install failed");
  };
  assert!(report.text.contains("brainstorm@toolu"));
}

#[test]
fn install_reports_json_and_exit_failure_when_a_step_failed() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "\"plugin install jev@toolu\"");
  let Ok(report) = go(
    &sb,
    &request(Verb::Install, &["jev"]),
    Some(Host::Claude),
    None,
  ) else {
    panic!("the install failed outright");
  };
  assert!(report.failed);
  assert_eq!(
    report.json["hosts"][0]["steps"][0]["outcome"],
    json!("failed")
  );
  assert_eq!(report.json["dryRun"], json!(false));
}

#[test]
fn remove_needs_names_and_yes_and_checks_an_explicit_scope_first() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  let no_yes = go(
    &sb,
    &request(Verb::Remove, &["jev"]),
    Some(Host::Claude),
    None,
  );
  assert_eq!(
    no_yes.map_err(|fail| (fail.exit, fail.message)),
    Err((Exit::Usage, "remove requires --yes to confirm".to_owned()))
  );
  let mut empty = request(Verb::Remove, &[]);
  empty.yes = true;
  assert!(
    go(&sb, &empty, Some(Host::Claude), None)
      .is_err_and(|fail| fail.message == "remove requires at least one name")
  );
  let mut scoped = request(Verb::Remove, &["jev"]);
  scoped.scope = Some(Scope::User);
  let codex = go(&sb, &scoped, Some(Host::Codex), None).map_err(|fail| fail.message);
  assert!(codex.is_err_and(|message| message.contains("codex has no scope concept")));
  let mut yes = request(Verb::Remove, &["jev"]);
  yes.yes = true;
  let Ok(removed) = go(&sb, &yes, Some(Host::Claude), None) else {
    panic!("remove failed");
  };
  assert_eq!(removed.text, "  - jev              removed\n");
  assert_eq!(removed.json["steps"][0]["removed"], json!(true));
}

#[test]
fn list_and_update_act_on_the_one_host() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  sb.write(
    "claude-installed.json",
    r#"[{"id":"jev@toolu","version":"7.11.0"}]"#,
  );
  let Ok(listed) = go(&sb, &request(Verb::List, &[]), None, None) else {
    panic!("list failed");
  };
  assert!(
    listed.text.contains("  = jev              7.11.0\n"),
    "{}",
    listed.text
  );
  assert_eq!(
    listed.json.as_array().map(Vec::len),
    Some(catalog().plugins.len())
  );
  let Ok(updated) = go(&sb, &request(Verb::Update, &[]), None, None) else {
    panic!("update failed");
  };
  assert_eq!(updated.text, "  + jev              updated\n");
}

#[test]
fn config_replays_the_selection_and_its_host() {
  let sb = Sandbox::new();
  sb.host("codex", r#"{"installed":[]}"#, "never");
  sb.host("claude", "[]", "never");
  let path = sb.write(
    ".toolu/plugins.json",
    r#"{"version":1,"host":"codex","enabled":["rust-quality"]}"#,
  );
  let mut replay = request(Verb::Install, &[]);
  replay.config = Some(path);
  replay.dry_run = true;
  let Ok(report) = go(&sb, &replay, None, None) else {
    panic!("the replay failed");
  };
  assert_eq!(
    report.text,
    "Planned commands (nothing was run):\n  codex plugin add toolu@toolu\n  codex plugin add rust-quality@toolu\n"
  );
  let empty = sb.write("empty.json", r#"{"version":1,"enabled":[]}"#);
  replay.config = Some(empty);
  let Ok(nothing) = go(&sb, &replay, Some(Host::Claude), None) else {
    panic!("an empty replay failed");
  };
  assert_eq!(
    (nothing.text.as_str(), nothing.failed),
    ("Planned commands (nothing was run):\n", false)
  );
}

#[test]
fn an_unsupported_host_is_a_usage_error() {
  let sb = Sandbox::new();
  let hermes =
    go(&sb, &request(Verb::List, &[]), Some(Host::Hermes), None).map_err(|fail| fail.exit);
  assert_eq!(hermes, Err(Exit::Usage));
}

#[test]
fn host_cli_remove_and_update_check_names_and_dry_runs_list_the_commands() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  // A plugin-shaped name may be a retired plugin; this one cannot be.
  let mut unknown = request(Verb::Remove, &["No_Such"]);
  unknown.yes = true;
  let refused = go(&sb, &unknown, Some(Host::Claude), None).map_err(|fail| fail.exit);
  assert_eq!(refused, Err(Exit::Usage));
  let update = go(
    &sb,
    &request(Verb::Update, &["nope"]),
    Some(Host::Claude),
    None,
  )
  .map_err(|fail| fail.exit);
  assert_eq!(update, Err(Exit::Usage));
  assert_eq!(sb.log("claude"), Vec::<String>::new());
  let mut dry = request(Verb::Remove, &["jev"]);
  dry.yes = true;
  dry.dry_run = true;
  let Ok(report) = go(&sb, &dry, Some(Host::Claude), None) else {
    panic!("the dry run failed");
  };
  assert_eq!(
    report.text,
    "Planned commands (nothing was run):\n  claude plugin uninstall jev@toolu\n"
  );
  assert_eq!(report.json["steps"][0]["removed"], json!(false));
  let mut dry_update = request(Verb::Update, &["jev"]);
  dry_update.dry_run = true;
  let Ok(updated) = go(&sb, &dry_update, Some(Host::Claude), None) else {
    panic!("the dry update failed");
  };
  assert_eq!(
    updated.text,
    "Planned commands (nothing was run):\n  claude plugin update jev@toolu --json\n"
  );
  assert!(
    !sb
      .log("claude")
      .iter()
      .any(|line| line.contains("uninstall") || line.contains("update"))
  );
}

#[test]
fn a_dry_update_lists_only_what_would_run_and_a_dry_removal_removes_nothing() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  sb.write(
    "claude-installed.json",
    r#"[{"id":"jev@toolu","version":"7.11.0"},{"id":"toolu@toolu","version":"7.10.0"}]"#,
  );
  let mut dry = request(Verb::Update, &[]);
  dry.dry_run = true;
  let Ok(report) = go(&sb, &dry, Some(Host::Claude), None) else {
    panic!("the dry update failed");
  };
  assert_eq!(
    report.text,
    "Planned commands (nothing was run):\n  claude plugin update jev@toolu --json\n  claude plugin update toolu@toolu --json\n"
  );
  let mut remove = request(Verb::Remove, &["jev"]);
  remove.yes = true;
  remove.dry_run = true;
  let Ok(planned) = go(&sb, &remove, Some(Host::Claude), None) else {
    panic!("the dry removal failed");
  };
  assert!(
    planned.json["steps"].as_array().is_some_and(
      |steps| !steps.is_empty() && steps.iter().all(|step| step["removed"] == json!(false))
    ),
    "{}",
    planned.json
  );
}

#[test]
fn a_terminal_style_draws_the_rails_report_and_opens_with_the_intro() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  sb.write(
    "claude-installed.json",
    r#"[{"id":"jev@toolu","version":"7.11.0"}]"#,
  );
  let mut cx = Context {
    env: sb.env(),
    host: Some(Host::Claude),
    ask: None,
    timeout: Duration::from_secs(10),
    version: "7.11.0",
    origin: String::new(),
    style: toolu_runtime::term::Style::Mono,
    started: std::time::Instant::now(),
    intro_shown: false,
    progress: None,
  };
  let Ok(listed) = dispatch(&request(Verb::List, &[]), &catalog(), &mut cx) else {
    panic!("list failed");
  };
  assert!(
    listed
      .text
      .starts_with("┌  toolu plugins list\n│\n◇  claude\n"),
    "{}",
    listed.text
  );
  assert!(listed.text.contains("│  ✔ jev "), "{}", listed.text);
  cx.intro_shown = true;
  let Ok(again) = dispatch(&request(Verb::List, &[]), &catalog(), &mut cx) else {
    panic!("list failed");
  };
  assert!(again.text.starts_with("◇  claude\n"), "{}", again.text);
}
