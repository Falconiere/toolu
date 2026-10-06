//! `fixtures/state/cases.json` against the Rust state layer (AC-4, AC-11):
//! the schema, telemetry and branch families here, the diff family in
//! `diff_cases.rs`, the I/O family in `io_cases.rs`, the gate-file family in
//! `gate_cases.rs`, the concurrency family in `interleave.rs`. Of the `package` case, which checks
//! TypeScript's module exports, the version and sample document hold in Rust.

#[path = "helpers/cases.rs"]
mod cases;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::collections::BTreeMap;

use cases::{cases_of, field, text};
use sandbox::{Res, Sandbox};
use toolu_runtime::json::ordered::Ordered;
use toolu_state::edit_records::parse_edit_record;
use toolu_state::gate_schema::{GATE_FILE_VERSION, validate_gate_file};
use toolu_state::git::{base_branch, branch_slug, branch_slugs, current_branch};
use toolu_state::telemetry::{TELEMETRY_EVENTS, TELEMETRY_VERSION};
use toolu_state::telemetry_schema::{TelemetryLine, parse_telemetry_extras, parse_telemetry_line};

const CASES: &str = "state/cases.json";

fn strings(value: &Ordered) -> Vec<String> {
  let Ordered::Array(items) = value else {
    return Vec::new();
  };
  items
    .iter()
    .filter_map(|item| {
      if let Ordered::String(text) = item {
        Some(text.clone())
      } else {
        None
      }
    })
    .collect()
}

fn as_json(value: &Ordered) -> Res<serde_json::Value> {
  serde_json::from_str(&value.to_text(false)).map_err(|err| err.to_string())
}

/// Whether `input` passes the schema the case names.
fn passes(schema: &str, input: &Ordered) -> Res<bool> {
  Ok(match schema {
    "gate" => validate_gate_file(input).is_ok(),
    "telemetry" => parse_telemetry_line(input).is_ok(),
    "edit" => parse_edit_record(&as_json(input)?).is_ok(),
    other => return Err(format!("unknown schema {other}")),
  })
}

#[test]
fn every_case_has_a_rust_runner_but_the_typescript_package_one() {
  let all = cases_of(CASES, "").unwrap_or_default();
  assert!(all.is_empty(), "no case lacks a kind");
  let kinds = [
    "schema",
    "telemetry-events",
    "telemetry-extras",
    "branch-slug",
    "base-branch",
  ];
  let more = [
    "current-branch",
    "branch-slugs",
    "diff-sha",
    "io",
    "gate-file",
    "concurrency",
  ];
  let counts: BTreeMap<&str, usize> = kinds
    .iter()
    .chain(&more)
    .map(|kind| (*kind, cases_of(CASES, kind).unwrap().len()))
    .collect();
  assert_eq!(counts.values().sum::<usize>(), 76);
  assert_eq!(cases_of(CASES, "package").unwrap().len(), 1);
}

#[test]
fn the_package_case_version_and_sample_document_hold_in_rust() {
  let [case] = &cases_of(CASES, "package").unwrap()[..] else {
    panic!("one case")
  };
  let version = Ordered::Number(GATE_FILE_VERSION.into());
  assert_eq!(field(case, "gateFileVersion").unwrap(), &version);
  assert!(validate_gate_file(field(case, "validGateFile").unwrap()).is_ok());
}

#[test]
fn schema_cases_accept_and_reject_as_zod_does() {
  let cases = cases_of(CASES, "schema").unwrap();
  assert_eq!(cases.len(), 22);
  for case in &cases {
    let valid = field(case, "valid").unwrap() == &Ordered::Bool(true);
    let ok = passes(
      &text(case, "schema").unwrap(),
      field(case, "input").unwrap(),
    )
    .unwrap();
    assert_eq!(ok, valid, "{}", text(case, "name").unwrap());
  }
}

#[test]
fn every_event_line_parses_and_the_event_set_is_closed() {
  let [case] = &cases_of(CASES, "telemetry-events").unwrap()[..] else {
    panic!("one case")
  };
  let Ordered::Array(lines) = field(case, "lines").unwrap() else {
    panic!("lines")
  };
  let parsed: Vec<TelemetryLine> = lines
    .iter()
    .map(|line| parse_telemetry_line(line).unwrap())
    .collect();
  let version = Ordered::Number(TELEMETRY_VERSION.into());
  assert!(lines.iter().all(|line| line.get("v") == Some(&version)));
  let mut seen: Vec<String> = parsed
    .iter()
    .map(|line| line.event.name().to_owned())
    .collect();
  seen.sort();
  let mut known: Vec<String> = TELEMETRY_EVENTS
    .iter()
    .map(|event| (*event).to_owned())
    .collect();
  known.sort();
  assert_eq!(seen, strings(field(case, "events").unwrap()));
  assert_eq!(known, seen);
}

#[test]
fn extras_never_take_protocol_keys() {
  let [case] = &cases_of(CASES, "telemetry-extras").unwrap()[..] else {
    panic!("one case")
  };
  let Ordered::Array(checks) = field(case, "checks").unwrap() else {
    panic!("checks")
  };
  for check in checks {
    let input = field(check, "input").unwrap();
    assert!(
      parse_telemetry_extras(&text(check, "event").unwrap(), input).is_err(),
      "{}",
      input.to_text(false)
    );
  }
}

#[test]
fn branch_slugs_match() {
  let cases = cases_of(CASES, "branch-slug").unwrap();
  assert_eq!(cases.len(), 7);
  for case in &cases {
    assert_eq!(
      branch_slug(&text(case, "branch").unwrap()),
      text(case, "expected").unwrap()
    );
  }
}

#[test]
fn base_branch_follows_origin_head() {
  for case in &cases_of(CASES, "base-branch").unwrap() {
    let sb = Sandbox::new(Some(&text(case, "initial").unwrap())).unwrap();
    let env = sb.env();
    assert_eq!(
      base_branch(&env, Some(&sb.project), &sb.project),
      text(case, "before").unwrap()
    );
    let remote = text(case, "remote").unwrap();
    sb.git(&["update-ref", &remote, "HEAD"]).unwrap();
    sb.git(&["symbolic-ref", "refs/remotes/origin/HEAD", &remote])
      .unwrap();
    assert_eq!(
      base_branch(&env, Some(&sb.project), &sb.project),
      text(case, "after").unwrap()
    );
  }
}

#[test]
fn current_branch_is_the_name_head_or_nothing() {
  for case in &cases_of(CASES, "current-branch").unwrap() {
    let expected = strings(field(case, "expected").unwrap());
    let repo = Sandbox::new(Some(&text(case, "branch").unwrap())).unwrap();
    let env = repo.env();
    let mut seen = vec![current_branch(&env, &repo.project)];
    repo.git(&["checkout", "-q", "--detach"]).unwrap();
    seen.push(current_branch(&env, &repo.project));
    let unborn = Sandbox::new(None).unwrap();
    unborn.git(&["init", "-q"]).unwrap();
    seen.push(current_branch(&env, &unborn.project));
    let plain = Sandbox::new(None).unwrap();
    seen.push(current_branch(&env, &plain.project));
    assert_eq!(seen, expected);
  }
}

#[test]
fn branch_lists_see_local_and_merged_branches() {
  for case in &cases_of(CASES, "branch-slugs").unwrap() {
    let get = |key: &str| text(case, key).unwrap();
    let sb = Sandbox::new(Some("main")).unwrap();
    sb.git(&["branch", &get("merged")]).unwrap();
    sb.git(&["checkout", "-q", "-b", &get("ahead")]).unwrap();
    std::fs::write(sb.project.join(get("file")), get("body")).unwrap();
    sb.git(&["add", &get("file")]).unwrap();
    sb.git(&["commit", "-q", "-m", "ahead"]).unwrap();
    let env = sb.env();
    let all: Vec<String> = branch_slugs(&env, &sb.project, None).into_iter().collect();
    assert_eq!(all, strings(field(case, "expectedAll").unwrap()));
    let merged: Vec<String> = branch_slugs(&env, &sb.project, Some(&get("base")))
      .into_iter()
      .collect();
    assert_eq!(merged, strings(field(case, "expectedMerged").unwrap()));
    assert!(branch_slugs(&env, &sb.project, Some(&get("missingBase"))).is_empty());
  }
}
