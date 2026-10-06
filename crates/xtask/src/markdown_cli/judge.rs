//! Judge one `toolu …` command against the command tree of
//! `docs/cli/commands.json`: commands by name or alias, flags of the command
//! and every global flag above it, flag values, and the positional count.

use serde_json::Value;

use crate::command_tree::{flag_set, list, text};

/// Where the command was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
  /// A fenced shell block: it must be a complete command.
  Fenced,
  /// An inline code span: prose may name a namespace or omit arguments.
  Inline,
}

/// Flags clap adds to every command; the tree does not list them.
const HELP: &str = "help";

/// Flags that end the run before a command is needed.
const TERMINAL: &[&str] = &["help", "version", "hook-protocol"];

/// The problem with `words` (unquoted, `words[0]` is `toolu`), if any.
pub(crate) fn judge(tree: &Value, words: &[String], origin: Origin) -> Option<String> {
  let rest = words.get(1..).unwrap_or_default();
  let state = State {
    node: tree,
    path: "toolu".to_owned(),
    globals: Vec::new(),
    args: 0,
    done: false,
  };
  walk(state, rest, origin).err()
}

/// Where the walk stands.
#[derive(Clone)]
struct State<'a> {
  node: &'a Value,
  path: String,
  /// Global flags of the ancestors.
  globals: Vec<&'a Value>,
  /// Positionals consumed by `node`.
  args: usize,
  /// A help, version or protocol flag was given.
  done: bool,
}

fn walk(mut state: State<'_>, words: &[String], origin: Origin) -> Result<(), String> {
  let mut index = 0;
  while let Some(word) = words.get(index) {
    index += 1;
    let rest = words.get(index..).unwrap_or_default();
    if is_ellipsis(word) {
      return Ok(());
    }
    if word == "--" {
      state.args += rest.len();
      return arity(&state, origin);
    }
    if word.starts_with('-') && word.len() > 1 && !is_placeholder(word) {
      index += flag(&mut state, word, rest, origin)?;
      continue;
    }
    if has_children(state.node) {
      return descend(&state, word, rest, origin);
    }
    state.args += 1;
    arity(&state, Origin::Inline)?;
  }
  arity(&state, origin)
}

/// Enter the child `word` names; a placeholder tries every child.
fn descend(state: &State<'_>, word: &str, rest: &[String], origin: Origin) -> Result<(), String> {
  if let Some(child) = child(state.node, word) {
    return walk(enter(state, child, word), rest, origin);
  }
  if !is_placeholder(word) {
    return Err(unknown_command(state, word));
  }
  let optional = word.starts_with('[') && walk(state.clone(), rest, origin).is_ok();
  let fits = list(state.node, "commands")
    .iter()
    .any(|child| walk(enter(state, child, text(child, "name")), rest, origin).is_ok());
  if optional || fits {
    return Ok(());
  }
  let tried = std::iter::once(word.to_owned())
    .chain(rest.iter().cloned())
    .collect::<Vec<_>>();
  Err(format!(
    "no command under `{}` fits `{}`",
    state.path,
    tried.join(" ")
  ))
}

fn enter<'a>(state: &State<'a>, child: &'a Value, name: &str) -> State<'a> {
  let mut globals = state.globals.clone();
  globals.extend(
    list(state.node, "flags")
      .iter()
      .filter(|flag| flag_set(flag, "global")),
  );
  State {
    node: child,
    path: format!("{} {name}", state.path),
    globals,
    args: 0,
    done: state.done,
  }
}

/// Consume one flag word (and its value); returns the extra words used.
fn flag(
  state: &mut State<'_>,
  word: &str,
  rest: &[String],
  origin: Origin,
) -> Result<usize, String> {
  let (name, inline_value) = match word.split_once('=') {
    Some((name, value)) => (name, Some(value.to_owned())),
    None => (word, None),
  };
  let found = match name.strip_prefix("--") {
    Some(long) => find(state, |flag| text(flag, "long") == long, long),
    None => short(state, name),
  };
  let Some(found) = found else {
    return Err(format!(
      "unknown flag `{name}` on `{}` (valid: {})",
      state.path,
      valid_flags(state)
    ));
  };
  let Found::Flag(flag) = found else {
    state.done = true;
    return Ok(0);
  };
  let long = text(flag, "long");
  state.done |= TERMINAL.contains(&long);
  if !flag_set(flag, "takesValue") {
    return Ok(0);
  }
  let (value, used) = match inline_value {
    Some(value) => (Some(value), 0),
    None => (rest.first().cloned(), 1),
  };
  match value {
    Some(value) => check_value(state, flag, &value).map(|()| used),
    None if origin == Origin::Fenced => {
      Err(format!("`--{long}` on `{}` needs a value", state.path))
    }
    None => Ok(0),
  }
}

/// A flag a command accepts.
#[derive(Clone, Copy)]
enum Found<'a> {
  /// clap's own `--help`/`-h`.
  Help,
  /// A flag of the tree.
  Flag(&'a Value),
}

/// A short flag (`-q`); clusters are judged by their last letter's value need.
fn short<'a>(state: &State<'a>, name: &str) -> Option<Found<'a>> {
  let letters = name.strip_prefix('-')?;
  let mut found = None;
  for letter in letters.chars() {
    let shown = letter.to_string();
    found = Some(find(state, |flag| text(flag, "short") == shown, &shown)?);
  }
  found
}

/// The flag `matches` picks among the node's flags and inherited globals.
fn find<'a>(state: &State<'a>, matches: impl Fn(&Value) -> bool, name: &str) -> Option<Found<'a>> {
  if name == HELP || name == "h" {
    return Some(Found::Help);
  }
  flags(state)
    .into_iter()
    .find(|flag| matches(flag))
    .map(Found::Flag)
}

fn flags<'a>(state: &State<'a>) -> Vec<&'a Value> {
  let mut all: Vec<&Value> = list(state.node, "flags").iter().collect();
  all.extend(state.globals.iter().copied());
  all
}

fn check_value(state: &State<'_>, flag: &Value, value: &str) -> Result<(), String> {
  let allowed = list(flag, "possibleValues");
  if allowed.is_empty()
    || is_placeholder(value)
    || allowed.iter().any(|v| v.as_str() == Some(value))
  {
    return Ok(());
  }
  let shown: Vec<&str> = allowed.iter().filter_map(Value::as_str).collect();
  Err(format!(
    "`--{}` on `{}` does not accept `{value}` (valid: {})",
    text(flag, "long"),
    state.path,
    shown.join(", ")
  ))
}

/// Too many positionals always fail; a fenced command must also be complete.
fn arity(state: &State<'_>, origin: Origin) -> Result<(), String> {
  let args = list(state.node, "args");
  let multiple = args.last().is_some_and(|arg| flag_set(arg, "multiple"));
  if state.args > args.len() && !multiple {
    return Err(format!(
      "too many arguments for `{}` (takes {})",
      state.path,
      args.len()
    ));
  }
  if origin == Origin::Inline || state.done {
    return Ok(());
  }
  if has_children(state.node) {
    return Err(format!(
      "`{}` needs a command (valid: {})",
      state.path,
      visible(state.node).join(", ")
    ));
  }
  match args
    .iter()
    .filter(|arg| flag_set(arg, "required"))
    .nth(state.args)
  {
    Some(missing) => Err(format!(
      "`{}` needs its {} argument",
      state.path,
      text(missing, "name")
    )),
    None => Ok(()),
  }
}

fn unknown_command(state: &State<'_>, word: &str) -> String {
  let names = visible(state.node);
  let closest = names
    .iter()
    .fold(None::<(usize, &str)>, |best, name| {
      let score = distance(word, name);
      match best {
        Some((low, _)) if low <= score => best,
        _ => Some((score, name)),
      }
    })
    .map_or_else(String::new, |(_, name)| format!("closest: `{name}`; "));
  format!(
    "unknown command `{word}` under `{}` ({closest}valid: {})",
    state.path,
    names.join(", ")
  )
}

fn valid_flags(state: &State<'_>) -> String {
  let mut shown: Vec<String> = flags(state)
    .into_iter()
    .filter(|flag| !flag_set(flag, "hidden"))
    .map(|flag| match flag.get("short").and_then(Value::as_str) {
      Some(short) => format!("--{}/-{short}", text(flag, "long")),
      None => format!("--{}", text(flag, "long")),
    })
    .collect();
  shown.push("--help/-h".to_owned());
  shown.join(", ")
}

/// Levenshtein distance.
fn distance(a: &str, b: &str) -> usize {
  let b: Vec<char> = b.chars().collect();
  let mut previous: Vec<usize> = (0..=b.len()).collect();
  for (i, left) in a.chars().enumerate() {
    let mut current = vec![i + 1];
    for (j, right) in b.iter().enumerate() {
      let substitute = previous
        .get(j)
        .map_or(0, |d| d + usize::from(left != *right));
      let delete = previous.get(j + 1).map_or(0, |d| d + 1);
      let insert = current.get(j).map_or(0, |d| d + 1);
      current.push(substitute.min(delete).min(insert));
    }
    previous = current;
  }
  previous.last().copied().unwrap_or_default()
}

fn child<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
  list(node, "commands").iter().find(|command| {
    text(command, "name") == name
      || list(command, "aliases")
        .iter()
        .any(|alias| alias.as_str() == Some(name))
  })
}

fn visible(node: &Value) -> Vec<&str> {
  list(node, "commands")
    .iter()
    .filter(|command| !flag_set(command, "hidden"))
    .map(|command| text(command, "name"))
    .collect()
}

fn has_children(node: &Value) -> bool {
  !list(node, "commands").is_empty()
}

/// `<…>`, `[…]` and anything with a `$` stand for a value.
pub(crate) fn is_placeholder(word: &str) -> bool {
  (word.contains('<') && word.contains('>'))
    || (word.starts_with('[') && word.ends_with(']'))
    || word.contains('$')
}

fn is_ellipsis(word: &str) -> bool {
  word == "…" || word == "..."
}

#[cfg(test)]
#[path = "tests/judge_test.rs"]
mod tests;
