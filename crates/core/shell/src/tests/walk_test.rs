//! `shell-walk.test.ts`: every construct that runs a command yields it with the
//! right origin, quoted heredoc bodies stay data, compound redirects are
//! reported, and the exit-status rule holds per position.

use crate::analysis::CommandOrigin;
use crate::{MAX_NESTING, analyze};

fn pushes(source: &str) -> Vec<CommandOrigin> {
  let analysis = analyze(source);
  let push = |argv: &[Option<String>]| {
    argv.first().and_then(Option::as_deref) == Some("git")
      && argv.get(1).and_then(Option::as_deref) == Some("push")
  };
  analysis
    .commands
    .iter()
    .filter(|c| push(&c.argv))
    .map(|c| c.origin)
    .collect()
}

#[test]
fn a_command_is_reported_inside_every_construct() {
  use CommandOrigin::{Function, Line, Substitution};
  let cases = [
    ("echo $(git push)", Substitution),
    ("echo `git push`", Substitution),
    ("diff <(git push) x", Substitution),
    ("echo x | tee >(git push)", Substitution),
    ("(cd x && git push)", Line),
    ("{ git push; }", Line),
    ("case $x in a) git push;; esac", Line),
    ("deploy() { git push; }", Function),
    ("[[ -n $(git push) ]]", Substitution),
    ("(( $(git push) + 1 ))", Substitution),
    ("echo $(( $(git push) + 1 ))", Substitution),
    ("echo ${X:-$(git push)}", Substitution),
    ("x=$(git push)", Substitution),
    ("echo hi > $(git push)", Substitution),
    ("cat <<EOF\n$(git push)\nEOF", Substitution),
    ("if git push; then :; fi", Line),
    ("while true; do git push; done", Line),
    ("for x in $(git push); do :; done", Substitution),
    ("for ((i=$(git push);;)); do :; done", Substitution),
    ("select x in a; do git push; done", Line),
  ];
  for (source, origin) in cases {
    assert_eq!(pushes(source), [origin], "{source}");
  }
}

#[test]
fn a_quoted_heredoc_body_is_data() {
  assert_eq!(
    pushes("cat <<'EOF'\n$(git push)\nEOF"),
    Vec::<CommandOrigin>::new()
  );
  assert_eq!(
    pushes("cat <<\"EOF\"\ngit push\nEOF"),
    Vec::<CommandOrigin>::new()
  );
  assert_eq!(
    pushes("cat <<EOF\ngit push\nEOF"),
    Vec::<CommandOrigin>::new()
  );
}

#[test]
fn redirects_on_compound_commands_are_compound_redirects() {
  let targets = |source: &str| -> Vec<Option<String>> {
    analyze(source)
      .compound_redirects
      .into_iter()
      .map(|r| r.target)
      .collect()
  };
  let one = |value: &str| vec![Some(value.to_owned())];
  assert_eq!(targets("{ echo x; } >.env"), one(".env"));
  assert_eq!(targets("(echo x) >> .env"), one(".env"));
  assert_eq!(
    targets("for i in 1; do echo $i; done > out.txt"),
    one("out.txt")
  );
  assert_eq!(targets("f() { echo x; } > log"), one("log"));
  assert_eq!(targets("echo x > .env"), Vec::<Option<String>>::new());
}

/// `(argv[0], pipeline index, exit proves)` of every command.
fn view(source: &str) -> Vec<(String, usize, bool)> {
  let analysis = analyze(source);
  let name = |argv: &[Option<String>]| argv.first().cloned().flatten().unwrap_or_default();
  analysis
    .commands
    .iter()
    .map(|c| (name(&c.argv), c.pipeline.index, c.exit_proves))
    .collect()
}

fn row(name: &str, index: usize, proves: bool) -> (String, usize, bool) {
  (name.to_owned(), index, proves)
}

#[test]
fn pipeline_position_and_exit_observability_follow_bash() {
  assert_eq!(
    view("bun test 2>&1 | tail -20"),
    [row("bun", 0, false), row("tail", 1, true)]
  );
  assert_eq!(
    view("cd x && bun test"),
    [row("cd", 0, true), row("bun", 0, true)]
  );
  assert_eq!(
    view("bun test || true"),
    [row("bun", 0, false), row("true", 0, false)]
  );
  assert_eq!(
    view("bun test; echo done"),
    [row("bun", 0, false), row("echo", 0, true)]
  );
  assert_eq!(view("bun test &"), [row("bun", 0, false)]);
  assert_eq!(view("! bun test"), [row("bun", 0, false)]);
  assert_eq!(
    view("a || b && bun test"),
    [row("a", 0, false), row("b", 0, false), row("bun", 0, true)]
  );
  assert_eq!(view("(bun test)"), [row("bun", 0, true)]);
  assert_eq!(
    view("if bun test; then echo ok; fi"),
    [row("bun", 0, false), row("echo", 0, false)]
  );
  assert_eq!(
    view("echo $(bun test)"),
    [row("bun", 0, false), row("echo", 0, true)]
  );
}

#[test]
fn a_list_under_one_redirect_is_read_as_bash_groups_it() {
  let rows = view("bun run lint && bun test 2>&1 | tail -5");
  assert_eq!(
    rows,
    [
      row("bun", 0, true),
      row("bun", 0, false),
      row("tail", 1, true)
    ]
  );
  let analysis = analyze("bun run lint && bun test 2>&1 | tail -5");
  assert_eq!(analysis.commands[1].redirects.len(), 1);
  assert_eq!(
    analysis.compound_redirects,
    Vec::<crate::analysis::ShellRedirect>::new()
  );
}

#[test]
fn heredoc_continuations_are_put_back_in_the_line() {
  assert_eq!(
    view("cat <<EOF | bash\ngit push\nEOF"),
    [
      row("cat", 0, false),
      row("bash", 1, true),
      row("", 0, false)
    ]
  );
  assert_eq!(
    view("cat <<'EOF' >f && git push\nbody\nEOF"),
    [row("cat", 0, true), row("git", 0, true)]
  );
  assert_eq!(
    view("cat <<EOF | grep x && git push\nbody\nEOF"),
    [
      row("cat", 0, false),
      row("grep", 1, true),
      row("git", 0, true)
    ]
  );
}

#[test]
fn words_keep_static_values_and_mark_expansions_and_globs() {
  let analysis = analyze("echo 'a b' \"c\\\"d\" $'e\\tf' $HOME \"$(date)\" x*.ts 'y*.ts' z\\*.ts");
  let names: Vec<Option<String>> = analysis
    .commands
    .iter()
    .map(|c| c.argv[0].clone())
    .collect();
  assert_eq!(names, [Some("date".to_owned()), Some("echo".to_owned())]);
  let words: Vec<Option<&str>> = analysis.commands[1]
    .words
    .iter()
    .map(Option::as_deref)
    .collect();
  assert_eq!(
    words,
    [
      Some("echo"),
      Some("a b"),
      Some("c\"d"),
      Some("e\tf"),
      None,
      None,
      None,
      Some("y*.ts"),
      Some("z*.ts")
    ]
  );
  assert_eq!(analysis.commands[1].patterns[6].as_deref(), Some("x*.ts"));
}

#[test]
fn a_heredoc_piped_through_cat_in_double_quotes_is_its_body() {
  let message = "feat(core): x\n\nBody line with $HOME and `ticks`.";
  let source = format!("git commit -m \"$(cat <<'EOF'\n{message}\nEOF\n)\"");
  let words = analyze(&source).commands.pop().unwrap().words;
  assert_eq!(words[3].as_deref(), Some(message));
  let tabbed = analyze("git commit -m \"$(cat <<-EOF\n\tfeat: y\n\tEOF\n)\"")
    .commands
    .pop()
    .unwrap();
  assert_eq!(tabbed.words[3].as_deref(), Some("feat: y"));
  let expanding = analyze("git commit -m \"$(cat <<EOF\nfeat: $X\nEOF\n)\"")
    .commands
    .pop()
    .unwrap();
  assert_eq!(expanding.words[3], None);
}

#[test]
fn nesting_past_the_limit_is_unknown() {
  let deep = format!(
    "{}x{}",
    "$(".repeat(MAX_NESTING + 2),
    ")".repeat(MAX_NESTING + 2)
  );
  let analysis = analyze(&deep);
  assert!(analysis.unknown);
  assert!(
    analysis
      .errors
      .iter()
      .any(|e| e.message.starts_with("nesting:"))
  );
  let shallow = format!(
    "{}x{}",
    "$(".repeat(MAX_NESTING - 2),
    ")".repeat(MAX_NESTING - 2)
  );
  assert!(!analyze(&shallow).unknown);
}

#[test]
fn escaped_backticks_inside_backticks_are_parsed_decoded() {
  let analysis = analyze("echo `echo \\`node -e x\\``");
  let names: Vec<Option<String>> = analysis
    .commands
    .iter()
    .map(|c| c.argv[0].clone())
    .collect();
  assert_eq!(
    names,
    [
      Some("node".to_owned()),
      Some("echo".to_owned()),
      Some("echo".to_owned())
    ]
  );
  assert_eq!(analysis.commands[1].text, "echo `node -e x`");
}

#[test]
fn a_case_terminator_outside_a_case_is_an_error() {
  let analysis = analyze("git push;;");
  assert_eq!(analysis.errors[0].message, "unexpected token ';;'");
  assert!(!analysis.commands[0].exit_proves);
}

#[test]
fn a_bare_redirect_is_a_command_without_words() {
  let analysis = analyze("echo $(< file)");
  assert_eq!(analysis.commands[0].words, Vec::<Option<String>>::new());
  assert_eq!(analysis.commands[0].text, "< file");
  assert_eq!(
    analysis.commands[0].redirects[0].target.as_deref(),
    Some("file")
  );
}
