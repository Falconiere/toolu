//! Quote removal, ANSI-C decoding (unbash's `decodeAnsiCQuoted`), backtick
//! bodies, and the pathname-pattern and brace-expansion tests.

use super::{ansi_c, backticks, double_quoted, has_brace, has_glob, unquoted};

#[test]
fn unquoted_text_drops_backslashes_and_continuations() {
  assert_eq!(unquoted(r"a\ b"), "a b");
  assert_eq!(unquoted("a\\\nb"), "ab");
  assert_eq!(unquoted(r"z\*.ts"), "z*.ts");
  assert_eq!(unquoted("trailing\\"), "trailing\\");
}

#[test]
fn double_quotes_escape_only_dollar_backtick_quote_backslash_newline() {
  assert_eq!(double_quoted(r#"c\"d"#), "c\"d");
  assert_eq!(double_quoted(r"\$x \` \\ \e"), "$x ` \\ \\e");
  assert_eq!(double_quoted("a\\\nb"), "ab");
  assert_eq!(double_quoted("end\\"), "end\\");
}

#[test]
fn ansi_c_decodes_every_escape_unbash_decodes() {
  assert_eq!(
    ansi_c(r#"e\tf\n\r\a\b\f\v\E\e\\\'\"\?"#),
    "e\tf\n\r\u{7}\u{8}\u{c}\u{b}\u{1b}\u{1b}\\'\"?"
  );
  assert_eq!(ansi_c(r"\x41\101é\U0001F600"), "AAé😀");
  assert_eq!(ansi_c(r"\xZ \q"), r"\xZ \q");
  assert_eq!(ansi_c("a\\\nb"), "ab");
  assert_eq!(ansi_c(r"\cA\c?\c"), "\u{1}\u{7f}\\c");
  assert_eq!(ansi_c(r"\c\\x"), "\u{1c}x");
  assert_eq!(ansi_c(r"\c\x"), "\u{1c}x");
  assert_eq!(ansi_c(r"\c\"), "\u{1c}");
  assert_eq!(ansi_c(r"\UFFFFFFFF"), r"\UFFFFFFFF");
  assert_eq!(ansi_c(r"\777"), "\u{ff}");
  assert_eq!(ansi_c("end\\"), "end\\");
}

#[test]
fn backtick_bodies_lose_backslashes_before_dollar_backtick_backslash() {
  assert_eq!(
    backticks(r"echo \`date\` \$x \\ \n"),
    r"echo `date` $x \ \n"
  );
}

#[test]
fn a_glob_is_an_unescaped_star_question_or_bracket_pair() {
  for raw in ["x*.ts", ".e?v", ".en[v]", "a]b[c]"] {
    assert!(has_glob(raw), "{raw}");
  }
  for raw in ["plain", r"z\*.ts", r".en\[v\]", "a[b", "]["] {
    assert!(!has_glob(raw), "{raw}");
  }
}

#[test]
fn brace_expansion_needs_a_comma_or_a_range() {
  for raw in ["{a,b}.txt", "a{b,c}d", "{,}", "{1..3}", "{a..b}", "{x,{y}}"] {
    assert!(has_brace(raw), "{raw}");
  }
  for raw in ["{}", "{a}", "a,b", r"\{a,b\}", "{a", "a}", "}{"] {
    assert!(!has_brace(raw), "{raw}");
  }
}
