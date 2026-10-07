//! The glob matcher against real bash `case` patterns on every (path, glob)
//! pair, the docs-sync defaults included (`glob.test.ts`).

use toolu_runtime::config::docs_sync::DocsSyncKey;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use super::{matches, matches_any};

const PATHS: [&str; 16] = [
  "README.md",
  "docs/config.md",
  "docs/releases/v1.md",
  "plugins/toolu/README.md",
  "plugins/x/skills/y/SKILL.md",
  "a/b/workflows/w.md",
  "src/a.ts",
  "lib/foo.sh",
  "plugins/x/commands/c.md",
  ".claude-plugin/plugin.json",
  "tsconfig.json",
  "knip.config.json",
  "dir with space/é.ts",
  "[x].md",
  "a\\b",
  "-dash.ts",
];

const EXTRA: [&str; 19] = [
  "?EADME.md",
  "[a-d]ocs/*",
  "[!d]*",
  "[^d]*",
  "*[[:space:]]*",
  "*[[:upper:]]*.md",
  "\\[x\\].md",
  "[x].md",
  "[",
  "a\\\\b",
  "*.[tj]s",
  "[]]*",
  "*/*/*",
  "-*",
  "",
  "[[:digit:]x-z]*",
  "*[[:punct:]]json",
  "*.[!t]s",
  "[a-]*",
];

fn globs() -> Vec<String> {
  let keys = [
    DocsSyncKey::Surfaces,
    DocsSyncKey::SurfaceExcludes,
    DocsSyncKey::CodeSurfaces,
  ];
  let mut out: Vec<String> = keys
    .iter()
    .flat_map(|key| key.defaults().iter().map(|glob| (*glob).to_owned()))
    .collect();
  out.extend(EXTRA.iter().map(|glob| (*glob).to_owned()));
  out
}

#[test]
fn the_matcher_agrees_with_bash_case_on_every_pair() {
  let globs = globs();
  let script = "for g in \"${@:2}\"; do case \"$1\" in $g) printf 1;; *) printf 0;; esac; done";
  let path_var = std::env::var("PATH").unwrap();
  for path in PATHS {
    let mut spec = Spec::new(["bash", "-c", script, "_", path]);
    spec.argv.extend(globs.iter().cloned());
    spec.env = Some(Env::from_pairs([
      ("LC_ALL", "C.UTF-8"),
      ("PATH", path_var.as_str()),
    ]));
    let bash: String = run(&spec)
      .unwrap()
      .stdout
      .chars()
      .zip(&globs)
      .map(|(bit, glob)| if glob.is_empty() { '0' } else { bit })
      .collect();
    let ours: String = globs
      .iter()
      .map(|glob| {
        if matches_any(path, std::slice::from_ref(glob)) {
          '1'
        } else {
          '0'
        }
      })
      .collect();
    assert_eq!((path, ours), (path, bash));
  }
}

#[test]
fn an_unclosed_bracket_is_a_literal_and_an_empty_glob_matches_nothing() {
  assert!(matches("[", "["));
  assert!(matches("a[b", "a[b"));
  assert!(!matches_any("x", &[String::new()]));
  assert!(matches("", ""));
  assert!(matches("", "*"));
}
