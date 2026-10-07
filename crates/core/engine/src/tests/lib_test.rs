use crate::LinkError;

#[test]
fn link_errors_name_the_porting_issue_or_the_failure() {
  assert_eq!(
    LinkError::NotPorted { issue: 433 }.to_string(),
    "not ported yet (#433)"
  );
  assert_eq!(LinkError::Failed("boom".into()).to_string(), "boom");
}
