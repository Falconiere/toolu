use super::{above, docstring, sentence};

#[test]
fn outline_doc_reads_the_first_paragraph_without_tags() {
  let lines = [
    "/**",
    " * Opens a box.",
    " * Then closes it.",
    " *",
    " * Later text.",
    " * @param x size",
    " */",
  ];
  assert_eq!(
    above(&lines).as_deref(),
    Some("Opens a box. Then closes it.")
  );
  assert_eq!(above(&["#[derive(Debug)]"]), None);
}

#[test]
fn outline_doc_reads_a_raw_multi_line_docstring() {
  let lines = [
    "def f():",
    "    r\"\"\"",
    "    Do it.",
    "    \"\"\"",
    "    return 1",
  ];
  assert_eq!(
    docstring(&lines, Some((0, 7)), 4).as_deref(),
    Some("Do it.")
  );
  assert_eq!(docstring(&["def g(): return 1"], Some((0, 7)), 0), None);
}

#[test]
fn outline_doc_sentence_stops_at_the_first_full_stop() {
  assert_eq!(sentence("One. Two.").as_deref(), Some("One."));
  assert_eq!(sentence("  "), None);
}
