//! Settings-driven shell allow and deny rules, checked on every simple command.

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::config::gate_mode::{GateMode, guardrail_warning};
use toolu_runtime::config::settings::{BASH_ALLOWLIST, BASH_DENYLIST, read_list};
use toolu_runtime::registry::rule::RuleContext;
use toolu_shell::analysis::{ShellAnalysis, ShellCommand};
use toolu_shell::rules::matches_rule;

use super::{command_analysis, decided, gate_config, gate_settings_dir, pre_mode};
use crate::gate::Gate;

/// The shell deny-list gate.
pub(crate) struct BashCommands;
/// Its singleton in the ordered pre-tool table.
pub(crate) static BASH_COMMANDS: BashCommands = BashCommands;

#[derive(Debug, PartialEq, Eq)]
enum Verdict {
  Allow,
  Deny(String),
  Unknown(String),
}

fn matches(command: &ShellCommand, rule: &str) -> bool {
  if rule.contains(' ') {
    matches_rule(command, rule)
  } else {
    command.text.contains(rule)
  }
}

fn verdict(analysis: &ShellAnalysis, allow: &[String], deny: &[String]) -> Verdict {
  if analysis.unknown {
    let why = analysis
      .errors
      .first()
      .map_or("no command could be read", |error| error.message.as_str());
    return Verdict::Unknown(why.to_owned());
  }
  for rule in deny {
    let blocked = analysis.commands.iter().any(|command| {
      matches(command, rule)
        && !allow
          .iter()
          .any(|override_rule| matches(command, override_rule))
    });
    if blocked {
      return Verdict::Deny(rule.clone());
    }
  }
  Verdict::Allow
}

const WHY_GUARDED: &str = "Rules in settings/bash-denylist.txt cover commands that execute arbitrary code from a string (node -e, bun -e) or that this project has ruled out.";

fn rule_reason(mode: GateMode, rule: &str) -> String {
  match mode {
    GateMode::Ask => guardrail_warning(
      &format!("Claude wants to run a command matching the deny rule \"{rule}\"."),
      &format!("{WHY_GUARDED} The command runs with your full shell privileges if you approve."),
    ),
    GateMode::Advise => format!(
      "Command matches deny rule \"{rule}\" (plugins/toolu/settings/bash-denylist.txt). The command was not stopped — gates.bashCommands.mode is 'advise'."
    ),
    GateMode::Block | GateMode::Off => format!("Command blocked by deny rule: {rule}"),
  }
}

fn unknown_reason(mode: GateMode, why: &str) -> String {
  match mode {
    GateMode::Ask => guardrail_warning(
      &format!("Claude wants to run a command toolu could not analyze ({why})."),
      &format!(
        "{WHY_GUARDED} A command that cannot be parsed cannot be checked against them. It runs with your full shell privileges if you approve."
      ),
    ),
    GateMode::Advise => format!(
      "Command could not be analyzed ({why}), so plugins/toolu/settings/bash-denylist.txt was not checked. The command was not stopped — gates.bashCommands.mode is 'advise'."
    ),
    GateMode::Block | GateMode::Off => {
      format!("Command blocked: it could not be analyzed against the deny rules ({why})")
    }
  }
}

impl Gate for BashCommands {
  fn name(&self) -> &'static str {
    "bash-commands"
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String> {
    if !matches!(event, NormalizedEvent::ShellPre { .. }) {
      return Ok(Decision::Allow);
    }
    let Some(dir) = gate_settings_dir(ctx) else {
      return Ok(Decision::Allow);
    };
    let deny = read_list(&dir.join(BASH_DENYLIST))?;
    if deny.is_empty() {
      return Ok(Decision::Allow);
    }
    let allow = read_list(&dir.join(BASH_ALLOWLIST))?;
    let hit = verdict(&command_analysis(ctx), &allow, &deny);
    if hit == Verdict::Allow {
      return Ok(Decision::Allow);
    }
    let mode = pre_mode(&gate_config(ctx), "bashCommands", ctx, true);
    let reason = match hit {
      Verdict::Deny(rule) => rule_reason(mode, &rule),
      Verdict::Unknown(why) => unknown_reason(mode, &why),
      Verdict::Allow => return Ok(Decision::Allow),
    };
    decided(mode, reason)
  }
}

#[cfg(test)]
#[path = "tests/bash_commands_test.rs"]
mod tests;
