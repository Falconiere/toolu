//! Heredocs: operators, quoting, static content, `<<-` tab stripping, the
//! `"$(cat <<EOF … EOF)"` value, and what tree-sitter nests after the delimiter.

use crate::analysis::{Heredoc, RedirectOperator, ShellRedirect};
use crate::analyze;

fn heredoc(source: &str) -> ShellRedirect {
  let analysis = analyze(source);
  analysis.commands[0].redirects[0].clone()
}

fn body(content: Option<&str>, quoted: bool) -> Heredoc {
  Heredoc {
    content: content.map(str::to_owned),
    quoted,
  }
}

#[test]
fn a_quoted_body_is_static_data() {
  for source in [
    "cat <<'EOF'\n$x `y`\nEOF",
    "cat <<\"EOF\"\n$x\nEOF",
    "cat <<\\EOF\n$x\nEOF",
  ] {
    let found = heredoc(source);
    assert!(
      found
        .heredoc
        .as_ref()
        .is_some_and(|h| h.quoted && h.content.is_some()),
      "{source}"
    );
  }
}

#[test]
fn an_unquoted_body_is_static_only_without_expansions() {
  assert_eq!(
    heredoc("cat <<EOF\nplain\nEOF").heredoc,
    Some(body(Some("plain\n"), false))
  );
  assert_eq!(
    heredoc("cat <<EOF\n$x\nEOF").heredoc,
    Some(body(None, false))
  );
  assert_eq!(
    heredoc("cat <<EOF\na`b`\nEOF").heredoc,
    Some(body(None, false))
  );
}

#[test]
fn strip_tabs_removes_leading_tabs_from_every_line() {
  let found = heredoc("cat <<-EOF\n\tone\n\t\ttwo\n\tEOF");
  assert_eq!(found.operator, RedirectOperator::HeredocStrip);
  assert_eq!(found.heredoc, Some(body(Some("one\ntwo\n"), false)));
}

#[test]
fn trailing_words_and_redirects_belong_to_the_command() {
  let analysis = analyze("cat <<EOF file.txt >out more\nx\nEOF");
  let argv: Vec<Option<&str>> = analysis.commands[0]
    .argv
    .iter()
    .map(Option::as_deref)
    .collect();
  assert_eq!(argv, [Some("cat"), Some("file.txt"), Some("more")]);
  let targets: Vec<Option<&str>> = analysis.commands[0]
    .redirects
    .iter()
    .map(|r| r.target.as_deref())
    .collect();
  assert_eq!(targets, [Some("out"), None]);
  assert_eq!(analysis.errors, Vec::<crate::analysis::ShellError>::new());
  assert_eq!(
    analysis.commands[0].text.len(),
    "cat <<EOF file.txt >out more".len()
  );
}

#[test]
fn the_cat_heredoc_value_needs_cat_alone_with_one_heredoc() {
  let value = |source: &str| analyze(source).commands.pop().unwrap().words[1].clone();
  assert_eq!(
    value("echo \"$(cat <<'EOF'\nhi\n\nEOF\n)\"").as_deref(),
    Some("hi")
  );
  assert_eq!(value("echo \"$(cat -n <<'EOF'\nhi\nEOF\n)\""), None);
  assert_eq!(value("echo \"$(cat <<'EOF' >x\nhi\nEOF\n)\""), None);
  assert_eq!(value("echo \"$(cat <<'EOF' &\nhi\nEOF\n)\""), None);
  assert_eq!(value("echo \"$(tac <<'EOF'\nhi\nEOF\n)\""), None);
  assert_eq!(value("echo \"$(cat <<'EOF'\nhi\nEOF\necho)\""), None);
  assert_eq!(value("echo \"$(cat < f)\""), None);
  assert_eq!(value("echo $(cat <<'EOF'\nhi\nEOF\n)"), None);
}

#[test]
fn a_lone_dash_before_a_heredoc_is_kept() {
  for source in ["python3 - <<'EOF'\nx\nEOF", "cat -<<EOF\nx\nEOF"] {
    let argv = analyze(source).commands[0].argv.clone();
    assert_eq!(
      argv.get(1).cloned().flatten().as_deref(),
      Some("-"),
      "{source}"
    );
  }
}

#[test]
fn a_body_starts_after_the_first_newline_no_backslash_escapes() {
  assert_eq!(super::body_start("<<EOF\n  x\nEOF", 5), Some(6));
  // `foo \` continues the delimiter's line onto the next one.
  assert_eq!(
    super::body_start("<<EOF | foo \\\n\nbody\nEOF", 5),
    Some(15)
  );
  assert_eq!(super::body_start("<<EOF \\\\\nbody", 5), Some(9));
  assert_eq!(super::body_start("<<EOF", 5), None);
}
