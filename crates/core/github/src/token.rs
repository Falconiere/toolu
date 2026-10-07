//! The GitHub token: `GH_TOKEN`, else `gh auth token`. It is held in memory,
//! never written or printed, and every token read is redacted from messages.

use std::fmt;
use std::time::Duration;

use toolu_runtime::env::Env;
use toolu_runtime::process::{self, RunError, Spec};

/// How long `gh auth token` may run.
const GH_DEADLINE: Duration = Duration::from_secs(10);

/// What replaces a token in a message.
const REDACTED: &str = "<redacted>";

/// Where a token came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
  /// The `GH_TOKEN` variable.
  Env,
  /// `gh auth token`.
  Gh,
}

impl fmt::Display for Source {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Self::Env => "GH_TOKEN",
      Self::Gh => "`gh auth token`",
    })
  }
}

/// Why no token could be read. The messages never hold a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
  /// `GH_TOKEN` is unset and `gh auth token` failed for `gh`.
  Unavailable {
    /// Why `gh auth token` failed.
    gh: String,
  },
  /// The source held whitespace or a control character inside its value.
  Malformed(Source),
}

impl fmt::Display for TokenError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Unavailable { gh } => write!(
        f,
        "no GitHub token: GH_TOKEN is unset and `gh auth token` failed: {gh}"
      ),
      Self::Malformed(source) => write!(f, "{source} holds a malformed token"),
    }
  }
}

impl std::error::Error for TokenError {}

/// A token. Its `Debug` hides the value.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Token(String);

impl Token {
  /// The value, for the `Authorization` header only.
  pub(crate) fn expose(&self) -> &str {
    &self.0
  }
}

impl fmt::Debug for Token {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("Token(<redacted>)")
  }
}

/// Read the token from `env`: `GH_TOKEN`, else `gh auth token` run with `env`
/// as its whole environment.
pub(crate) fn resolve(env: &Env) -> Result<Token, TokenError> {
  resolve_within(env, GH_DEADLINE)
}

/// [`resolve`], with `gh` given `deadline`.
pub(crate) fn resolve_within(env: &Env, deadline: Duration) -> Result<Token, TokenError> {
  if let Some(value) = env.get("GH_TOKEN") {
    return checked(value, Source::Env);
  }
  let spec = Spec {
    env: Some(env.clone()),
    timeout: deadline,
    max_output_bytes: 64 * 1024,
    ..Spec::new(["gh", "auth", "token"])
  };
  let unavailable = |gh: String| TokenError::Unavailable { gh };
  let output = process::run(&spec).map_err(|err| unavailable(run_error(&err)))?;
  if output.timed_out {
    return Err(unavailable(format!("timed out after {deadline:?}")));
  }
  if output.exit_code != 0 {
    let line = output
      .stderr
      .lines()
      .rev()
      .find(|line| !line.trim().is_empty());
    let reason = line.map_or_else(|| format!("exited {}", output.exit_code), str::to_owned);
    return Err(unavailable(reason));
  }
  if output.stdout.trim().is_empty() {
    return Err(unavailable("printed no token".to_owned()));
  }
  checked(&output.stdout, Source::Gh)
}

/// `value` without surrounding whitespace, refused when anything inside it is
/// whitespace or a control character.
pub(crate) fn checked(value: &str, source: Source) -> Result<Token, TokenError> {
  let value = value.trim();
  if value.is_empty()
    || value
      .chars()
      .any(|ch| ch.is_whitespace() || ch.is_control())
  {
    return Err(TokenError::Malformed(source));
  }
  Ok(Token(value.to_owned()))
}

fn run_error(err: &RunError) -> String {
  match err {
    RunError::Spawn(reason)
    | RunError::Wait(reason)
    | RunError::Stdin(reason)
    | RunError::Callback(reason) => reason.clone(),
    RunError::EmptyArgv | RunError::ZeroTimeout | RunError::TimeoutTooLong => format!("{err:?}"),
  }
}

/// The current token and every token read before it, for redaction.
#[derive(Debug)]
pub(crate) struct Tokens {
  current: Token,
  seen: Vec<Token>,
}

impl Tokens {
  /// Start with `token`.
  pub(crate) fn new(token: Token) -> Tokens {
    Tokens {
      seen: vec![token.clone()],
      current: token,
    }
  }

  /// The token requests carry now.
  pub(crate) fn current(&self) -> Token {
    self.current.clone()
  }

  /// After a `401` to `sent`, make `fresh` current: whether it differs from
  /// `sent`, so a retry can succeed. Another thread may have made `fresh`
  /// current already; the retry is still worth it.
  pub(crate) fn refresh(&mut self, sent: &Token, fresh: Token) -> bool {
    if fresh == *sent {
      return false;
    }
    if !self.seen.contains(&fresh) {
      self.seen.push(fresh.clone());
    }
    self.current = fresh;
    true
  }

  /// `text` with every token read so far replaced by `<redacted>`.
  pub(crate) fn redact(&self, text: &str) -> String {
    let mut seen: Vec<&str> = self.seen.iter().map(Token::expose).collect();
    seen.sort_by_key(|token| std::cmp::Reverse(token.len()));
    seen
      .into_iter()
      .fold(text.to_owned(), |text, token| text.replace(token, REDACTED))
  }
}

#[cfg(test)]
#[path = "tests/token_test.rs"]
mod tests;
