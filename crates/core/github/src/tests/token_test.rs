use std::fs;

use toolu_runtime::env::Env;

use crate::token::{Source, Token, TokenError, Tokens, checked, resolve};

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
  let Err(TokenError::Unavailable { gh }) = resolve(&env) else {
    panic!("expected Unavailable");
  };
  assert!(gh.starts_with("gh: "), "{gh}");
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
fn tokens_track_the_current_one_and_redact_every_one_read() {
  let mut tokens = Tokens::new(token("old-token"));
  assert!(!tokens.replace(token("old-token")));
  assert!(tokens.replace(token("new-token")));
  assert_eq!(tokens.current().expose(), "new-token");
  assert_eq!(
    tokens.redact("old-token then new-token"),
    "<redacted> then <redacted>"
  );
}
