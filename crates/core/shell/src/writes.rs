//! The files a command line writes (`shell-writes.ts`): output redirections on
//! any command or compound command, and the file operands of commands that
//! write in place or copy. A dynamic target is reported with `path: None`
//! rather than dropped, and an unquoted pathname pattern (`> .en[v]`) with its
//! `pattern`: bash writes whichever existing file the pattern matches.

mod copy;
mod python;

use crate::analysis::{RedirectOperator, ShellAnalysis, ShellCommand, ShellRedirect};
use crate::argv::basename;
use crate::options::{OptionSpec, ParsedArgs, parse_args};

/// How a file is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteVia {
  /// An output redirection.
  Redirect,
  /// `tee FILE…`
  Tee,
  /// `sed -i`
  Sed,
  /// `perl -i`
  Perl,
  /// `cp`
  Cp,
  /// `mv`
  Mv,
  /// `install`
  Install,
  /// `dd of=`
  Dd,
  /// `open(…, 'w')` in a python script.
  Python,
}

impl WriteVia {
  /// The name TypeScript uses.
  pub fn as_str(self) -> &'static str {
    match self {
      WriteVia::Redirect => "redirect",
      WriteVia::Tee => "tee",
      WriteVia::Sed => "sed",
      WriteVia::Perl => "perl",
      WriteVia::Cp => "cp",
      WriteVia::Mv => "mv",
      WriteVia::Install => "install",
      WriteVia::Dd => "dd",
      WriteVia::Python => "python",
    }
  }
}

/// A file the line writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteTarget<'a> {
  /// The written path's static value, or `None` when it is dynamic or a pathname pattern.
  pub path: Option<String>,
  /// An unquoted pathname pattern: a guardrail must treat it as every path it matches.
  pub pattern: Option<String>,
  /// The target with quotes removed and expansions as written (`$HOME/.env`).
  pub text: String,
  /// How it is written.
  pub via: WriteVia,
  /// The command that writes; `None` for a redirect on a compound command.
  pub command: Option<&'a ShellCommand>,
}

/// A written file before it is tied to its command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
  pub(crate) path: Option<String>,
  pub(crate) pattern: Option<String>,
  pub(crate) text: String,
}

/// `>`, `>>`, `>|`, `&>`, `&>>`, `<>`, and `>&` onto a file rather than a descriptor.
fn writes_file(redirect: &ShellRedirect) -> bool {
  match redirect.operator {
    RedirectOperator::DupOut => {
      let target = redirect.target.as_deref().unwrap_or_default();
      !(target == "-" || (!target.is_empty() && target.bytes().all(|b| b.is_ascii_digit())))
    }
    RedirectOperator::Out
    | RedirectOperator::Append
    | RedirectOperator::Clobber
    | RedirectOperator::OutErr
    | RedirectOperator::AppendOutErr
    | RedirectOperator::ReadWrite => true,
    RedirectOperator::In
    | RedirectOperator::Heredoc
    | RedirectOperator::HeredocStrip
    | RedirectOperator::HereString
    | RedirectOperator::DupIn => false,
  }
}

/// The argument at `index` as a target.
pub(crate) fn arg_at(command: &ShellCommand, index: usize) -> Target {
  Target {
    path: command.argv.get(index).cloned().flatten(),
    pattern: command.patterns.get(index).cloned().flatten(),
    text: command.texts.get(index).cloned().unwrap_or_default(),
  }
}

/// Every operand as a target.
pub(crate) fn operands(command: &ShellCommand, parsed: &ParsedArgs<'_>) -> Vec<Target> {
  parsed
    .operand_at
    .iter()
    .map(|at| arg_at(command, *at))
    .collect()
}

/// `dd`: `of=…` as a whole never names an existing file, so it is not globbed.
fn dd(command: &ShellCommand) -> Vec<Target> {
  let mut found = Vec::new();
  for (index, text) in command.texts.iter().enumerate() {
    let Some(shown) = text.strip_prefix("of=") else {
      continue;
    };
    let word = command.argv.get(index).cloned().flatten();
    let word = word.or_else(|| command.patterns.get(index).cloned().flatten());
    let path = word.and_then(|word| word.strip_prefix("of=").map(str::to_owned));
    found.push(Target {
      path,
      pattern: None,
      text: shown.to_owned(),
    });
  }
  found
}

/// The writer `name` stands for, and the files it writes.
fn targets_of(name: &str, command: &ShellCommand) -> Option<(WriteVia, Vec<Target>)> {
  let python = name
    .strip_prefix("python")
    .is_some_and(|rest| rest.bytes().all(|b| b.is_ascii_digit() || b == b'.'));
  Some(match name {
    "tee" => (
      WriteVia::Tee,
      operands(
        command,
        &parse_args(&command.argv, 1, &OptionSpec::default()),
      ),
    ),
    "sed" => (WriteVia::Sed, copy::sed(command)),
    "perl" => (WriteVia::Perl, copy::perl(command)),
    "cp" => (WriteVia::Cp, copy::copy_targets(command, false)),
    "mv" => (WriteVia::Mv, copy::copy_targets(command, false)),
    "install" => (WriteVia::Install, copy::copy_targets(command, true)),
    "dd" => (WriteVia::Dd, dd(command)),
    _ if python => (WriteVia::Python, python::targets(command)),
    _ => return None,
  })
}

fn redirect_targets<'a>(
  redirects: &[ShellRedirect],
  command: Option<&'a ShellCommand>,
) -> Vec<WriteTarget<'a>> {
  redirects
    .iter()
    .filter(|redirect| writes_file(redirect))
    .map(|redirect| WriteTarget {
      path: redirect.target.clone(),
      pattern: redirect.pattern.clone(),
      text: redirect.text.clone(),
      via: WriteVia::Redirect,
      command,
    })
    .collect()
}

/// Every write target in the line: per command its redirects, then the files
/// its arguments name; then the redirects on compound commands.
pub fn write_targets(analysis: &ShellAnalysis) -> Vec<WriteTarget<'_>> {
  let mut targets = Vec::new();
  for command in &analysis.commands {
    targets.extend(redirect_targets(&command.redirects, Some(command)));
    let name = command.argv.first().cloned().flatten().unwrap_or_default();
    if let Some((via, found)) = targets_of(basename(&name), command) {
      targets.extend(found.into_iter().map(|target| WriteTarget {
        path: target.path,
        pattern: target.pattern,
        text: target.text,
        via,
        command: Some(command),
      }));
    }
  }
  targets.extend(redirect_targets(&analysis.compound_redirects, None));
  targets
}

#[cfg(test)]
#[path = "tests/writes_test.rs"]
mod tests;
