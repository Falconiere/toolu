//! Redirections: operators, descriptors, closed descriptors, `<>`, herestrings,
//! one-word targets, and the script a command reads on stdin.

use super::stdin_script;
use crate::analysis::{RedirectOperator, ShellRedirect};
use crate::analyze;

fn redirects(source: &str) -> Vec<ShellRedirect> {
  analyze(source).commands.swap_remove(0).redirects
}

fn spelled(source: &str) -> Vec<(&'static str, Option<u32>, Option<String>)> {
  let found = redirects(source);
  found
    .into_iter()
    .map(|r| (r.operator.as_str(), r.fd, r.target))
    .collect()
}

fn target(value: &str) -> Option<String> {
  value.parse().ok()
}

#[test]
fn every_operator_is_read_with_its_descriptor() {
  assert_eq!(spelled("cmd 2>&1"), [(">&", Some(2), target("1"))]);
  assert_eq!(spelled("cmd 2>&-"), [(">&", Some(2), target("-"))]);
  assert_eq!(spelled("cmd <&-"), [("<&", None, target("-"))]);
  assert_eq!(
    spelled("cmd < in >> out"),
    [("<", None, target("in")), (">>", None, target("out"))]
  );
  assert_eq!(
    spelled("cmd &>a &>>b >|c"),
    [
      ("&>", None, target("a")),
      ("&>>", None, target("b")),
      (">|", None, target("c"))
    ]
  );
  assert_eq!(spelled("exec 3<>.env"), [("<>", Some(3), target(".env"))]);
  assert_eq!(spelled("cat <<< 'x y'"), [("<<<", None, target("x y"))]);
}

#[test]
fn a_descriptor_glued_to_its_redirect_is_not_a_word() {
  let analysis = analyze("cmd 0<file");
  assert_eq!(analysis.commands[0].redirects[0].fd, Some(0));
  assert_eq!(analysis.commands[0].argv.len(), 1);
  let named = analyze("exec {fd}>file");
  assert_eq!(named.commands[0].redirects[0].fd, None);
  assert_eq!(named.commands[0].words, [Some("exec".to_owned())]);
}

#[test]
fn the_target_is_the_first_word_after_the_operator() {
  let analysis = analyze("echo hi > out.txt more args");
  assert_eq!(
    analysis.commands[0].redirects[0].target.as_deref(),
    Some("out.txt")
  );
  assert_eq!(analysis.commands[0].argv.len(), 4);
}

#[test]
fn stdin_is_a_static_heredoc_or_herestring_on_descriptor_zero() {
  assert_eq!(
    stdin_script(&redirects("bash <<'EOF'\ngit push\nEOF")).as_deref(),
    Some("git push\n")
  );
  assert_eq!(
    stdin_script(&redirects("bash <<< 'git push'")).as_deref(),
    Some("git push")
  );
  assert_eq!(
    stdin_script(&redirects("bash 0<<< 'git push'")).as_deref(),
    Some("git push")
  );
  assert_eq!(stdin_script(&redirects("bash 3<<< 'git push'")), None);
  assert_eq!(stdin_script(&redirects("bash < script.sh")), None);
  assert!(
    redirects("bash <<EOF\n$x\nEOF")[0]
      .heredoc
      .as_ref()
      .is_some_and(|h| h.content.is_none())
  );
}

#[test]
fn a_redirect_target_runs_its_substitutions() {
  let analysis = analyze("echo hi > \"$(git push)\"");
  assert_eq!(analysis.commands[0].argv[0].as_deref(), Some("git"));
  assert_eq!(
    analysis.commands[1].redirects[0].operator,
    RedirectOperator::Out
  );
}
