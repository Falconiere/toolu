use toolu_shell::analyze;

use super::quality_commands;

fn labels(command: &str) -> Vec<String> {
  quality_commands(&analyze(command))
    .into_iter()
    .map(|quality| quality.label)
    .collect()
}

fn assert_labels(cases: &[(&str, &[&str])]) {
  for (command, want) in cases {
    assert_eq!(labels(command), *want, "{command}");
  }
}

#[test]
fn quality_commands_trigger_directly_and_through_runners_and_shells() {
  assert_labels(&[
    ("cargo test", &["cargo test"]),
    ("  cargo clippy", &["cargo clippy"]),
    ("cd crate && cargo test", &["cargo test"]),
    ("bun test", &["bun test"]),
    ("bun run check", &["bun run check"]),
    ("cd web && bun test", &["bun test"]),
    ("find . | cargo test", &["cargo test"]),
    ("foo | tsc", &["tsc"]),
    ("tsc", &["tsc"]),
    ("tsc --noEmit", &["tsc"]),
    ("bun run ts:check:fix", &["bun run ts:check:fix"]),
    ("cargo nextest run", &["cargo nextest"]),
    ("vitest run", &["vitest"]),
    ("jest --ci", &["jest"]),
    ("tools/api/check.sh", &["tools/api/check.sh"]),
    ("bash tools/api/check.sh", &["tools/api/check.sh"]),
    ("sh tools/web/test.sh --fast", &["tools/web/test.sh"]),
    ("bash ./scripts/ts-check.sh", &["./scripts/ts-check.sh"]),
    ("./scripts/ts-check.sh", &["./scripts/ts-check.sh"]),
    ("npx tsc --noEmit", &["tsc"]),
    ("npx -y vitest run", &["vitest"]),
    ("bunx vitest run", &["vitest"]),
    ("bun x tsc", &["tsc"]),
    ("pnpm exec jest", &["jest"]),
    ("yarn tsc -b", &["tsc"]),
  ]);
}

#[test]
fn parsed_forms_reach_through_wrappers_paths_and_toolchains() {
  assert_labels(&[
    ("./tools/api/check.sh", &["tools/api/check.sh"]),
    ("timeout 600 bun test", &["bun test"]),
    ("cargo +nightly clippy", &["cargo clippy"]),
    ("/usr/local/bin/tsc -p .", &["tsc"]),
    ("bash -c 'bun test'", &["bun test"]),
    ("sudo -u ci cargo test", &["cargo test"]),
  ]);
}

#[test]
fn every_quality_command_in_the_line_is_reported_in_order() {
  assert_eq!(
    labels("bun run lint && bun test 2>&1 | tail -5"),
    ["bun run lint", "bun test"]
  );
}

#[test]
fn names_prose_and_other_verbs_do_not_trigger() {
  let none: &[&str] = &[];
  assert_labels(&[
    ("cat tsconfig.json", none),
    ("ls tooling/foo/test.sh", none),
    ("vitests-helper", none),
    ("cattsc", none),
    ("echo \"remember to run bun test later\"", none),
    ("git commit -m \"fix: make cargo test pass\"", none),
    ("grep -rn tsc docs", none),
    ("bun run dev", none),
    ("bun install", none),
    ("cargo run", none),
    ("npx prettier --check .", none),
    ("t() { bun test; }", none),
    ("$RUNNER test", none),
  ]);
}

#[test]
fn argv_boundaries_match_typescript() {
  let none: &[&str] = &[];
  assert_labels(&[
    ("./node_modules/.bin/tsc --noEmit", &["tsc"]),
    ("/usr/local/bin/cargo test", &["cargo test"]),
    ("cargo +nightly test", &["cargo test"]),
    ("/repo/tools/api/check.sh", &["tools/api/check.sh"]),
    ("yarn run vitest", none),
    ("npx -p typescript tsc", none),
    ("bash -x ./scripts/ts-check.sh", none),
  ]);
}

#[test]
fn a_wrapper_script_needs_a_tools_dir_of_its_own() {
  let none: &[&str] = &[];
  assert_labels(&[
    ("mytools/api/check.sh", none),
    ("tools/check.sh", none),
    ("tools//check.sh", none),
    ("tools/a b/check.sh", none),
    ("tools/api/lint.sh", none),
    ("x/tools/web.v2/format.sh", &["tools/web.v2/format.sh"]),
    ("npx", none),
    ("bash", none),
  ]);
}
