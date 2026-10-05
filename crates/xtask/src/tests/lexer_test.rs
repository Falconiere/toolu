use super::lex;

#[test]
fn blank_and_comment_lines_are_not_code() {
  let lines =
    lex("//! doc\n\nfn f() {\n  // note\n  let x = 1; // trailing\n  /* block\n     still */\n}\n");
  assert_eq!(
    lines.code,
    [false, false, true, false, true, false, false, true, false]
  );
  assert_eq!(lines.code_lines(), 3);
  let texts: Vec<(usize, &str)> = lines
    .comments
    .iter()
    .map(|(at, t)| (*at, t.as_str()))
    .collect();
  assert_eq!(
    texts,
    [
      (1, "! doc"),
      (4, " note"),
      (5, " trailing"),
      (6, " block\n     still ")
    ]
  );
}

#[test]
fn strings_raw_strings_and_chars_never_open_comments() {
  let source = "let a = \"/* not\";\nlet b = r#\"// not \"either\"\"#;\nlet c = '\"';\nlet d = '\\'';\nlet e: &'static str = \"x\";\n";
  let lines = lex(source);
  assert_eq!(lines.code_lines(), 5);
  assert_eq!(lines.comments, Vec::new(), "{:?}", lines.comments);
}

#[test]
fn nested_block_comments_and_multiline_strings() {
  let lines = lex("/* a /* b */ c */\nlet s = \"one\ntwo\\\nthree\";\nlet r = r\"x\ny\";\n");
  assert_eq!(lines.code, [false, true, true, true, true, true, false]);
  assert_eq!(lines.comments.len(), 1);
}

#[test]
fn ranges_count_only_their_lines_and_unterminated_comments_close() {
  let lines = lex("a\nb\n// c\nd\n/* open");
  assert_eq!(lines.code_lines_between(2, 4), 2);
  assert_eq!(lines.code_lines_between(5, 9), 0);
  assert_eq!(lines.comments.last().map(|(at, _)| *at), Some(5));
  assert_eq!(lex("x // end").comments.len(), 1);
}

#[test]
fn raw_identifiers_and_lifetimes_are_code() {
  let lines = lex("let r#type = 1;\nfn f<'a>(x: &'a str) {}\nlet c = '\\u{1F600}';\n");
  assert_eq!(lines.code_lines(), 3);
  assert_eq!(lines.comments, Vec::new());
}
