//! Where tree-sitter-bash reads a script differently from bash: the `time`
//! keyword and openers glued to the next character.

use super::{Fixups, blank, find, mask};
use crate::parse::Syntax;
use crate::{PARSE_BUDGET, analyze};

fn fixups(source: &str, time: bool) -> Fixups {
  let mut syntax = Syntax::new(PARSE_BUDGET).unwrap();
  let (tree, text) = syntax.script(source).unwrap();
  assert_eq!(text.len(), source.len());
  find(&tree, &text, time)
}

#[test]
fn the_time_keyword_is_blanked_before_a_pipeline_only() {
  assert_eq!(
    analyze("time git push").commands[0].words,
    [Some("git".to_owned()), Some("push".to_owned())]
  );
  assert_eq!(analyze("time -p git push").commands[0].text, "git push");
  let twice = analyze("time time git push");
  assert_eq!(twice.commands[0].wrappers, ["time"]);
  let piped = analyze("x | time git push");
  assert_eq!(piped.commands[1].wrappers, ["time"]);
  let grouped = analyze("time { git push; }");
  assert_eq!(grouped.commands[0].argv[0].as_deref(), Some("git"));
  let negated = analyze("time ! git push");
  assert_eq!(negated.commands[0].argv[0].as_deref(), Some("git"));
  assert!(!negated.commands[0].exit_proves);
  assert_eq!(
    analyze("time").commands,
    Vec::<crate::analysis::ShellCommand>::new()
  );
  assert_eq!(
    analyze("/usr/bin/time git push").commands[0].wrappers,
    ["time"]
  );
}

#[test]
fn glued_openers_are_words_not_tests_or_groups() {
  for source in ["[g]it push", "{git,} push", "[[g]]it push", "{node,} -e x"] {
    let analysis = analyze(source);
    assert_eq!(analysis.commands[0].argv[0], None, "{source}");
  }
  assert_eq!(
    analyze("[ -f x ]").commands[0].words[0].as_deref(),
    Some("[")
  );
  assert_eq!(
    analyze("{ git push; }").commands[0].argv[0].as_deref(),
    Some("git")
  );
}

#[test]
fn a_script_with_nothing_to_fix_parses_once() {
  assert_eq!(fixups("git push", true), Fixups::default());
  assert_eq!(fixups("echo time", true), Fixups::default());
}

#[test]
fn blank_and_mask_keep_every_offset() {
  let keyword = std::ops::Range { start: 0, end: 7 };
  assert_eq!(blank("time -p x", &[keyword]), "        x");
  assert_eq!(mask("[g]it", &[0]), "_g]it");
  assert_eq!(mask("ab", &[5]), "ab");
}
