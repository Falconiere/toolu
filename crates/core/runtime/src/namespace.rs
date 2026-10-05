//! The two namespace shapes a plugin crate has before its port (#442): `Planned`,
//! one placeholder verb that names the planned verbs and their issues, and
//! `Guide`, the help namespace of a Markdown-only plugin. Each plugin crate
//! declares one as data, so the crates stay a few lines.

use clap::Command;
use serde_json::json;

use crate::cli::{Ctx, Outcome};

/// The placeholder verb of every namespace that is not ported yet.
pub const PLACEHOLDER: &str = "planned";

/// A namespace whose verbs are not ported yet.
#[derive(Debug)]
pub struct Planned {
  /// The namespace: `toolu <name>`.
  pub name: &'static str,
  /// One line for `toolu --help`.
  pub about: &'static str,
  /// The verbs its port plans, in the order the issue lists them.
  pub verbs: &'static [&'static str],
  /// The issues that port it.
  pub issues: &'static [u32],
}

impl Planned {
  /// The namespace with its placeholder verb; a missing verb is a usage error.
  pub fn command(&self) -> Command {
    Command::new(self.name)
      .about(self.about)
      .subcommand_required(true)
      .arg_required_else_help(true)
      .subcommand(Command::new(PLACEHOLDER).about(format!(
        "Not ported yet ({}): show the planned verbs",
        self.issue_list()
      )))
  }

  /// The placeholder verb: what is planned and who ports it.
  pub fn run(&self, ctx: &Ctx) -> Outcome {
    if ctx.json {
      let doc = json!({
        "namespace": self.name,
        "ported": false,
        "planned": self.verbs,
        "issues": self.issues,
      });
      return Outcome::data(doc.to_string());
    }
    let verbs = if self.verbs.is_empty() {
      "none; the command itself is planned".to_owned()
    } else {
      self.verbs.join(", ")
    };
    Outcome::data(format!(
      "toolu {} is not ported yet ({}).\nPlanned verbs: {verbs}",
      self.name,
      self.issue_list()
    ))
  }

  fn issue_list(&self) -> String {
    let issues: Vec<String> = self
      .issues
      .iter()
      .map(|issue| format!("#{issue}"))
      .collect();
    issues.join(", ")
  }
}

/// The help namespace of a Markdown-only plugin: no verbs, only its guide.
#[derive(Debug)]
pub struct Guide {
  /// The namespace: `toolu <name>`.
  pub name: &'static str,
  /// One line for `toolu --help`.
  pub about: &'static str,
  /// The guide `toolu <name>` prints and `--help` shows.
  pub text: &'static str,
}

impl Guide {
  /// The namespace, its guide as the long help.
  pub fn command(&self) -> Command {
    Command::new(self.name)
      .about(self.about)
      .long_about(self.text)
  }

  /// `toolu <name>`: the guide.
  pub fn run(&self, ctx: &Ctx) -> Outcome {
    if ctx.json {
      let doc = json!({ "namespace": self.name, "about": self.about, "text": self.text });
      return Outcome::data(doc.to_string());
    }
    Outcome::data(self.text.to_owned())
  }
}

#[cfg(test)]
#[path = "tests/namespace_test.rs"]
mod tests;
