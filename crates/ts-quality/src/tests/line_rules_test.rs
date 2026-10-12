use std::path::Path;

use toolu_engine::quality::EditedFile;
use toolu_runtime::env::Env;
use toolu_state::detect::project::NodePackageManager;

use super::{
  catch_toast, component_file_name, confirm_alert, console_log, factories, manual_type_guard,
  mutable_props, raw_radix_import, react_hooks, suppression_comment, type_assertion,
};
use crate::file::{Limits, Place, TsFile};

type LineRule = fn(&TsFile<'_>) -> Option<String>;

fn found_in(root: &Path, path: &str, text: &str, rule: LineRule) -> Option<String> {
  let env = Env::default();
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
  rule(&TsFile::new(
    &file,
    text,
    at,
    Limits::default(),
    NodePackageManager::Bun,
  ))
}

fn found(path: &str, text: &str, rule: LineRule) -> Option<String> {
  found_in(Path::new("/nonexistent"), path, text, rule)
}

#[test]
fn type_assertions_skip_const_imports_reexports_and_comments() {
  let text = "const a = b as Foo;\nconst c = d as const;\nimport { x as y } from \"m\";\n\
    export { e as F };\n// g as Any\nconst h = (i) as string;\n";
  assert_eq!(
    found("src/a.ts", text, type_assertion).as_deref(),
    Some(
      "Forbidden 'as' type assertion in src/a.ts — use type guards or Zod\n\
       1:const a = b as Foo;\n6:const h = (i) as string;"
    )
  );
  assert_eq!(found("src/a.ts", "const a = 1;\n", type_assertion), None);
}

#[test]
fn hook_files_doing_too_much_and_factories_are_counted() {
  let hook = "const [a] = x;\nconst [b] = y;\nuseRef(1);\nuseEffect(f);\n";
  let message = found("src/useThing.ts", hook, react_hooks).unwrap_or_default();
  assert!(
    message.contains("(4 useState/useRef/useEffect)"),
    "{message}"
  );
  assert!(found("src/use-thing.ts", hook, react_hooks).is_some());
  assert_eq!(found("src/thing.ts", hook, react_hooks), None);
  assert_eq!(found("src/useThing.ts", "useRef(1);\n", react_hooks), None);
  let three = "export function createA() {}\nexport async function createB() {}\n\
    export function createC() {}\n";
  assert_eq!(
    found("src/f.ts", three, factories).as_deref(),
    Some("Too many factory functions in src/f.ts (3) — simplify construction")
  );
  assert_eq!(
    found("src/f.ts", "export function createA() {}\n", factories),
    None
  );
}

#[test]
fn manual_type_guards_matter_only_in_a_zod_project() {
  let guard = "function isUser(x: unknown): x is User { return true; }\n";
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(
    found_in(dir.path(), "src/g.ts", guard, manual_type_guard),
    None
  );
  std::fs::write(
    dir.path().join("package.json"),
    r#"{"dependencies":{"zod":"3"}}"#,
  )
  .unwrap();
  assert_eq!(
    found_in(dir.path(), "src/g.ts", guard, manual_type_guard).as_deref(),
    Some("Manual type guard in src/g.ts — use Zod schema instead")
  );
  assert_eq!(
    found_in(dir.path(), "src/g.ts", "const a = 1;\n", manual_type_guard),
    None
  );
}

#[test]
fn grab_bag_component_names_are_rejected() {
  let message = found("src/parts.tsx", "", component_file_name).unwrap_or_default();
  assert!(message.starts_with("Forbidden component filename 'parts.tsx' in src/parts.tsx"));
  assert!(found("src/card-sections.tsx", "", component_file_name).is_some());
  assert_eq!(found("src/card.tsx", "", component_file_name), None);
  assert_eq!(found("src/parts.ts", "", component_file_name), None);
}

#[test]
fn console_log_and_suppressions_are_capped_excerpts() {
  let logs =
    "console.log(1);\n// console.log(2);\n  console.log(3);\nconsole.log(4);\nconsole.log(5);\n";
  assert_eq!(
    found("src/c.ts", logs, console_log).as_deref(),
    Some(
      "Forbidden console.log in src/c.ts — use console.error/warn/info\n\
       1:console.log(1);\n3:  console.log(3);\n4:console.log(4);"
    )
  );
  assert_eq!(found("src/c.ts", "console.error(1);\n", console_log), None);
  let expect_error = "// @ts-expect-error\nconst a = 1;\n";
  assert!(found("src/s.ts", expect_error, suppression_comment).is_some());
  assert_eq!(
    found("src/s.test.ts", expect_error, suppression_comment),
    None
  );
  let ignored = "/** @ts-ignore */\n";
  assert!(
    found("src/s.test.ts", ignored, suppression_comment)
      .is_some_and(|message| message.ends_with("\n1:/** @ts-ignore */"))
  );
}

#[test]
fn frontend_rules_apply_under_components_and_routes() {
  let dialog = "confirm(\"x\");\nalert(1);\nAlertDialog.alert(2);\n// confirm(3)\n";
  assert_eq!(
    found("app/components/a.tsx", dialog, confirm_alert).as_deref(),
    Some(
      "Forbidden confirm()/alert() in app/components/a.tsx — use AlertDialog component\n\
       1:confirm(\"x\");\n2:alert(1);"
    )
  );
  assert_eq!(found("app/lib/a.tsx", dialog, confirm_alert), None);
  let toast = "try {\n} catch (e) {\n  toast(\"no\");\n}\ntoast(\"ok\");\n";
  assert_eq!(
    found("app/routes/r.tsx", toast, catch_toast).as_deref(),
    Some(
      "Manual try/catch+toast in app/routes/r.tsx — use shared error handling\n3:   toast(\"no\");"
    )
  );
  assert_eq!(found("app/lib/r.tsx", toast, catch_toast), None);
  assert_eq!(found("app/routes/r.tsx", "toast(1);\n", catch_toast), None);
}

#[test]
fn radix_imports_and_mutable_props() {
  let radix = "import { Root } from \"@radix-ui/react-dialog\";\n";
  assert!(found("app/x.tsx", radix, raw_radix_import).is_some());
  assert_eq!(
    found("app/packages/ui/x.tsx", radix, raw_radix_import),
    None
  );
  assert_eq!(
    found(
      "app/x.tsx",
      "// from '@radix-ui/react-dialog'\n",
      raw_radix_import
    ),
    None
  );
  let props = "function A(props: CardProps) {}\nfunction B(props: Readonly<CardProps>) {}\n";
  assert_eq!(
    found("app/a.tsx", props, mutable_props).as_deref(),
    Some("Mutable props in app/a.tsx — wrap in Readonly<Props>\n1:function A(props: CardProps) {}")
  );
  assert_eq!(found("app/a.tsx", "const a = 1;\n", mutable_props), None);
}
