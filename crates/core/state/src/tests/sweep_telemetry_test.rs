use super::kept_telemetry_lines;

const CUTOFF: &str = "2026-01-08T00:00:00Z";

#[test]
fn lines_at_or_after_the_cutoff_are_kept_in_compact_jq_text() {
  let content = concat!(
    "{\"t\":\"2026-01-01T00:00:00Z\",\"event\":\"old\"}\n",
    "{ \"t\" : \"2026-01-08T00:00:00Z\", \"x\": \"\u{7f}\" }\n",
    "\n",
    "   \n",
    "null\n",
    "{\"t\":{\"nested\":1}}\n",
    "{\"t\":[1]}\n",
    "{\"t\":5}\n",
    "{\"t\":true}\n",
    "{\"t\":null}\n",
    "{\"no\":\"t\"}\n",
    "{\"t\":\"2026-02-01T00:00:00Z\"}",
  );
  let kept = kept_telemetry_lines(content, CUTOFF).unwrap();
  assert_eq!(
    kept,
    [
      "{\"t\":\"2026-01-08T00:00:00Z\",\"x\":\"\\u007f\"}",
      "{\"t\":{\"nested\":1}}",
      "{\"t\":[1]}",
      "{\"t\":\"2026-02-01T00:00:00Z\"}",
    ]
  );
}

#[test]
fn a_line_jq_would_fail_on_leaves_the_file_alone() {
  for bad in ["{oops}\n", "[1]\n", "\"text\"\n", "7\n", "true\n"] {
    assert_eq!(kept_telemetry_lines(bad, CUTOFF), None, "{bad:?}");
  }
  assert_eq!(kept_telemetry_lines("", CUTOFF), Some(Vec::new()));
}
