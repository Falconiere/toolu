//! `cp`/`mv`/`install` and the in-place editors `sed -i`/`perl -i`
//! (`copyTargets`, `inPlaceTargets` in `shell-writes.ts`).

use crate::analysis::ShellCommand;
use crate::argv::basename;
use crate::options::{OptionSpec, named, parse_args};
use crate::parse::MAX_SHELL_INPUT;
use crate::writes::{Target, arg_at, operands};

const MOVE: OptionSpec = OptionSpec {
  value_short: "tS",
  rest_short: "",
  value_long: "target-directory suffix",
  numeric: false,
  stop_at_operand: false,
  plus: false,
};

const INSTALL: OptionSpec = OptionSpec {
  value_short: "tSmog",
  rest_short: "",
  value_long: "target-directory suffix mode owner group strip-program",
  numeric: false,
  stop_at_operand: false,
  plus: false,
};

/// `dir/basename(source)`, or `None` when either is unknown.
fn join(dir: Option<&str>, source: Option<&str>) -> Option<String> {
  let (dir, source) = (dir?, source?);
  Some(format!(
    "{}/{}",
    dir.trim_end_matches('/'),
    basename(source)
  ))
}

/// `source` copied into the directory `dir`.
fn in_dir(dir: &Target, source: &Target) -> Target {
  let path = join(dir.path.as_deref(), source.path.as_deref());
  let pattern = if path.is_none() {
    let dir_any = dir.path.as_deref().or(dir.pattern.as_deref());
    join(
      dir_any,
      source.path.as_deref().or(source.pattern.as_deref()),
    )
  } else {
    None
  };
  let text = join(Some(&dir.text), Some(&source.text)).unwrap_or_default();
  Target {
    path,
    pattern,
    text,
  }
}

/// The most bytes the sources inside one directory may take. Each copies the
/// directory's name, so many sources and a long name would grow without bound;
/// past this they are one unknown target, as cautious as any path.
const MAX_INSIDE_BYTES: usize = 4 * MAX_SHELL_INPUT;

fn size(target: &Target) -> usize {
  let path = target.path.as_ref().map_or(0, String::len);
  path + target.pattern.as_ref().map_or(0, String::len) + target.text.len()
}

/// Each of `sources` inside the directory `dir`.
fn inside(dir: &Target, sources: &[Target]) -> Vec<Target> {
  let total: usize = sources
    .iter()
    .map(|source| size(dir) + size(source) + 3)
    .sum();
  if total > MAX_INSIDE_BYTES {
    let text = dir.text.clone();
    return vec![Target {
      path: None,
      pattern: None,
      text,
    }];
  }
  sources.iter().map(|source| in_dir(dir, source)).collect()
}

/// cp/mv/install: the destination and each source inside it, or each source inside `-t DIR`.
pub(super) fn copy_targets(command: &ShellCommand, install: bool) -> Vec<Target> {
  let parsed = parse_args(&command.argv, 1, if install { &INSTALL } else { &MOVE });
  let mut files = operands(command, &parsed);
  if install && parsed.has("d directory") {
    return files;
  }
  let dir = parsed
    .options
    .iter()
    .find(|option| named("t target-directory", option.name));
  if let Some(dir) = dir {
    let attached = || {
      let path = dir.value.text().map(str::to_owned);
      let text = path.clone().unwrap_or_default();
      Target {
        path,
        pattern: None,
        text,
      }
    };
    let into = dir.at.map_or_else(attached, |at| arg_at(command, at));
    return inside(&into, &files);
  }
  let Some(dest) = files.pop() else {
    return Vec::new();
  };
  if files.is_empty() {
    return Vec::new();
  }
  // DEST may be an existing directory (known only at run time), so each
  // DEST/basename(SRC) is a candidate too: `cp .env.example apps/web`.
  if dest.path.is_none() && dest.pattern.is_none() {
    return vec![dest];
  }
  let within = inside(&dest, &files);
  std::iter::once(dest).chain(within).collect()
}

/// sed/perl edit files only in place; their first operand is the script unless
/// a `script` option gave one. BSD `sed -i ''` takes the empty word as the suffix.
fn in_place(command: &ShellCommand, spec: &OptionSpec, script: &str) -> Vec<Target> {
  let parsed = parse_args(&command.argv, 1, spec);
  if !parsed.has("i in-place") {
    return Vec::new();
  }
  let bsd = command.argv.windows(2).position(
    |pair| matches!(pair, [Some(flag), Some(suffix)] if flag == "-i" && suffix.is_empty()),
  );
  let files: Vec<usize> = parsed
    .operand_at
    .iter()
    .copied()
    .filter(|at| bsd.is_none_or(|flag| *at != flag + 1))
    .collect();
  let skip = usize::from(!parsed.has(script));
  files
    .iter()
    .skip(skip)
    .map(|at| arg_at(command, *at))
    .collect()
}

/// `sed -i`.
pub(super) fn sed(command: &ShellCommand) -> Vec<Target> {
  let spec = OptionSpec {
    value_short: "efl",
    rest_short: "i",
    value_long: "expression file line-length",
    ..OptionSpec::default()
  };
  in_place(command, &spec, "e f expression file")
}

/// `perl -i`.
pub(super) fn perl(command: &ShellCommand) -> Vec<Target> {
  let spec = OptionSpec {
    value_short: "eE",
    rest_short: "iIMmlx0dDC",
    ..OptionSpec::default()
  };
  in_place(command, &spec, "e E")
}

#[cfg(test)]
#[path = "tests/copy_test.rs"]
mod tests;
