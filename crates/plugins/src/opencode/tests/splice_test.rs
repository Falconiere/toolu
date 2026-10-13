//! The JSONC fixtures of `tools/toolu-cli/src/opencode/__tests__/config.test.ts`,
//! byte for byte.

use super::{append, remove};

const COMMENTED: &str = "// user settings
{
  /* keep me */
  \"plugin\": [
    \"file:///opt/probe.ts\", // unrelated
  ],
  \"share\": \"disabled\",
}
";

#[test]
fn append_after_a_trailing_comma_and_comment_keeps_every_other_byte() {
  let added = append(COMMENTED, "plugin", "@toolu/opencode@1.0.0");
  assert_eq!(
    added,
    COMMENTED.replace(
      "// unrelated\n",
      "// unrelated\n    \"@toolu/opencode@1.0.0\",\n"
    )
  );
  assert_eq!(remove(&added, "plugin", 1), COMMENTED);
}

#[test]
fn append_without_a_trailing_comma_puts_the_comma_before_the_comment() {
  let strict = "{\n  \"plugin\": [\n    \"a\" // first\n  ]\n}\n";
  let appended = append(strict, "plugin", "@toolu/opencode@1.0.0");
  assert_eq!(
    appended,
    "{\n  \"plugin\": [\n    \"a\", // first\n    \"@toolu/opencode@1.0.0\"\n  ]\n}\n"
  );
  assert_eq!(
    remove(&appended, "plugin", 1),
    "{\n  \"plugin\": [\n    \"a\" // first\n  ]\n}\n"
  );
}

#[test]
fn line_breaks_inside_a_block_comment_are_not_line_ends() {
  let pinned = "{\n  \"plugin\": [\n    \"other\",\n    \"@toolu/opencode@1.0.0\" /* pinned for\n       the team */\n  ]\n}\n";
  assert_eq!(
    remove(pinned, "plugin", 1),
    "{\n  \"plugin\": [\n    \"other\"\n  ]\n}\n"
  );
  let closing = "{\n  \"plugin\": [\n    \"a\" /* spans\n  */]\n}\n";
  assert_eq!(
    append(closing, "plugin", "b"),
    "{\n  \"plugin\": [\n    \"a\", /* spans\n  */\n    \"b\"]\n}\n"
  );
}

#[test]
fn inline_arrays_and_crlf_files_use_the_fallbacks_and_keep_their_line_ends() {
  let inline = "{ \"plugin\": [\"a\"] }\n";
  let added = append(inline, "plugin", "b");
  let parsed: serde_json::Value = serde_json::from_str(&added).unwrap_or_default();
  assert_eq!(parsed["plugin"], serde_json::json!(["a", "b"]));
  let dropped = remove("{ \"plugin\": [\"a\", \"b\"] }", "plugin", 0);
  let parsed: serde_json::Value = serde_json::from_str(&dropped).unwrap_or_default();
  assert_eq!(parsed["plugin"], serde_json::json!(["b"]));
  let crlf = "{\r\n  \"plugin\": [\r\n    \"a\"\r\n  ]\r\n}\r\n";
  let added = append(crlf, "plugin", "b");
  assert_eq!(
    added,
    "{\r\n  \"plugin\": [\r\n    \"a\",\r\n    \"b\"\r\n  ]\r\n}\r\n"
  );
  assert_eq!(remove(&added, "plugin", 1), crlf);
  let empty = append("{\n  \"plugin\": []\n}\n", "plugin", "b");
  assert!(empty.contains("\"b\""), "{empty}");
  let none = remove("{\n  \"plugin\": [\n    \"a\"\n  ]\n}\n", "plugin", 4);
  assert_eq!(none, "{\n  \"plugin\": [\n    \"a\"\n  ]\n}\n");
  assert_eq!(append("{}", "missing", "b"), "{\n  \"missing\": [\"b\"]\n}");
}

#[test]
fn removing_the_middle_element_keeps_its_neighbours() {
  let three = "{\n  \"plugin\": [\n    \"a\",\n    \"b\", // mine\n    \"c\"\n  ]\n}\n";
  assert_eq!(
    remove(three, "plugin", 1),
    "{\n  \"plugin\": [\n    \"a\",\n    \"c\"\n  ]\n}\n"
  );
  assert_eq!(
    remove(three, "plugin", 2),
    "{\n  \"plugin\": [\n    \"a\",\n    \"b\" // mine\n  ]\n}\n"
  );
}
