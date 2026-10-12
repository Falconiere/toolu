use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use toolu_engine::quality::EditedFile;
use toolu_runtime::env::Env;
use toolu_state::detect::project::NodePackageManager;

use super::{bre, docs, duplication, runner, typecheck_command, unhandled_await};
use crate::file::{Limits, Place, TsFile};

type Advisory = fn(&TsFile<'_>) -> String;

fn advised(env: &Env, root: &Path, path: &str, text: &str, check: Advisory) -> String {
  let file = EditedFile {
    path: path.to_owned(),
    absolute: root.join(path),
    removed: false,
  };
  let at = Place {
    env,
    project_root: root,
    cwd: root,
  };
  check(&TsFile::new(
    &file,
    text,
    at,
    Limits::default(),
    NodePackageManager::Npm,
  ))
}

fn plain(path: &str, text: &str, check: Advisory) -> String {
  advised(
    &Env::default(),
    Path::new("/nonexistent"),
    path,
    text,
    check,
  )
}

#[test]
fn package_managers_map_to_their_runner_and_typecheck() {
  assert_eq!(runner(NodePackageManager::Bun).1, ["bunx", "jscpd"]);
  assert_eq!(runner(NodePackageManager::Pnpm).1, ["pnpm", "dlx", "jscpd"]);
  assert_eq!(runner(NodePackageManager::Yarn).0, "yarn");
  assert_eq!(runner(NodePackageManager::Npm).0, "npx");
  assert_eq!(
    typecheck_command(NodePackageManager::Bun),
    "bun run typecheck"
  );
  assert_eq!(
    typecheck_command(NodePackageManager::Pnpm),
    "pnpm -w typecheck"
  );
  assert_eq!(
    typecheck_command(NodePackageManager::Yarn),
    "yarn typecheck"
  );
  assert_eq!(
    typecheck_command(NodePackageManager::Npm),
    "npm run typecheck"
  );
}

#[test]
fn a_basic_regex_keeps_ere_operators_literal() {
  assert_eq!(bre("a+b?(c){d}|e"), r"a\+b\?\(c\)\{d\}\|e");
  assert_eq!(bre("^a^b$c$"), r"^a\^b\$c$");
}

#[test]
fn duplication_runs_the_projects_jscpd_on_its_package() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let bin = root.join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  std::fs::create_dir_all(root.join("apps/web")).unwrap();
  std::fs::write(root.join(".jscpd.json"), "{}").unwrap();
  let npx = bin.join("npx");
  std::fs::write(
    &npx,
    "#!/bin/sh\necho \"Found 1 clones.\"\necho \"web/a.ts matched\" >&2\n",
  )
  .unwrap();
  std::fs::set_permissions(&npx, std::fs::Permissions::from_mode(0o755)).unwrap();
  let env = Env::from_pairs([("PATH", format!("{}:/usr/bin:/bin", bin.display()))]);
  let path = format!("{}/apps/web/a.ts", root.display());
  assert_eq!(
    advised(&env, root, &path, "", duplication),
    format!(
      "Code duplication detected involving {path} — deduplicate or run 'npm run typecheck' before commit"
    )
  );
  assert_eq!(
    advised(
      &env,
      root,
      &format!("{}/apps/web/b.ts", root.display()),
      "",
      duplication
    ),
    ""
  );
  assert_eq!(
    advised(&env, root, "apps/web/a.test.ts", "", duplication),
    ""
  );
  assert_eq!(advised(&env, root, "src/a.ts", "", duplication), "");
  assert_eq!(advised(&env, root, "apps/none/a.ts", "", duplication), "");
}

#[test]
fn exported_api_needs_a_concise_jsdoc() {
  let text = "export function a() {}\n/** A. */\nexport function b() {}\n// x\n\
    export const C = 1;\n/**\n * B.\n */\nexport class D {}\n";
  assert_eq!(
    plain("src/api.ts", text, docs),
    "Exported API missing a JSDoc in src/api.ts — add a concise /** */ doc:\n\
     1: export function a() {}\n5: export const C = 1;"
  );
  let long = format!("/**\n{}*/\nexport function e() {{}}\n", " * x\n".repeat(12));
  assert_eq!(
    plain("src/api.ts", &long, docs),
    "Verbose JSDoc in src/api.ts — docs must be present but concise:\n\
     1: JSDoc block is 14 lines — trim to the essentials"
  );
  for exempt in ["src/index.ts", "src/a.d.ts", "src/a.test.ts"] {
    assert_eq!(plain(exempt, text, docs), "");
  }
}

#[test]
fn await_needs_a_handler_somewhere_in_the_file() {
  let message = plain("src/a.ts", "await go();\n", unhandled_await);
  assert!(message.starts_with("Async code in src/a.ts uses await with no try/catch"));
  assert_eq!(
    plain(
      "src/a.ts",
      "try {\n  await go();\n} finally {}\n",
      unhandled_await
    ),
    ""
  );
  assert_eq!(
    plain("src/a.ts", "await go().catch(log);\n", unhandled_await),
    ""
  );
  assert_eq!(plain("src/a.ts", "// await go();\n", unhandled_await), "");
}
