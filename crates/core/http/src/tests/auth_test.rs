use crate::Auth;

#[test]
fn basic_auth_keeps_empty_password() {
  let value = Auth::Basic {
    username: "alice".into(),
    password: String::new(),
  }
  .header_value();
  assert_eq!(value.as_deref(), Some("Basic YWxpY2U6"));
}

#[test]
fn bearer_and_none_header_values() {
  assert_eq!(
    Auth::Bearer("t-1".into()).header_value().as_deref(),
    Some("Bearer t-1")
  );
  assert_eq!(Auth::None.header_value(), None);
}

#[test]
fn debug_never_prints_a_secret() {
  let basic = Auth::Basic {
    username: "alice".into(),
    password: "secret-pass".into(),
  };
  assert_eq!(
    format!("{basic:?}"),
    "Basic { username: \"alice\", password: <redacted> }"
  );
  assert_eq!(
    format!("{:?}", Auth::Bearer("secret-bearer".into())),
    "Bearer(<redacted>)"
  );
  assert_eq!(format!("{:?}", Auth::None), "None");
}
