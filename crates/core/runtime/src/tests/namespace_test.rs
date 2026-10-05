use clap::error::ErrorKind;
use serde_json::{Value, json};

use super::{Guide, PLACEHOLDER, Planned};
use crate::cli::Ctx;

const EPIC: Planned = Planned {
  name: "epic",
  about: "Drive an epic to merged PRs",
  verbs: &["engine", "start", "gate", "queue"],
  issues: &[434, 435, 448],
};

const DOCTOR: Planned = Planned {
  name: "doctor",
  about: "Check the installation",
  verbs: &[],
  issues: &[445],
};

const GUIDE: Guide = Guide {
  name: "brainstorm",
  about: "Think a change through before building",
  text: "brainstorm is Markdown only.\nRun /brainstorm:brainstorm.",
};

fn json_ctx() -> Ctx {
  Ctx {
    json: true,
    ..Ctx::default()
  }
}

#[test]
fn a_planned_namespace_has_exactly_the_placeholder_verb() {
  let command = EPIC.command();
  let verbs: Vec<&str> = command
    .get_subcommands()
    .map(clap::Command::get_name)
    .collect();
  assert_eq!(verbs, [PLACEHOLDER]);
  let about = command
    .find_subcommand(PLACEHOLDER)
    .and_then(|verb| verb.get_about())
    .map(ToString::to_string);
  assert_eq!(
    about.as_deref(),
    Some("Not ported yet (#434, #435, #448): show the planned verbs")
  );
}

#[test]
fn a_missing_verb_is_a_usage_error_that_shows_help() {
  let err = EPIC.command().try_get_matches_from(["epic"]).unwrap_err();
  assert_eq!(
    err.kind(),
    ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
  );
  assert!(
    EPIC
      .command()
      .try_get_matches_from(["epic", "planned"])
      .is_ok()
  );
}

#[test]
fn the_placeholder_names_the_planned_verbs_and_issues() {
  let outcome = EPIC.run(&Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(
      "toolu epic is not ported yet (#434, #435, #448). Planned verbs: engine, start, gate, queue"
    )
  );
  assert_eq!(outcome.stderr, None);
}

#[test]
fn a_namespace_without_verbs_says_the_command_itself_is_planned() {
  let outcome = DOCTOR.run(&Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(
      "toolu doctor is not ported yet (#445). Planned verbs: none (the command itself is planned)"
    )
  );
}

#[test]
fn under_json_the_placeholder_is_one_document() {
  let stdout = EPIC.run(&json_ctx()).stdout.unwrap();
  let doc: Value = serde_json::from_str(&stdout).unwrap();
  assert_eq!(
    doc,
    json!({
      "namespace": "epic",
      "ported": false,
      "planned": ["engine", "start", "gate", "queue"],
      "issues": [434, 435, 448],
    })
  );
}

#[test]
fn a_guide_prints_its_text_and_shows_it_as_long_help() {
  assert_eq!(
    GUIDE.run(&Ctx::default()).stdout.as_deref(),
    Some(GUIDE.text)
  );
  let help = GUIDE.command().render_long_help().to_string();
  assert!(help.contains("Run /brainstorm:brainstorm."), "{help}");
  assert_eq!(GUIDE.command().get_subcommands().count(), 0);
}

#[test]
fn under_json_a_guide_is_one_document() {
  let stdout = GUIDE.run(&json_ctx()).stdout.unwrap();
  let doc: Value = serde_json::from_str(&stdout).unwrap();
  assert_eq!(
    doc,
    json!({ "namespace": "brainstorm", "about": GUIDE.about, "text": GUIDE.text })
  );
}
