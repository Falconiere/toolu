use super::Error;

#[test]
fn typed_errors_keep_safe_diagnostic_fields() {
  let truncated = Error::Truncated {
    finish_reason: Some("length".into()),
    usage: None,
  };
  assert!(truncated.to_string().contains("length"));
  let rate = Error::RateLimited {
    retry_after_secs: Some(5),
  };
  assert!(rate.to_string().contains('5'));
  assert!(!Error::Authentication(401).to_string().contains("secret"));
}
