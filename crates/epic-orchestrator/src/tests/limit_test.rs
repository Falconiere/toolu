use super::limit_line as scan_line;

#[test]
fn limit_line() {
  assert!(scan_line("All 34 tests passed").is_none());
  assert_eq!(
    scan_line("ok\nusage limit reached\n").as_deref(),
    Some("usage limit reached")
  );
  assert_eq!(
    scan_line("usage limit first\nquota exceeded\n").as_deref(),
    Some("quota exceeded")
  );
  assert!(scan_line("You've hit your usage limit. Upgrade or try again at 5:00 PM.").is_some());
  assert!(scan_line("Error: 429 Too Many Requests").is_some());
  assert!(scan_line("stream error: rate limited, retrying").is_some());
  let long = format!("{} usage limit", "x".repeat(210));
  let got = scan_line(&long).expect("long");
  assert_eq!(got.chars().count(), 200);
}
