use std::time::Duration;

use toolu_protocol::host::Host;
use toolu_runtime::term::Style;

use super::{Frame, list, steps};
use crate::args::Verb;
use crate::dispatch::dispatch;
use crate::host::Target;
use crate::report::Row;
use crate::step::{Status, Step};
use crate::support::{Sandbox, catalog, context, request};

fn frame(style: Style, title: Option<&str>) -> Frame {
  Frame {
    style,
    elapsed: Duration::from_millis(1250),
    title: title.map(str::to_owned),
  }
}

fn step(name: &str, status: Status, detail: &str) -> Step {
  let argv = vec![
    "claude".to_owned(),
    "plugin".to_owned(),
    "install".to_owned(),
    format!("{name}@toolu"),
  ];
  Step::new(name, status, detail, argv)
}

#[test]
fn an_install_report_draws_a_section_per_host_a_glyph_per_plugin_and_the_counts() {
  let sections = vec![
    (
      Target::Claude,
      vec![
        step("toolu", Status::Installed, "installed"),
        step("jev", Status::Already, "already installed at 8.5.0"),
        step("epic-orchestrator", Status::Skew, "installed at 8.4.0"),
        step("ast-grep", Status::Failed, "boom"),
      ],
    ),
    (
      Target::Codex,
      vec![step("toolu", Status::Installed, "installed")],
    ),
  ];
  assert_eq!(
    steps(
      &frame(Style::Mono, Some("toolu plugins install")),
      &sections,
      &[],
      false
    ),
    "┌  toolu plugins install\n│\n\
     ◇  claude\n\
     │  ✔ toolu             installed\n\
     │  ● jev               already installed at 8.5.0\n\
     │  ▲ epic-orchestrator installed at 8.4.0\n\
     │  ✘ ast-grep          boom\n\
     │\n\
     ◇  codex\n\
     │  ✔ toolu             installed\n\
     └  2 installed · 1 already · 1 skew · 1 failed  (1.2s)\n"
  );
}

#[test]
fn a_dry_run_names_the_command_each_skipped_plugin_would_run() {
  let sections = vec![(
    Target::Claude,
    vec![
      step("x", Status::Skipped, "dry run"),
      step("jev", Status::Already, "already installed at 8.5.0"),
    ],
  )];
  let text = steps(&frame(Style::Mono, None), &sections, &[], true);
  assert_eq!(
    text,
    "◇  claude\n\
     │  ○ x   would run: claude plugin install x@toolu\n\
     │  ● jev already installed at 8.5.0\n\
     └  1 already · 1 skipped  (1.2s)\n"
  );
}

#[test]
fn mono_has_no_escape_and_color_paints_the_glyphs() {
  let sections = vec![(Target::Claude, vec![step("jev", Status::Failed, "boom")])];
  assert!(!steps(&frame(Style::Mono, Some("t")), &sections, &[], false).contains('\x1b'));
  let color = steps(&frame(Style::Color, Some("t")), &sections, &[], false);
  assert!(color.contains("\x1b[31m✘\x1b[0m"), "{color}");
}

#[test]
fn a_list_shows_installed_disabled_and_missing_plugins() {
  let rows = vec![
    Row {
      name: "toolu".to_owned(),
      installed: true,
      version: Some("8.5.0".to_owned()),
      enabled: true,
    },
    Row {
      name: "jev".to_owned(),
      installed: true,
      version: Some("8.5.0".to_owned()),
      enabled: false,
    },
    Row {
      name: "statusline".to_owned(),
      installed: false,
      version: None,
      enabled: false,
    },
  ];
  assert_eq!(
    list(&frame(Style::Mono, None), Target::Claude, &rows),
    "◇  claude\n\
     │  ✔ toolu      8.5.0\n\
     │  ▲ jev        8.5.0 disabled\n\
     │  ○ statusline not installed\n\
     └  2 installed · 1 not installed  (1.2s)\n"
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
  let mut cx = context(
    &sb,
    Some(Host::Claude),
    None,
    toolu_runtime::term::Style::Mono,
  );
  let Ok(listed) = dispatch(&request(Verb::List, &[]), &catalog(), &mut cx) else {
    panic!("list failed");
  };
  let names = catalog().names();
  let width = names
    .iter()
    .map(|name| name.chars().count())
    .max()
    .unwrap_or(0);
  let rows: String = names
    .iter()
    .map(|name| match name.as_str() {
      "jev" => format!("│  ✔ {name:<width$} 7.11.0\n"),
      _missing => format!("│  ○ {name:<width$} not installed\n"),
    })
    .collect();
  let missing = names.len() - 1;
  let (body, elapsed) = listed.text.rsplit_once("  (").expect("a closing line");
  assert_eq!(
    body,
    format!("┌  toolu plugins list\n│\n◇  claude\n{rows}└  1 installed · {missing} not installed")
  );
  assert!(elapsed.ends_with("s)\n"), "{elapsed}");
  cx.progress = Some(std::sync::Arc::new(crate::progress::Progress::to_writer(
    Box::new(std::io::sink()),
    toolu_runtime::term::Style::Mono,
  )));
  let Ok(again) = dispatch(&request(Verb::List, &[]), &catalog(), &mut cx) else {
    panic!("list failed");
  };
  // Progress drew the intro on the terminal, so the report starts at the host.
  let (again_body, _) = again.text.rsplit_once("  (").expect("a closing line");
  assert_eq!(
    Some(again_body),
    body.strip_prefix("┌  toolu plugins list\n│\n")
  );
}
