//! The cases of `fixtures/statusline/cases.json` replayed against the built `toolu`: the
//! renderer, `setup` and `SessionStart`. The suites that did this through the TypeScript
//! harness are gone with the Bun implementation (#440). The `jev`, `report` and
//! `report-error` kinds describe the TypeScript plugin's text report and its `jev.sh`
//! wrapper probe, which #431 replaced with `toolu status` JSON and readiness by `PATH`, so
//! they are counted, named in [`EXCLUDED`], and not replayed.

#[macro_use]
#[path = "helpers/ensure.rs"]
mod ensure;
#[path = "helpers/golden_statusline_actions.rs"]
mod golden_statusline_actions;
#[path = "helpers/golden_statusline_checks.rs"]
mod golden_statusline_checks;
#[path = "helpers/golden_statusline_proc.rs"]
mod golden_statusline_proc;
#[path = "helpers/golden_statusline_render.rs"]
mod golden_statusline_render;
#[path = "helpers/golden_statusline_sandbox.rs"]
mod golden_statusline_sandbox;
#[path = "helpers/golden_statusline_setup.rs"]
mod golden_statusline_setup;
#[path = "helpers/golden_statusline_setup_legacy.rs"]
mod golden_statusline_setup_legacy;
#[path = "helpers/golden_statusline_startup.rs"]
mod golden_statusline_startup;
#[path = "helpers/golden_statusline_value.rs"]
mod golden_statusline_value;

use golden_statusline_startup::ADAPTED;
use golden_statusline_value::{TestResult, all_cases, replay, str_of};

const TOTAL: usize = 75;

/// Case kinds the native statusline no longer has, with the case count and why.
const EXCLUDED: [(&str, usize, &str); 3] = [
  (
    "jev",
    16,
    "probes the retired jev.sh wrapper; #431 reads Jev readiness from PATH",
  ),
  (
    "report",
    7,
    "the TypeScript text report; #431 prints the toolu status JSON snapshot",
  ),
  (
    "report-error",
    1,
    "a text-report host error; toolu status has no host override error",
  ),
];

#[test]
fn the_case_file_holds_all_75_cases_by_kind() -> TestResult<()> {
  let cases = all_cases()?;
  ensure_eq!(cases.len(), TOTAL);
  let mut kinds = std::collections::BTreeMap::new();
  for case in &cases {
    *kinds
      .entry(str_of(case, "kind")?.to_owned())
      .or_insert(0_usize) += 1;
  }
  let want = [
    ("jev", 16),
    ("render", 31),
    ("report", 7),
    ("report-error", 1),
    ("setup", 15),
    ("startup", 5),
  ];
  ensure_eq!(
    kinds.into_iter().collect::<Vec<_>>(),
    want.map(|(k, n)| (k.to_owned(), n))
  );
  Ok(())
}

#[test]
fn adapted_cases_are_startup_cases_of_the_file() -> TestResult<()> {
  let names = all_cases()?
    .iter()
    .filter(|case| str_of(case, "kind").is_ok_and(|kind| kind == "startup"))
    .map(|case| str_of(case, "name").map(str::to_owned))
    .collect::<TestResult<Vec<_>>>()?;
  for (name, reason) in ADAPTED {
    ensure!(
      names.iter().any(|known| known == name),
      "unknown adapted case {name}"
    );
    ensure!(!reason.is_empty());
  }
  Ok(())
}

#[test]
fn render_cases_replay() {
  let done = replay("render", golden_statusline_render::replay_case);
  assert_eq!(done.unwrap_or_else(|error| panic!("{error}")), 31);
}

#[test]
fn setup_cases_replay() {
  let done = replay("setup", golden_statusline_setup_legacy::replay_case);
  assert_eq!(done.unwrap_or_else(|error| panic!("{error}")), 15);
}

#[test]
fn startup_cases_replay_with_the_old_path_retired() {
  let done = replay("startup", golden_statusline_startup::replay_case);
  assert_eq!(done.unwrap_or_else(|error| panic!("{error}")), 5);
}

#[test]
fn excluded_kinds_are_counted_and_explained() -> TestResult<()> {
  let cases = all_cases()?;
  for (kind, count, reason) in EXCLUDED {
    let found = cases
      .iter()
      .filter(|case| str_of(case, "kind").is_ok_and(|known| known == kind))
      .count();
    ensure_eq!(found, count);
    ensure!(!reason.is_empty());
  }
  Ok(())
}
