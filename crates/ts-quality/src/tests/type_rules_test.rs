use std::path::Path;
use std::process::Command;

use toolu_engine::quality::EditedFile;
use toolu_runtime::env::Env;
use toolu_state::detect::project::NodePackageManager;

use super::{duplicate_types, throw_literal};
use crate::file::{Limits, Place, TsFile};

/// The type rules for `path` (absolute under `root`) holding `text`.
fn typed(root: &Path, path: &str, text: &str) -> (Vec<String>, Option<String>) {
  let env = Env::from_pairs([("PATH", "/usr/bin:/bin:/opt/homebrew/bin:/usr/local/bin")]);
  let file = EditedFile {
    path: path.to_owned(),
    absolute: root.join(path),
    removed: false,
  };
  let at = Place {
    env: &env,
    project_root: root,
    cwd: root,
  };
  let f = TsFile::new(&file, text, at, Limits::default(), NodePackageManager::Bun);
  (duplicate_types(&f), throw_literal(&f))
}

#[test]
fn an_exported_type_another_package_exports_is_a_duplicate() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let status = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(["init", "-q"])
    .status()
    .unwrap();
  assert!(status.success());
  std::fs::create_dir_all(root.join("packages/core")).unwrap();
  std::fs::write(
    root.join("packages/core/user.ts"),
    "export interface User {\n}\n",
  )
  .unwrap();
  let mine = format!("{}/apps/web/user.ts", root.display());
  let text = "export interface User {\n}\nexport type Other = 1;\n";
  std::fs::create_dir_all(root.join("apps/web")).unwrap();
  std::fs::write(&mine, text).unwrap();
  assert_eq!(
    typed(Path::new("/"), &mine, text).0,
    Vec::<String>::new(),
    "outside the project root git finds nothing"
  );
  let (dups, _) = typed(root, &mine, text);
  assert_eq!(
    dups,
    [format!(
      "Type 'User' in {mine} already defined in packages/core/user.ts — import instead of redefining"
    )]
  );
  assert_eq!(typed(root, "apps/web/a.ts", "const a = 1;\n").0.len(), 0);
}

#[test]
fn throws_of_literals_ignore_comments() {
  let dir = tempfile::tempdir().unwrap();
  let text = "throw 1;\n/* throw null */\n// throw undefined\nrethrow 2;\nthrow false\nthrow new Error(\"x\");\n";
  assert_eq!(
    typed(dir.path(), "src/t.ts", text).1.as_deref(),
    Some(
      "throw of non-Error literal in src/t.ts — throw an Error (or subclass) instead\n\
       1: throw 1;\n5: throw false"
    )
  );
  assert_eq!(
    typed(dir.path(), "src/t.ts", "throw new Error(\"x\");\n").1,
    None
  );
}
