//! `fixtures/state/cases.json` against the Rust state layer (AC-4, AC-11):
//! the schema, telemetry, branch and diff families here, the I/O family in
//! `io_cases.rs`, the gate-file family in `gate_cases.rs`, the concurrency
//! family in `interleave.rs`. The one `package` case checks TypeScript's
//! module exports and has no Rust counterpart.

#[path = "helpers/cases.rs"]
mod cases;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::collections::BTreeMap;
use std::path::Path;

use cases::{cases_of, field, text};
use sandbox::{Res, Sandbox};
use toolu_runtime::json::ordered::Ordered;
use toolu_state::diff_sha::diff_sha;
use toolu_state::edit_records::parse_edit_record;
use toolu_state::gate_schema::validate_gate_file;
use toolu_state::git::{base_branch, branch_slug, branch_slugs, current_branch};
use toolu_state::telemetry::TELEMETRY_EVENTS;
use toolu_state::telemetry_schema::{parse_telemetry_extras, parse_telemetry_line};

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
  let mut seen: Vec<String> = lines
    .iter()
    .map(|line| parse_telemetry_line(line).unwrap().event.name().to_owned())
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

/// `{ "$template" | "$path": … }` with `$ROOT`, `$PROJECT` and `$HOME` filled in.
fn materialize(sb: &Sandbox, value: &Ordered) -> Res<String> {
  let raw = match value {
    Ordered::String(text) => return Ok(text.clone()),
    Ordered::Object(_) => text(value, "$template").or_else(|_| text(value, "$path"))?,
    Ordered::Null | Ordered::Bool(_) | Ordered::Number(_) | Ordered::Array(_) => {
      return Err("not a string or a tagged value".to_owned());
    }
  };
  let root = sb
    .project
    .parent()
    .map(Path::to_path_buf)
    .unwrap_or_default();
  let pairs = [
    ("$PROJECT", &sb.project),
    ("$HOME", &sb.home),
    ("$ROOT", &root),
  ];
  Ok(pairs.iter().fold(raw, |raw, (token, path)| {
    raw.replace(token, &path.display().to_string())
  }))
}

/// The case's repository: `files` committed on `main` when `git`, then its steps.
fn diff_repo(case: &Ordered) -> Res<Sandbox> {
  let sb = Sandbox::new(None)?;
  let Ordered::Object(files) = field(case, "files")? else {
    return Err("files".to_owned());
  };
  for (path, body) in files {
    let Ordered::String(body) = body else {
      return Err("body".to_owned());
    };
    std::fs::write(sb.project.join(path), body).map_err(|err| err.to_string())?;
  }
  if field(case, "git")? == &Ordered::Bool(true) {
    sb.git(&["init", "-q", "-b", "main"])?;
    sb.git(&["add", "-A"])?;
    sb.git(&[
      "commit",
      "-q",
      "--allow-empty",
      "-m",
      "harness: initial commit",
    ])?;
  }
  let Ordered::Array(steps) = field(case, "steps")? else {
    return Err("steps".to_owned());
  };
  for step in steps {
    apply_step(&sb, step)?;
  }
  Ok(sb)
}

/// One `git`, `write` or `repeat-write` step.
fn apply_step(sb: &Sandbox, step: &Ordered) -> Res<()> {
  let op = text(step, "op")?;
  if op == "git" {
    let args = strings(field(step, "args")?);
    sb.git(&args.iter().map(String::as_str).collect::<Vec<_>>())?;
    return Ok(());
  }
  let body = text(step, "body")?;
  let body = if op == "repeat-write" {
    let Ordered::Number(count) = field(step, "count")? else {
      return Err("count".to_owned());
    };
    body.repeat(usize::try_from(count.as_u64().ok_or("count")?).map_err(|err| err.to_string())?)
  } else {
    body
  };
  std::fs::write(sb.project.join(text(step, "path")?), body).map_err(|err| err.to_string())
}

#[test]
fn diff_hashes_match_the_typescript_cases() {
  let cases = cases_of(CASES, "diff-sha").unwrap();
  assert_eq!(cases.len(), 6);
  for case in &cases {
    let name = text(case, "name").unwrap();
    let sb = diff_repo(case).unwrap();
    let base = materialize(&sb, field(case, "base").unwrap()).unwrap();
    let sha = diff_sha(&sb.env(), &sb.project, &base);
    match text(case, "expect").unwrap().as_str() {
      "undefined" => assert_eq!(sha, None, "{name}"),
      "empty-blob" => assert_eq!(sha, Some(text(case, "hash").unwrap()), "{name}"),
      _ => {
        let sha = sha.unwrap();
        assert!(
          sha.len() >= 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()),
          "{name}"
        );
        assert_ne!(Some(sha), text(case, "notHash").ok(), "{name}");
      }
    }
    if let Some(planted) = case.get("planted") {
      assert!(
        !std::path::Path::new(&materialize(&sb, planted).unwrap()).exists(),
        "{name}"
      );
    }
  }
}
