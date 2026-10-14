//! The three fixer hosts, their CLIs, default model routing per tier, and the
//! approval, name, model and effort flags a fixer pane is launched with —
//! the same contract the epic orchestrator launches workers with.

use crate::error::{Failure, Result};
use crate::pattern::{is, pattern};

pattern!(SAFE, r"^[A-Za-z0-9_./:=,@%+#-]+$");

/// The word that opens a rejected argument's message.
const UNSAFE: &str = "unsafe";

/// A host that can run a fixer agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Fixer {
  /// Claude Code.
  Claude,
  /// Codex.
  Codex,
  /// Cursor's agent CLI.
  Cursor,
}

/// A tier's model and effort; either may be left to the host.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Choice {
  /// The model id.
  pub(crate) model: Option<String>,
  /// The reasoning effort.
  pub(crate) effort: Option<String>,
}

impl Choice {
  fn of(model: &str, effort: Option<&str>) -> Choice {
    Choice {
      model: Some(model.to_owned()),
      effort: effort.map(str::to_owned),
    }
  }
}

/// Whether `text` is safe to pass unquoted to a pane's shell.
pub(crate) fn safe(text: &str) -> bool {
  is(&SAFE, text)
}

impl Fixer {
  /// Every fixer host.
  pub(crate) const ALL: [Fixer; 3] = [Fixer::Claude, Fixer::Codex, Fixer::Cursor];

  /// The host `name` names, accepting `claude-code` and `cursor-agent`.
  pub(crate) fn parse(name: &str) -> Result<Fixer> {
    match name.trim().to_lowercase().as_str() {
      "claude" | "claude-code" => Ok(Fixer::Claude),
      "codex" => Ok(Fixer::Codex),
      "cursor" | "cursor-agent" => Ok(Fixer::Cursor),
      _ => Err(
        Failure::new(
          "config_invalid",
          format!("unknown host '{name}'; use one of claude, codex, cursor"),
        )
        .with("host", name),
      ),
    }
  }

  /// The host's name.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Fixer::Claude => "claude",
      Fixer::Codex => "codex",
      Fixer::Cursor => "cursor",
    }
  }

  /// The host's command.
  pub(crate) fn cli(self) -> &'static str {
    match self {
      Fixer::Cursor => "cursor-agent",
      Fixer::Claude | Fixer::Codex => self.name(),
    }
  }

  /// The default model and effort per tier, trivial to critical.
  pub(crate) fn defaults(self) -> Vec<Choice> {
    match self {
      Fixer::Claude => vec![
        Choice::of("sonnet", Some("low")),
        Choice::of("sonnet", Some("medium")),
        Choice::of("opus", Some("high")),
        Choice::of("opus", Some("xhigh")),
      ],
      Fixer::Codex => ["low", "medium", "high", "xhigh"]
        .iter()
        .map(|effort| Choice::of("gpt-6-sol", Some(effort)))
        .collect(),
      Fixer::Cursor => [
        "composer-2.5",
        "gpt-5.6-sol-high",
        "claude-opus-5-thinking-high",
        "gpt-5.6-sol-xhigh",
      ]
      .iter()
      .map(|model| Choice::of(model, None))
      .collect(),
    }
  }

  fn approvals(self, unattended: bool) -> &'static [&'static str] {
    match (self, unattended) {
      (Fixer::Claude, true) => &["--dangerously-skip-permissions"],
      (Fixer::Codex, true) => &["--dangerously-bypass-approvals-and-sandbox"],
      (Fixer::Cursor, true) => &["--yolo", "--trust", "--approve-mcps"],
      (Fixer::Claude, false) => &["--permission-mode", "auto"],
      (Fixer::Codex, false) => &[
        "--ask-for-approval",
        "on-request",
        "--sandbox",
        "workspace-write",
      ],
      (Fixer::Cursor, false) => &["--trust"],
    }
  }

  fn effort(self, effort: &str) -> Vec<String> {
    match self {
      Fixer::Claude => vec!["--effort".into(), effort.into()],
      Fixer::Codex => vec!["-c".into(), format!("model_reasoning_effort={effort}")],
      Fixer::Cursor => Vec::new(),
    }
  }

  /// A fixer's approval, name, model and effort flags, every one shell-safe.
  ///
  /// # Errors
  /// `config_invalid` naming the first unsafe argument.
  pub(crate) fn args(
    self,
    name: &str,
    model: Option<&str>,
    effort: Option<&str>,
    unattended: bool,
  ) -> Result<Vec<String>> {
    let mut args: Vec<String> = Vec::new();
    if self == Fixer::Codex {
      args.push("--no-daemon".into());
    }
    args.extend(
      self
        .approvals(unattended)
        .iter()
        .map(|arg| (*arg).to_owned()),
    );
    if self == Fixer::Claude {
      args.extend(["-n".to_owned(), name.to_owned()]);
    }
    if let Some(model) = model.filter(|model| !model.is_empty()) {
      args.extend(["--model".to_owned(), model.to_owned()]);
    }
    if let Some(effort) = effort.filter(|effort| !effort.is_empty()) {
      args.extend(self.effort(effort));
    }
    if let Some(bad) = args.iter().find(|arg| !safe(arg)) {
      return Err(
        Failure::new(
          "config_invalid",
          format!("{UNSAFE} {} arg for the pane shell: {bad}", self.name()),
        )
        .with("arg", bad.clone()),
      );
    }
    Ok(args)
  }
}

#[cfg(test)]
#[path = "tests/host_test.rs"]
mod tests;
