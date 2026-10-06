//! What a simple command actually runs (`shell-argv.ts`): wrapper commands are
//! peeled (`sudo -u me timeout 60 git push` runs `git push`), and a shell or
//! `eval` that runs a string names the string to analyze next.

use crate::analysis::{CommandOrigin, Word};
use crate::options::{OptionSpec, parse_args};

/// A wrapper's option table. Names are space-separated.
struct Wrapper {
  /// Its own options; parsing stops at the command.
  options: OptionSpec,
  /// Options after which it runs no command (`command -v`, `sudo -l`).
  inert: &'static str,
  /// Options that hide the command from static reading (`env -S STRING`).
  opaque: &'static str,
  /// `NAME=value` words may sit between the options and the command.
  assignments: bool,
  /// Operands before the command (`timeout DURATION`).
  operands: usize,
  /// Appends arguments read at run time (`xargs`).
  appends_dynamic: bool,
  /// A bare `-` is an option, not the command (`env -` means `env -i`).
  dash_option: bool,
}

const PLAIN: Wrapper = Wrapper {
  options: OptionSpec {
    value_short: "",
    rest_short: "",
    value_long: "",
    numeric: false,
    stop_at_operand: true,
    plus: false,
  },
  inert: "",
  opaque: "",
  assignments: false,
  operands: 0,
  appends_dynamic: false,
  dash_option: false,
};

/// Options with value letters and long names, stopping at the command.
const fn takes(value_short: &'static str, value_long: &'static str) -> OptionSpec {
  OptionSpec {
    value_short,
    value_long,
    ..PLAIN.options
  }
}

/// The wrapper table, keyed by basename.
const WRAPPERS: [(&str, Wrapper); 12] = [
  (
    "sudo",
    Wrapper {
      options: takes("ugCDprtTU", "user group close-from chdir prompt role type"),
      inert: "e l v K V h edit list validate remove-timestamp version help",
      assignments: true,
      ..PLAIN
    },
  ),
  (
    "doas",
    Wrapper {
      options: takes("uC", ""),
      inert: "L",
      ..PLAIN
    },
  ),
  (
    "env",
    Wrapper {
      options: takes("uCPa", "unset chdir argv0"),
      opaque: "S split-string",
      assignments: true,
      dash_option: true,
      ..PLAIN
    },
  ),
  (
    "command",
    Wrapper {
      inert: "v V",
      ..PLAIN
    },
  ),
  ("builtin", PLAIN),
  (
    "exec",
    Wrapper {
      options: takes("a", ""),
      ..PLAIN
    },
  ),
  ("nohup", PLAIN),
  (
    "time",
    Wrapper {
      options: takes("fo", "format output"),
      ..PLAIN
    },
  ),
  (
    "nice",
    Wrapper {
      options: OptionSpec {
        numeric: true,
        ..takes("n", "adjustment")
      },
      ..PLAIN
    },
  ),
  (
    "timeout",
    Wrapper {
      options: takes("sk", "signal kill-after"),
      operands: 1,
      ..PLAIN
    },
  ),
  (
    "xargs",
    Wrapper {
      options: takes(
        "adEILnPs",
        "arg-file delimiter max-args max-procs max-chars process-slot-var",
      ),
      appends_dynamic: true,
      ..PLAIN
    },
  ),
  (
    "stdbuf",
    Wrapper {
      options: takes("ioe", "input output error"),
      ..PLAIN
    },
  ),
];

/// The wrapper `name` (a basename) stands for.
fn wrapper(name: &str) -> Option<&'static Wrapper> {
  let found = WRAPPERS.iter().find(|(known, _)| *known == name);
  found.map(|(_, spec)| spec)
}

/// The last path component (`/usr/bin/sudo` is `sudo`), as `node:path` `basename` reads it.
pub(crate) fn basename(path: &str) -> &str {
  let trimmed = path.trim_end_matches('/');
  match trimmed.rsplit_once('/') {
    Some((_, name)) => name,
    None if trimmed.is_empty() && !path.is_empty() => "",
    None => trimmed,
  }
}

/// `NAME=value`.
fn is_assignment(word: &str) -> bool {
  let Some((name, _)) = word.split_once('=') else {
    return false;
  };
  let mut chars = name.chars();
  chars
    .next()
    .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
    && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Where the wrapped command starts.
enum Inner {
  /// At this index of the wrapper's words.
  At(usize),
  /// Hidden from static reading.
  Opaque,
  /// No command runs.
  None,
}

fn inner_start(words: &[Word], wrapper: &Wrapper) -> Inner {
  let parsed = parse_args(words, 1, &wrapper.options);
  if parsed.missing_value || parsed.has(wrapper.inert) {
    return Inner::None;
  }
  if parsed.has(wrapper.opaque) {
    return Inner::Opaque;
  }
  let mut start = parsed.next;
  while let Some(word) = words.get(start) {
    let word = word.as_deref().unwrap_or_default();
    let dash = wrapper.dash_option && word == "-";
    if !(dash || wrapper.assignments && is_assignment(word)) {
      break;
    }
    start += 1;
  }
  start += wrapper.operands;
  if start < words.len() {
    Inner::At(start)
  } else {
    Inner::None
  }
}

/// The wrappers peeled off a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Unwrapped {
  /// Wrapper names, outermost first.
  pub(crate) wrappers: Vec<String>,
  /// Index in the words where the command that runs starts; `None` when hidden (`env -S`).
  pub(crate) start: Option<usize>,
  /// The command gets arguments read at run time (`xargs`).
  pub(crate) appends_dynamic: bool,
}

/// Peel wrapper commands off `words` until the command that actually runs.
pub(crate) fn unwrap(words: &[Word]) -> Unwrapped {
  let mut unwrapped = Unwrapped {
    wrappers: Vec::new(),
    start: Some(0),
    appends_dynamic: false,
  };
  let mut start = 0;
  while let Some(Some(name)) = words.get(start) {
    let Some(spec) = wrapper(basename(name)) else {
      break;
    };
    let rest = words.get(start..).unwrap_or_default();
    let inner = inner_start(rest, spec);
    if matches!(inner, Inner::None) {
      break;
    }
    unwrapped.wrappers.push(basename(name).to_owned());
    let Inner::At(offset) = inner else {
      unwrapped.start = None;
      return unwrapped;
    };
    unwrapped.appends_dynamic |= spec.appends_dynamic;
    start += offset;
  }
  unwrapped.start = Some(start);
  unwrapped
}

/// `list` (aligned with the words) cut to the command that runs; `fill` stands for unknown words.
pub(crate) fn align<T: Clone>(list: &[T], unwrapped: &Unwrapped, fill: &T) -> Vec<T> {
  let Some(start) = unwrapped.start else {
    return vec![fill.clone()];
  };
  let mut aligned = list.get(start..).unwrap_or_default().to_vec();
  if unwrapped.appends_dynamic {
    aligned.push(fill.clone());
  }
  aligned
}

/// A string that a command runs as shell code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunTarget {
  /// `Shell` or `Eval`.
  pub(crate) origin: CommandOrigin,
  /// The code, or `None` when it is dynamic.
  pub(crate) script: Option<String>,
  /// The shell reads its script from stdin (no `-c`, no script operand).
  pub(crate) stdin: bool,
}

const SHELL_OPTIONS: OptionSpec = OptionSpec {
  value_short: "oO",
  rest_short: "",
  value_long: "rcfile init-file",
  numeric: false,
  stop_at_operand: true,
  plus: true,
};

/// `-c` runs the first operand (bash reads `+c` the same way); `-s` or no operand reads stdin.
fn shell_target(argv: &[Word]) -> Option<RunTarget> {
  let parsed = parse_args(argv, 1, &SHELL_OPTIONS);
  let lone_dash = argv
    .get(parsed.next)
    .is_some_and(|word| word.as_deref() == Some("-"));
  let at = parsed.next + usize::from(lone_dash);
  let operand = argv.get(at);
  let target = |script: Option<String>, stdin: bool| RunTarget {
    origin: CommandOrigin::Shell,
    script,
    stdin,
  };
  if parsed.has("c") {
    return operand.map(|word| target(word.clone(), false));
  }
  if parsed.has("s") || operand.is_none() {
    return Some(target(None, true));
  }
  operand.and_then(|word| word.is_none().then(|| target(None, false)))
}

/// The shell code `argv` runs as a string (`bash -c`, a shell on stdin, `eval`), if any.
pub(crate) fn run_target(argv: &[Word]) -> Option<RunTarget> {
  let name = argv.first()?.as_deref()?;
  if matches!(basename(name), "bash" | "sh" | "zsh" | "dash" | "ksh") {
    return shell_target(argv);
  }
  if name != "eval" {
    return None;
  }
  let skip = if argv
    .get(1)
    .is_some_and(|word| word.as_deref() == Some("--"))
  {
    2
  } else {
    1
  };
  let operands = argv.get(skip..).unwrap_or_default();
  if operands.is_empty() {
    return None;
  }
  let script: Option<Vec<&str>> = operands.iter().map(Option::as_deref).collect();
  let script = script.map(|parts| parts.join(" "));
  Some(RunTarget {
    origin: CommandOrigin::Eval,
    script,
    stdin: false,
  })
}

#[cfg(test)]
#[path = "tests/argv_test.rs"]
mod tests;
