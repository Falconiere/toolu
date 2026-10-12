use std::path::Path;
use std::process::Command;

use toolu_engine::quality::EditedFile;
use toolu_runtime::config::quality::ThresholdSource;
use toolu_runtime::env::Env;
use toolu_state::detect::project::{NodePackageManager, TsLinter};

use super::{any_long_function, file_too_long, function_too_long, hint, strip};
use crate::file::{Limits, Place, TsFile, split_lines};

/// The size rules over `text`, written to `<root>/src/big.ts`.
fn sized(root: &Path, text: &str, limits: Limits) -> (Option<String>, Option<String>) {
  let env = Env::default();
  let absolute = root.join("src/big.ts");
  std::fs::create_dir_all(root.join("src")).unwrap();
  std::fs::write(&absolute, text).unwrap();
  let file = EditedFile {
    path: "src/big.ts".to_owned(),
    absolute,
    removed: false,
  };
  let at = Place {
    env: &env,
    project_root: root,
    cwd: root,
  };
  let f = TsFile::new(&file, text, at, limits, NodePackageManager::Bun);
  (file_too_long(&f), function_too_long(&f))
}

#[test]
fn the_hint_names_where_the_limit_came_from() {
  let native = hint(Some(TsLinter::Eslint), ThresholdSource::Native, 300);
  assert_eq!(
    native,
    "split into smaller modules (eslint enforces this max-lines limit)"
  );
  let biome = hint(Some(TsLinter::Biome), ThresholdSource::Default, 300);
  assert!(biome.contains("biome has no max-lines equivalent — gate uses the 300-line default"));
  let unparsed = hint(Some(TsLinter::Oxc), ThresholdSource::Default, 200);
  assert!(unparsed.contains("is present but the gate's limit didn't come from its config"));
  assert_eq!(
    hint(None, ThresholdSource::Override, 9),
    "split into smaller modules"
  );
}

#[test]
fn a_file_over_its_limit_names_its_count_and_linter() {
  let dir = tempfile::tempdir().unwrap();
  let init = Command::new("git")
    .arg("-C")
    .arg(dir.path())
    .args(["init", "-q"])
    .status();
  assert!(init.unwrap().success());
  std::fs::write(dir.path().join("biome.json"), "{}").unwrap();
  let limits = Limits {
    file_lines: 2,
    ..Limits::default()
  };
  let (file, _) = sized(dir.path(), "a();\n\n// note\nb();\nc();\n", limits);
  assert_eq!(
    file.as_deref(),
    Some(
      "TS file exceeds 2-line limit: src/big.ts (3 code lines, blanks/comments excluded) — \
       split into smaller modules (biome has no max-lines equivalent — gate uses the 2-line default)"
    )
  );
  let (unterminated, _) = sized(dir.path(), "a();\nb();\nc();\n/* open\n", limits);
  assert!(
    unterminated
      .unwrap_or_default()
      .contains("(size approximated")
  );
  let (fits, _) = sized(dir.path(), "a();\n", limits);
  assert_eq!(fits, None);
}

#[test]
fn a_long_function_is_found_by_brace_depth() {
  let dir = tempfile::tempdir().unwrap();
  let limits = Limits {
    fn_lines: 3,
    ..Limits::default()
  };
  let long = "export function run() {\n  a();\n  b();\n  c();\n  d();\n}\n";
  let (_, function) = sized(dir.path(), long, limits);
  assert_eq!(
    function.as_deref(),
    Some("Function too long in src/big.ts (>3 lines) — simplify or split")
  );
  let short = "const f = () => {\n  a();\n};\n";
  assert_eq!(sized(dir.path(), short, limits).1, None);
}

#[test]
fn the_counter_starts_at_declarations_and_methods_only() {
  let method = split_lines("class A {\n  run(): void {\n    a();\n    b();\n  }\n}\n");
  assert!(any_long_function(&method, 2));
  assert!(!any_long_function(&method, 3));
  let call = split_lines("  if (x) {\n    a();\n    b();\n    c();\n  }\n");
  assert!(!any_long_function(&call, 1));
  let declared = split_lines("function f(): void;\nconst g = 1;\n");
  assert!(!any_long_function(&declared, 0));
  let braces_in_strings = split_lines("function f() {\n  s(\"}\");\n  t('{');\n}\n");
  assert!(any_long_function(&braces_in_strings, 2));
  assert_eq!(strip(r#"a("x\"y", 'z', `w`)"#), "a(, , )");
}
