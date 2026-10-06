//! getopt-style argument splitting over resolved words (`shell-options.ts`),
//! shared by the wrapper, git and write-target readers. A short cluster (`-am`)
//! is read letter by letter: a value letter takes the rest of the word or the
//! next word, and a "rest" letter (`sed -i.bak`) takes whatever follows it in
//! the same word. Long options take `--name=value`, or the next word when
//! listed as value-taking.

use crate::analysis::Word;

/// Which options a command takes. Names are space-separated.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct OptionSpec {
  /// Short letters that take a value: attached (`-oL`) or the next word.
  pub(crate) value_short: &'static str,
  /// Short letters whose value is only the rest of the cluster, possibly empty (`-i.bak`).
  pub(crate) rest_short: &'static str,
  /// Long names (without `--`) that take the next word when written without `=`.
  pub(crate) value_long: &'static str,
  /// `-<digits>` is one option (`nice -10`).
  pub(crate) numeric: bool,
  /// Stop at the first operand: the rest is a command (wrappers, git globals).
  pub(crate) stop_at_operand: bool,
  /// `+x` is an option cluster too, as a shell reads it (`bash +o posix`).
  pub(crate) plus: bool,
}

/// An option's value: TypeScript's `undefined`, `null` or a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Value<'w> {
  /// The option takes no value, or the value word is missing.
  Absent,
  /// The value word is dynamic.
  Dynamic,
  /// A static value.
  Text(&'w str),
}

impl<'w> Value<'w> {
  /// `None` for a dynamic or absent value.
  pub(crate) fn text(self) -> Option<&'w str> {
    match self {
      Value::Text(text) => Some(text),
      Value::Absent | Value::Dynamic => None,
    }
  }
}

/// One option as read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParsedOption<'w> {
  /// A short letter (`m`) or a long name (`message`).
  pub(crate) name: &'w str,
  /// Its value.
  pub(crate) value: Value<'w>,
  /// Index of the word the value was read from, when it was a separate word.
  pub(crate) at: Option<usize>,
}

/// The options and operands of `words[start..]`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ParsedArgs<'w> {
  /// Every option, in order.
  pub(crate) options: Vec<ParsedOption<'w>>,
  /// Index of each operand in the parsed words.
  pub(crate) operand_at: Vec<usize>,
  /// Index just past what was consumed; with `stop_at_operand`, where the command starts.
  pub(crate) next: usize,
  /// A value-taking option was the last word (`git -C`): the command line is malformed.
  pub(crate) missing_value: bool,
}

impl<'w> ParsedArgs<'w> {
  /// Whether any option among the space-separated `names` was given.
  pub(crate) fn has(&self, names: &str) -> bool {
    self.options.iter().any(|option| named(names, option.name))
  }

  /// Values of every option among the space-separated `names`, in order.
  pub(crate) fn values(&self, names: &str) -> Vec<Value<'w>> {
    self
      .options
      .iter()
      .filter(|option| named(names, option.name))
      .map(|option| option.value)
      .collect()
  }
}

/// Whether `name` is one of the space-separated `names`.
pub(crate) fn named(names: &str, name: &str) -> bool {
  names.split(' ').any(|candidate| candidate == name)
}

/// The value of `words[at]`: absent past the end, dynamic for a `None` word.
pub(crate) fn value_at(words: &[Word], at: usize) -> Value<'_> {
  match words.get(at) {
    None => Value::Absent,
    Some(None) => Value::Dynamic,
    Some(Some(text)) => Value::Text(text),
  }
}

/// Whether a static word is an option under `spec`.
fn is_option(word: &str, spec: &OptionSpec) -> bool {
  if word == "-" || word == "--" {
    return false;
  }
  if spec.plus {
    let mut chars = word.chars();
    matches!(chars.next(), Some('-' | '+')) && chars.next().is_some()
  } else {
    word.starts_with('-')
  }
}

/// The reader's state while walking the words.
struct Reader<'w, 's> {
  words: &'w [Word],
  spec: &'s OptionSpec,
  parsed: ParsedArgs<'w>,
  at: usize,
}

impl<'w> Reader<'w, '_> {
  /// The option's value is the next word.
  fn take_next(&mut self, name: &'w str) {
    self.at += 1;
    self.parsed.missing_value |= self.at >= self.words.len();
    let value = value_at(self.words, self.at);
    let at = Some(self.at);
    self.parsed.options.push(ParsedOption { name, value, at });
  }

  fn push(&mut self, name: &'w str, value: Value<'w>) {
    let option = ParsedOption {
      name,
      value,
      at: None,
    };
    self.parsed.options.push(option);
  }

  fn long(&mut self, body: &'w str) {
    match body.split_once('=') {
      Some((name, value)) => self.push(name, Value::Text(value)),
      None if named(self.spec.value_long, body) => self.take_next(body),
      None => self.push(body, Value::Absent),
    }
  }

  fn cluster(&mut self, word: &'w str) {
    for (index, letter) in word.char_indices().skip(1) {
      let end = index + letter.len_utf8();
      let (Some(name), Some(rest)) = (word.get(index..end), word.get(end..)) else {
        return;
      };
      let valued = self.spec.value_short.contains(letter);
      if self.spec.rest_short.contains(letter) || (valued && !rest.is_empty()) {
        self.push(name, Value::Text(rest));
        return;
      }
      if valued {
        self.take_next(name);
        return;
      }
      self.push(name, Value::Absent);
    }
  }

  fn option(&mut self, word: &'w str) {
    if let Some(body) = word.strip_prefix("--") {
      self.long(body);
    } else if self.spec.numeric && is_numeric(word) {
      self.push(word.get(1..).unwrap_or_default(), Value::Absent);
    } else {
      self.cluster(word);
    }
  }
}

/// `-<digits>`.
fn is_numeric(word: &str) -> bool {
  word
    .strip_prefix('-')
    .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Split `words[start..]` into options and operands under `spec`.
pub(crate) fn parse_args<'w>(words: &'w [Word], start: usize, spec: &OptionSpec) -> ParsedArgs<'w> {
  let mut reader = Reader {
    words,
    spec,
    parsed: ParsedArgs::default(),
    at: start,
  };
  while let Some(word) = words.get(reader.at) {
    match word.as_deref() {
      Some(text) if is_option(text, spec) => reader.option(text),
      operand => {
        if spec.stop_at_operand {
          let next = reader.at + usize::from(operand == Some("--"));
          return ParsedArgs {
            next,
            ..reader.parsed
          };
        }
        if operand == Some("--") {
          reader.parsed.operand_at.extend(reader.at + 1..words.len());
          break;
        }
        reader.parsed.operand_at.push(reader.at);
      }
    }
    reader.at += 1;
  }
  ParsedArgs {
    next: words.len(),
    ..reader.parsed
  }
}

#[cfg(test)]
#[path = "tests/options_test.rs"]
mod tests;
