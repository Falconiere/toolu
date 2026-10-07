use std::fs;
use std::time::Duration;

use toolu_runtime::env::Env;

use crate::token::{Source, Token, TokenError, Tokens, checked, resolve, resolve_within};

fn token(value: &str) -> Token {
  checked(value, Source::Env).expect("token")
}

#[test]
fn a_value_is_trimmed_and_refused_with_inner_whitespace_or_controls() {
  assert_eq!(token("  abc\n").expose(), "abc");
  for bad in ["a b", "a\nb", "a\u{7f}b", "   "] {
    assert_eq!(
      checked(bad, Source::Gh),
      Err(TokenError::Malformed(Source::Gh))
    );
  }
}

#[test]
fn messages_name_the_sources_and_never_a_value() {
  assert_eq!(
    TokenError::Unavailable {
      gh: "exited 1".into()
    }
    .to_string(),
    "no GitHub token: GH_TOKEN is unset and `gh auth token` failed: exited 1"
  );
  assert_eq!(
    TokenError::Malformed(Source::Env).to_string(),
    "GH_TOKEN holds a malformed token"
  );
  assert_eq!(
    TokenError::Malformed(Source::Gh).to_string(),
    "`gh auth token` holds a malformed token"
  );
  assert_eq!(format!("{:?}", token("secret-value")), "Token(<redacted>)");
}

#[test]
fn gh_token_wins_and_an_empty_one_counts_as_unset() {
  let env = Env::from_pairs([("GH_TOKEN", "from-env")]);
  assert_eq!(resolve(&env).expect("token").expose(), "from-env");
  let empty_path = tempfile::tempdir().expect("dir");
  let env = Env::from_pairs([
    ("GH_TOKEN", ""),
    ("PATH", empty_path.path().to_str().expect("utf-8")),
  ]);
  assert_eq!(
    resolve(&env),
    Err(TokenError::Unavailable {
      gh: "gh: No such file or directory (os error 2)".into()
    })
  );
}

#[test]
fn a_gh_that_exits_without_stderr_or_stdout_is_named() {
  let dir = tempfile::tempdir().expect("dir");
  let silent = dir.path().join("gh");
  fs::write(&silent, "#!/bin/sh\nexit 3\n").expect("script");
  set_executable(&silent);
  let path = dir.path().to_str().expect("utf-8");
  assert_eq!(
    resolve(&Env::from_pairs([("PATH", path)])),
    Err(TokenError::Unavailable {
      gh: "exited 3".into()
    })
  );
  fs::write(&silent, "#!/bin/sh\necho\n").expect("script");
  assert_eq!(
    resolve(&Env::from_pairs([("PATH", path)])),
    Err(TokenError::Unavailable {
      gh: "printed no token".into()
    })
  );
}

fn set_executable(path: &std::path::Path) {
  use std::os::unix::fs::PermissionsExt as _;
  fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

#[test]
fn a_refresh_retries_with_any_token_other_than_the_one_refused() {
  let mut tokens = Tokens::new(token("old-token"));
  assert!(!tokens.refresh(&token("old-token"), token("old-token")));
  assert!(tokens.refresh(&token("old-token"), token("new-token")));
  assert_eq!(tokens.current().expose(), "new-token");
  // Another thread refused with the old token sees the new one as worth a retry.
  assert!(tokens.refresh(&token("old-token"), token("new-token")));
  assert!(!tokens.refresh(&token("new-token"), token("new-token")));
  assert_eq!(
    tokens.redact("old-token then new-token"),
    "<redacted> then <redacted>"
  );
}

#[test]
fn a_gh_past_its_deadline_is_named() {
  let dir = tempfile::tempdir().expect("dir");
  let slow = dir.path().join("gh");
  fs::write(&slow, "#!/bin/sh\nexec /bin/sleep 5\n").expect("script");
  set_executable(&slow);
  let env = Env::from_pairs([("PATH", dir.path().to_str().expect("utf-8"))]);
  assert_eq!(
    resolve_within(&env, Duration::from_millis(200)),
    Err(TokenError::Unavailable {
      gh: "timed out after 200ms".into()
    })
  );
}
