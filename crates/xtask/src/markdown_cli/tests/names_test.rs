use std::collections::BTreeSet;

use super::judge_name;

fn external(names: &[&str]) -> BTreeSet<String> {
  names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn listed_external_commands_pass_and_unlisted_ones_fail() {
  let list = external(&["ast-grep", "gh"]);
  assert_eq!(judge_name("ast-grep", &[], &list), None);
  assert_eq!(judge_name("gh", &[], &list), None);
  let problem = judge_name("ast-grep", &[], &external(&["gh"])).unwrap();
  assert!(problem.contains("external allow-list"), "{problem}");
  assert!(judge_name("mod.sh", &[], &list).is_some());
}

#[test]
fn builtins_functions_paths_and_variables_pass() {
  let none = BTreeSet::new();
  for name in ["[", "export", "local", "break", "printf", "cd"] {
    assert_eq!(judge_name(name, &[], &none), None, "{name}");
  }
  assert_eq!(judge_name("judge", &["judge".to_owned()], &none), None);
  for name in [
    "\"$JEV_BUN\"",
    "$S/route.ts",
    "plugins/ast-grep/hooks/dist/ast-grep.js",
    "./install.sh",
  ] {
    assert_eq!(judge_name(name, &[], &none), None, "{name}");
  }
}
