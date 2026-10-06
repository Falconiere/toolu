//! What the shell lexer yields: commands as words with quotes kept, and the
//! rule that turns a run of words into the command the shell would run.

/// One command: its first line in the Markdown file and its words, quotes kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Command {
  /// The 1-based Markdown line of the command's first word.
  pub(crate) line: usize,
  /// The words, from the command name on.
  pub(crate) words: Vec<String>,
}

/// What a block runs and the functions it defines.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Lexed {
  /// Every command, in order.
  pub(crate) commands: Vec<Command>,
  /// Names of `name() …` and `function name` definitions.
  pub(crate) functions: Vec<String>,
}

/// Keywords that precede a command and are dropped.
const LEADING: &[&str] = &[
  "if", "then", "elif", "else", "while", "until", "do", "!", "time",
];

/// Keywords that end a construct, or headers that run nothing.
const SILENT: &[&str] = &[
  "fi", "done", "esac", "}", "{", "for", "select", "case", "function",
];

/// The word without its quote characters.
pub(crate) fn unquote(word: &str) -> String {
  word.chars().filter(|c| *c != '"' && *c != '\'').collect()
}

/// The command `words` run, after dropping leading assignments and keywords;
/// closing keywords, `for`, `select` and `case` headers and `function name`
/// run nothing.
pub(crate) fn command(words: Vec<(String, usize)>) -> Option<Command> {
  let mut words = words.into_iter().peekable();
  while words
    .peek()
    .is_some_and(|(word, _)| LEADING.contains(&word.as_str()) || is_assignment(word))
  {
    words.next();
  }
  let (name, line) = words.next()?;
  if SILENT.contains(&name.as_str()) {
    return None;
  }
  let mut all = vec![name];
  all.extend(words.map(|(word, _)| word));
  Some(Command { line, words: all })
}

fn is_assignment(word: &str) -> bool {
  word.split_once('=').is_some_and(|(name, _)| {
    // `NAME+=value` appends.
    let mut chars = name.strip_suffix('+').unwrap_or(name).chars();
    chars
      .next()
      .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
      && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
  })
}

/// `<…>`, `[…]` and anything with a `$` stand for a value.
pub(crate) fn is_placeholder(word: &str) -> bool {
  (word.contains('<') && word.contains('>'))
    || (word.starts_with('[') && word.ends_with(']'))
    || word.contains('$')
}

/// `…` or `...`: the rest of the command is elided.
pub(crate) fn is_ellipsis(word: &str) -> bool {
  word == "…" || word == "..."
}

/// Levenshtein distance.
pub(crate) fn distance(a: &str, b: &str) -> usize {
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

#[cfg(test)]
#[path = "tests/words_test.rs"]
mod tests;
