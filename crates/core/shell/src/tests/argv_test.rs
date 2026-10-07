//! `shell-argv.test.ts`: wrappers are peeled with their own options, `command -v`
//! runs nothing, xargs appends run-time arguments, and shell strings are
//! analyzed to a fixed depth.

use super::{align, basename, run_target, unwrap};
use crate::analysis::{CommandOrigin, Word};
use crate::{MAX_RUN_DEPTH, analyze};

fn words(list: &[&str]) -> Vec<Word> {
  list.iter().map(|word| Some((*word).to_owned())).collect()
}

/// `(argv, wrappers)` of every command `source` runs.
fn runs(source: &str) -> Vec<(Vec<Word>, Vec<String>)> {
  let analysis = analyze(source);
  analysis
    .commands
    .into_iter()
    .map(|c| (c.argv, c.wrappers))
    .collect()
}

fn git_push() -> Vec<Word> {
  words(&["git", "push"])
}

#[test]
fn wrappers_are_peeled_with_their_options() {
  let cases: [(&str, &[&str]); 19] = [
    ("timeout 120 git push", &["timeout"]),
    ("timeout -s KILL -k 5 60 git push", &["timeout"]),
    ("nice -n 10 git push", &["nice"]),
    ("nice -10 git push", &["nice"]),
    ("sudo -u me git push", &["sudo"]),
    ("sudo -E VAR=1 git push", &["sudo"]),
    ("/usr/bin/sudo git push", &["sudo"]),
    ("doas -u me git push", &["doas"]),
    ("env -i PATH=/usr/bin git push", &["env"]),
    ("env -u HOME -C /tmp git push", &["env"]),
    ("env -P /usr/bin git push", &["env"]),
    ("env - git push", &["env"]),
    ("env -i - A=1 git push", &["env"]),
    ("command git push", &["command"]),
    ("builtin git push", &["builtin"]),
    ("exec -a name git push", &["exec"]),
    ("nohup git push", &["nohup"]),
    ("stdbuf -oL -e 0 git push", &["stdbuf"]),
    (
      "sudo -u me timeout 60 nice -n 5 git push",
      &["sudo", "timeout", "nice"],
    ),
  ];
  for (source, wrappers) in cases {
    let wrappers: Vec<String> = wrappers.iter().map(|name| (*name).to_owned()).collect();
    assert_eq!(runs(source), [(git_push(), wrappers)], "{source}");
  }
}

#[test]
fn the_words_as_written_keep_the_wrappers() {
  let analysis = analyze("sudo -u me git push");
  assert_eq!(
    analysis.commands[0].words,
    words(&["sudo", "-u", "me", "git", "push"])
  );
}

#[test]
fn xargs_appends_arguments_read_at_run_time() {
  let found = runs("echo main | xargs git push origin");
  assert_eq!(found[0], (words(&["echo", "main"]), vec![]));
  let mut argv = words(&["git", "push", "origin"]);
  argv.push(None);
  assert_eq!(found[1], (argv, vec!["xargs".to_owned()]));
  let last = runs("xargs -I {} -n 1 git push origin {}").pop().unwrap();
  assert_eq!(
    last.0,
    [Some("git"), Some("push"), Some("origin"), Some("{}"), None].map(|w| w.map(str::to_owned))
  );
}

#[test]
fn wrappers_that_run_no_command_are_not_peeled() {
  for source in [
    "command -v git",
    "sudo -l",
    "sudo -e /etc/hosts",
    "env",
    "timeout 5",
    "exec 3>.env",
  ] {
    assert!(runs(source)[0].1.is_empty(), "{source}");
  }
  assert_eq!(
    runs("command -v git")[0].0,
    words(&["command", "-v", "git"])
  );
}

#[test]
fn a_wrapper_hiding_its_command_makes_it_unknown() {
  assert_eq!(
    runs("env -S 'git push'"),
    [(vec![None], vec!["env".to_owned()])]
  );
  assert_eq!(runs("sudo $CMD"), [(vec![None], vec!["sudo".to_owned()])]);
}

#[test]
fn shell_strings_are_analyzed_as_their_own_lines() {
  for source in [
    "bash -c 'git push'",
    "sh -c \"git push\"",
    "bash -lc 'git push'",
    "zsh -o pipefail -c 'git push'",
    "/bin/dash -e -c 'git push'",
    "ksh -c 'git push' arg0",
    "bash +c 'git push'",
    "bash +o posix -c 'git push'",
    "bash --rcfile x -c 'git push'",
  ] {
    let inner = analyze(source).commands.pop().unwrap();
    assert_eq!(
      (inner.argv, inner.origin, inner.depth),
      (git_push(), CommandOrigin::Shell, 1),
      "{source}"
    );
  }
}

#[test]
fn a_static_heredoc_or_herestring_fed_to_a_shell_is_code() {
  let heredoc = analyze("bash <<'EOF'\ngit push\nEOF")
    .commands
    .pop()
    .unwrap();
  assert_eq!(
    (heredoc.argv, heredoc.origin),
    (git_push(), CommandOrigin::Shell)
  );
  assert_eq!(
    analyze("bash <<< \"git push\"")
      .commands
      .pop()
      .unwrap()
      .argv,
    git_push()
  );
  let cat = analyze("cat <<'EOF'\ngit push\nEOF");
  assert_eq!(cat.commands.len(), 1);
}

#[test]
fn eval_joins_its_arguments() {
  let inner = analyze("eval \"git push\" origin").commands.pop().unwrap();
  assert_eq!(
    (inner.argv, inner.origin),
    (words(&["git", "push", "origin"]), CommandOrigin::Eval)
  );
  assert_eq!(
    analyze("eval -- git push").commands.pop().unwrap().argv,
    git_push()
  );
  assert_eq!(analyze("eval").commands.len(), 1);
}

#[test]
fn an_unreadable_shell_string_is_an_unknown_command() {
  for source in ["bash -c \"$CMD\"", "eval $CMD", "curl -s x | bash", "sh -s"] {
    let inner = analyze(source).commands.pop().unwrap();
    assert_eq!(inner.argv, [None], "{source}");
  }
  assert_eq!(
    runs("bash deploy.sh"),
    [(words(&["bash", "deploy.sh"]), vec![])]
  );
}

#[test]
fn recursion_stops_at_the_depth_limit() {
  let mut source = "git push".to_owned();
  for _ in 0..MAX_RUN_DEPTH {
    source = format!("bash -c {}", serde_json::to_string(&source).unwrap());
  }
  assert_eq!(analyze(&source).commands.pop().unwrap().argv, git_push());
  let deeper = format!("bash -c {}", serde_json::to_string(&source).unwrap());
  let last = analyze(&deeper).commands.pop().unwrap();
  assert_eq!((last.argv, last.depth), (vec![None], MAX_RUN_DEPTH + 1));
}

#[test]
fn the_status_of_a_shell_string_is_its_last_commands() {
  let proves = |source: &str| analyze(source).commands.pop().unwrap().exit_proves;
  assert!(proves("bash -c 'cd x && bun test'"));
  assert!(proves("bash -c 'bun test | tail'"));
  let pipeline = analyze("bash -c 'bun test | tail'");
  assert!(!pipeline.commands[pipeline.commands.len() - 2].exit_proves);
  assert!(!proves("bash -c 'bun test' || true"));
}

#[test]
fn a_command_under_xargs_proves_nothing() {
  let proves = |source: &str| -> Vec<(Word, bool)> {
    analyze(source)
      .commands
      .into_iter()
      .map(|c| (c.argv[0].clone(), c.exit_proves))
      .collect()
  };
  assert_eq!(
    proves("printf \"\" | xargs -r bun test"),
    [
      (Some("printf".to_owned()), false),
      (Some("bun".to_owned()), false)
    ]
  );
  assert_eq!(
    proves("xargs bash -c 'bun test'").pop(),
    Some((Some("bun".to_owned()), false))
  );
}

#[test]
fn argv_words_keep_their_dequoted_text() {
  let command = analyze("sudo cp \"$HOME/a b\" 'c d' e")
    .commands
    .pop()
    .unwrap();
  assert_eq!(
    command.argv,
    [
      Some("cp".to_owned()),
      None,
      Some("c d".to_owned()),
      Some("e".to_owned())
    ]
  );
  assert_eq!(command.texts, ["cp", "$HOME/a b", "c d", "e"]);
}

#[test]
fn unwrap_and_align_cut_the_words_to_the_command() {
  let unwrapped = unwrap(&words(&["nohup", "git", "push"]));
  assert_eq!(
    (unwrapped.start, unwrapped.wrappers.clone()),
    (Some(1), vec!["nohup".to_owned()])
  );
  assert_eq!(align(&[1, 2, 3], &unwrapped, &0), [2, 3]);
  let hidden = unwrap(&words(&["env", "-S", "git push"]));
  assert_eq!(align(&[1, 2, 3], &hidden, &0), [0]);
  let missing = unwrap(&words(&["sudo", "-u"]));
  assert_eq!(missing.wrappers, Vec::<String>::new());
}

#[test]
fn basename_reads_paths_as_node_does() {
  for (path, name) in [
    ("/usr/bin/sudo", "sudo"),
    ("sudo", "sudo"),
    ("dir/", "dir"),
    ("/", ""),
    ("", ""),
  ] {
    assert_eq!(basename(path), name, "{path}");
  }
}

#[test]
fn run_targets_cover_shells_eval_and_neither() {
  assert_eq!(run_target(&words(&["ls", "-c", "x"])), None);
  assert_eq!(run_target(&[None, Some("x".to_owned())]), None);
  let stdin = run_target(&words(&["bash", "-s"])).unwrap();
  assert!(stdin.stdin && stdin.script.is_none());
  let dash = run_target(&words(&["bash", "-", "x"]));
  assert_eq!(dash, None);
  let dynamic = run_target(&[Some("bash".to_owned()), None]).unwrap();
  assert!(!dynamic.stdin && dynamic.script.is_none());
  assert_eq!(run_target(&words(&["bash", "-c"])), None);
}
