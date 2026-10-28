use super::{redact_line, scan};
use crate::contract::{Finding, Severity, Side, Source};
use crate::prepass::tests::{added, shaped, token};
use crate::shape::{ChangedLine, Shaped};

fn secrets_of(shaped: &Shaped) -> Vec<Finding> {
  let (findings, _, notices) = scan(shaped);
  assert_eq!(notices, Vec::new());
  findings
}

#[test]
fn prepass_secrets_github_token_is_masked_with_spans_into_the_unmasked_line() {
  let secret = token();
  let text = format!("let t = \"{secret}\";");
  let found = secrets_of(&shaped("src/a.rs", vec![added(7, &text)]));
  assert_eq!(found.len(), 1);
  let one = &found[0];
  assert_eq!(
    (one.source, one.severity, one.category.as_str()),
    (Source::Secrets, Severity::Blocker, "secret-github-token")
  );
  assert_eq!(
    (one.path.as_str(), one.line, one.end_line),
    ("src/a.rs", 7, 7)
  );
  assert_eq!(one.id, "prepass:secrets:src/a.rs:7:secret-github-token");
  assert_eq!(one.claim, "possible committed secret (github-token)");
  assert_eq!(one.quote, "let t = \"***\";");
  let span = one.redaction_spans[0];
  assert!(span.end > span.start);
  assert_eq!(text.get(span.start..span.end), Some(secret.as_str()));
}

#[test]
fn prepass_secrets_spans_are_utf8_byte_offsets() {
  let secret = token();
  let text = format!("é = \"{secret}\"");
  let found = secrets_of(&shaped("a.rs", vec![added(1, &text)]));
  let span = found[0].redaction_spans[0];
  assert_eq!(span.start, "é = \"".len());
  assert_eq!(text.get(span.start..span.end), Some(secret.as_str()));
  assert_eq!(found[0].quote, "é = \"***\"");
}

#[test]
fn prepass_secrets_high_entropy_token_is_a_medium_finding() {
  let text = "let k = \"Zx9Qk2Lm7Pv4Rt8Wn3Yc6Bd1Fh5Gj0Sa\";";
  let found = secrets_of(&shaped("a.rs", vec![added(2, text)]));
  assert_eq!(found.len(), 1);
  assert_eq!(
    (
      found[0].source,
      found[0].severity,
      found[0].category.as_str()
    ),
    (Source::Secrets, Severity::Medium, "secret-entropy")
  );
  assert_eq!(found[0].claim, "high-entropy token looks like a secret");
  assert_eq!(found[0].quote, "let k = \"***\";");
}

#[test]
fn prepass_secrets_low_entropy_long_tokens_and_short_tokens_are_ignored() {
  let lines = vec![
    added(1, "let a = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\";"),
    added(2, "let b = \"Zx9Qk2Lm7Pv4Rt8\";"),
    added(3, "fn a_perfectly_ordinary_long_function_name() {}"),
  ];
  assert_eq!(secrets_of(&shaped("a.rs", lines)), Vec::new());
}

#[test]
fn prepass_secrets_a_known_pattern_sets_the_category_and_old_lines_are_not_scanned() {
  let both = format!("\"{}\" \"Zx9Qk2Lm7Pv4Rt8Wn3Yc6Bd1Fh5Gj0Sa\"", token());
  let old = ChangedLine {
    side: Side::Old,
    line: 4,
    text: token(),
  };
  let found = secrets_of(&shaped("a.rs", vec![added(1, &both), old]));
  assert_eq!(found.len(), 1);
  assert_eq!(found[0].category, "secret-github-token");
  assert_eq!(found[0].severity, Severity::Blocker);
  assert_eq!(found[0].redaction_spans.len(), 2);
  assert_eq!(found[0].quote, "\"***\" \"***\"");
}

#[test]
fn prepass_secrets_two_known_secrets_on_one_line_are_both_masked() {
  let github = token();
  let aws = format!("AKIA{}", "ABCDEFGHIJKLMNOP");
  let text = format!("\"{github}\" \"{aws}\"");
  let found = secrets_of(&shaped("a.rs", vec![added(3, &text)]));
  assert_eq!(found.len(), 1);
  let one = &found[0];
  assert_eq!(one.category, "secret-aws-access-key");
  assert_eq!(one.claim, "possible committed secret (aws-access-key)");
  assert_eq!(one.severity, Severity::Blocker);
  assert_eq!(one.quote, "\"***\" \"***\"");
  let masked: Vec<&str> = one
    .redaction_spans
    .iter()
    .filter_map(|span| text.get(span.start..span.end))
    .collect();
  assert_eq!(masked, [github.as_str(), aws.as_str()]);
}

#[test]
fn prepass_secrets_a_known_secret_and_a_high_entropy_token_are_both_masked() {
  let aws = format!("AKIA{}", "ABCDEFGHIJKLMNOP");
  let entropic = "Zx9Qk2Lm7Pv4Rt8Wn3Yc6Bd1Fh5Gj0SaQw2Er4Ty6U";
  assert_eq!(entropic.len(), 42);
  let text = format!("\"{aws}\" \"{entropic}\"");
  let found = secrets_of(&shaped("a.rs", vec![added(5, &text)]));
  assert_eq!(found.len(), 1);
  let one = &found[0];
  assert_eq!(one.category, "secret-aws-access-key");
  assert_eq!(one.severity, Severity::Blocker);
  assert_eq!(one.quote, "\"***\" \"***\"");
  assert!(!one.quote.contains(entropic));
  let masked: Vec<&str> = one
    .redaction_spans
    .iter()
    .filter_map(|span| text.get(span.start..span.end))
    .collect();
  assert_eq!(masked, [aws.as_str(), entropic]);
}

#[test]
fn prepass_secrets_redact_line_masks_known_and_entropy_tokens_with_spans() {
  let secret = token();
  let text = format!("todo!(); // {secret} Zx9Qk2Lm7Pv4Rt8Wn3Yc6Bd1Fh5Gj0Sa");
  let (quote, spans) = redact_line(&text);
  assert_eq!(quote, "todo!(); // *** ***");
  let masked: Vec<&str> = spans
    .iter()
    .filter_map(|span| text.get(span.start..span.end))
    .collect();
  assert_eq!(
    masked,
    [secret.as_str(), "Zx9Qk2Lm7Pv4Rt8Wn3Yc6Bd1Fh5Gj0Sa"]
  );
}

#[test]
fn prepass_secrets_redact_line_leaves_a_clean_line_alone() {
  let (quote, spans) = redact_line("    todo!()");
  assert_eq!(quote, "    todo!()");
  assert_eq!(spans, Vec::new());
}
