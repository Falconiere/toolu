//! `shell-writes.test.ts`: every output redirection form, redirects on compound
//! commands, writes inside nested scripts, and pathname patterns. The file
//! operands of copy and in-place commands are in `writes/tests`.

use super::{WriteVia, write_targets};
use crate::analyze;

/// The static paths `source` writes.
pub(crate) fn paths(source: &str) -> Vec<Option<String>> {
  let analysis = analyze(source);
  write_targets(&analysis)
    .into_iter()
    .map(|target| target.path)
    .collect()
}

/// `(path, pattern, text)` of every target.
pub(crate) fn targets(source: &str) -> Vec<(Option<String>, Option<String>, String)> {
  let analysis = analyze(source);
  write_targets(&analysis)
    .into_iter()
    .map(|target| (target.path, target.pattern, target.text))
    .collect()
}

/// `(path, text)` of every target.
fn texts(source: &str) -> Vec<(Option<String>, String)> {
  let analysis = analyze(source);
  write_targets(&analysis)
    .into_iter()
    .map(|target| (target.path, target.text))
    .collect()
}

pub(crate) fn some(values: &[&str]) -> Vec<Option<String>> {
  values
    .iter()
    .map(|value| Some((*value).to_owned()))
    .collect()
}

#[test]
fn every_output_redirection_writes_its_target() {
  for source in [
    "echo x > .env",
    "echo x >.env",
    "echo SECRET=1>.env",
    "printf k=v 1>.env",
    "echo x >> .env",
    "echo x &>.env",
    "echo x &>>.env",
    "echo x >| .env",
    "echo x >&.env",
    "exec 3>.env",
    "exec 3<>.env",
    "{ echo x; } >.env",
    "(echo x) > .env",
  ] {
    assert_eq!(paths(source), some(&[".env"]), "{source}");
  }
}

#[test]
fn descriptor_duplication_and_input_write_nothing() {
  for source in [
    "cmd 2>&1",
    "cmd 2>&-",
    "cmd >&2",
    "cat < .env",
    "cat <<< x",
    "cat <<EOF\nx > .env\nEOF",
  ] {
    assert!(paths(source).is_empty(), "{source}");
  }
}

#[test]
fn a_dynamic_target_is_reported_not_dropped() {
  assert_eq!(texts("echo x > \"$OUT\""), [(None, "$OUT".to_owned())]);
  assert_eq!(
    texts("dd if=/dev/zero of=$HOME/.env"),
    [(None, "$HOME/.env".to_owned())]
  );
  assert_eq!(
    texts("cp x \"$DIR/.env\""),
    [(None, "$DIR/.env".to_owned())]
  );
  assert_eq!(
    texts("echo x | tee \"$ROOT\"/.env"),
    [(None, "$ROOT/.env".to_owned())]
  );
  assert_eq!(paths("cmd > $(echo .env)"), [None]);
}

#[test]
fn a_compound_redirect_has_no_command() {
  let compound = analyze("{ echo x; } >.env");
  assert!(write_targets(&compound)[0].command.is_none());
  let simple = analyze("echo x >.env");
  let argv = write_targets(&simple)[0]
    .command
    .map(|command| command.argv.clone());
  assert_eq!(argv, Some(some(&["echo", "x"])));
}

#[test]
fn tee_writes_every_operand() {
  let analysis = analyze("echo hi | tee -a .env notes.txt");
  let found: Vec<(WriteVia, Option<String>)> = write_targets(&analysis)
    .into_iter()
    .map(|target| (target.via, target.path))
    .collect();
  assert_eq!(
    found,
    [
      (WriteVia::Tee, Some(".env".to_owned())),
      (WriteVia::Tee, Some("notes.txt".to_owned()))
    ]
  );
}

#[test]
fn a_write_inside_bash_c_eval_or_a_substitution_is_found() {
  assert_eq!(paths("bash -c 'echo x > .env'"), some(&[".env"]));
  assert_eq!(paths("eval \"cp a .env\""), some(&[".env", ".env/a"]));
  assert_eq!(paths("echo \"$(echo hi > .env)\""), some(&[".env"]));
  assert_eq!(paths("cat <<EOF\n$(echo hi > .env)\nEOF"), some(&[".env"]));
  assert_eq!(
    paths("cat <<'EOF'\n$(echo hi > .env)\nEOF"),
    Vec::<Option<String>>::new()
  );
}

#[test]
fn a_redirect_target_is_one_word() {
  assert_eq!(paths("echo x > .env y"), some(&[".env"]));
  assert_eq!(
    analyze("echo x > .env y").commands[0].argv,
    some(&["echo", "x", "y"])
  );
  assert_eq!(paths("echo x > 'a'\\ 'b'"), some(&["a b"]));
}

/// The pattern of the first target and whether it is a static path.
fn pattern(source: &str) -> (Option<String>, Option<String>) {
  let analysis = analyze(source);
  let first = write_targets(&analysis).into_iter().next().unwrap();
  (first.path, first.pattern)
}

#[test]
fn an_unquoted_pattern_is_reported_as_a_pattern() {
  for (source, expected) in [
    ("echo x > .en[v]", ".en[v]"),
    ("echo x >.e?v", ".e?v"),
    ("printf k 1>.en*", ".en*"),
    ("echo x &>> .[e]nv", ".[e]nv"),
    ("cp src .en[v]", ".en[v]"),
    ("mv src .e?v", ".e?v"),
    ("echo x | tee .en[v]", ".en[v]"),
    ("sed -i s/a/b/ .en[v]", ".en[v]"),
    ("{ echo x; } > .e?v", ".e?v"),
  ] {
    assert_eq!(
      pattern(source),
      (None, Some(expected.to_owned())),
      "{source}"
    );
  }
  assert_eq!(
    pattern("cp -t apps/api src/.en[v]"),
    (None, Some("apps/api/.en[v]".to_owned()))
  );
}

#[test]
fn a_quoted_or_escaped_pattern_names_the_file_literally() {
  for (source, path) in [
    ("echo x > '.en[v]'", ".en[v]"),
    ("echo x > \".e?v\"", ".e?v"),
    ("echo x > .en\\[v\\]", ".en[v]"),
    ("echo x > .e\\?v", ".e?v"),
  ] {
    assert_eq!(pattern(source), (Some(path.to_owned()), None), "{source}");
  }
  assert_eq!(paths("dd if=x of=.en[v]"), some(&[".en[v]"]));
}

#[test]
fn every_via_has_its_typescript_name() {
  let vias = [
    WriteVia::Redirect,
    WriteVia::Tee,
    WriteVia::Sed,
    WriteVia::Perl,
    WriteVia::Cp,
    WriteVia::Mv,
    WriteVia::Install,
    WriteVia::Dd,
    WriteVia::Python,
  ];
  let names: Vec<&str> = vias.iter().map(|via| via.as_str()).collect();
  assert_eq!(
    names,
    [
      "redirect", "tee", "sed", "perl", "cp", "mv", "install", "dd", "python"
    ]
  );
}
