/**
 * Test layout (#267), the port of 20-tests: inline `#[cfg(test)]` under src/
 * (a bare one wiring a bodyless `mod name;` is exempt), then where a test
 * file lives. `isRustTest` is the `_is_rust_test` flag the size, error and
 * docs rules also read.
 */
import { ere, pathHas, testFileName, type RsFile } from "./rs-file.ts";

const CFG_COMBINATOR = ere(String.raw`^[[:space:]]*#\[cfg\((all|any)\(test([^A-Za-z0-9_]|$)`);
const MOD_DECL = String.raw`(pub[[:space:]]+)?mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*;[[:space:]]*(//.*)?$`;
const CFG_TEST_WITH_DECL = ere(String.raw`^[[:space:]]*#\[cfg\(test\)\][[:space:]]*${MOD_DECL}`);
const CFG_TEST_ALONE = ere(String.raw`^[[:space:]]*#\[cfg\(test\)\][[:space:]]*$`);
const CFG_TEST = ere(String.raw`^[[:space:]]*#\[cfg\(test\)\]`);
const OUTER_ATTR = ere(String.raw`^[[:space:]]*#\[[^!]`);
const BODYLESS_DECL = ere(String.raw`^[[:space:]]*${MOD_DECL}`);
const TEST_ATTR = ere(
  String.raw`^[[:space:]]*#\[([A-Za-z_][A-Za-z0-9_]*::)*(test|test_case)\b|^[[:space:]]*#\[rstest\b`,
);

/** 8 attributes plus the decl line itself. */
const DECL_LOOKAHEAD = 9;

/** The awk scan: lines a bare `#[cfg(test)]` consumes looking for its decl are not re-examined. */
function hasInlineCfgTest(lines: readonly string[]): boolean {
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i] ?? "";
    if (CFG_COMBINATOR.test(line)) return true;
    if (CFG_TEST_WITH_DECL.test(line)) continue;
    if (CFG_TEST_ALONE.test(line)) {
      let next = "";
      for (let read = 0; read < DECL_LOOKAHEAD; read += 1) {
        i += 1;
        if (i >= lines.length) return true;
        next = lines[i] ?? "";
        if (!OUTER_ATTR.test(next)) break;
      }
      if (BODYLESS_DECL.test(next)) continue;
      return true;
    }
    if (CFG_TEST.test(line)) return true;
  }
  return false;
}

/** Inline `#[cfg(test)]` in a src/ file. */
export function inlineCfgTest(f: RsFile): boolean {
  return pathHas(f, "/src/") && hasInlineCfgTest(f.lines);
}

/** `_is_rust_test`: a `_test.rs`/`_tests.rs` name, or a test attribute in the body. */
export function isRustTest(f: RsFile): boolean {
  return testFileName(f.file.path) || f.lines.some((line) => TEST_ATTR.test(line));
}

const ALLOWED_SUBDIRS = new Set(["fixtures", "helpers", "common"]);

/** 20-tests. */
export function testLayout(f: RsFile, isTest: boolean): string[] {
  const path = f.file.path;
  if (inlineCfgTest(f)) {
    return [
      `Inline #[cfg(test)] in ${path} — unit tests belong in a module-sibling tests/ dir wired by a bodyless #[cfg(test)] mod tests; decl; crate-root tests/ is for integration tests`,
    ];
  }
  if (!isTest) return [];
  if (!path.includes("/tests/")) {
    return [`Rust test file outside tests/: ${path} — move to a sibling tests/ directory`];
  }
  const after = path.slice(path.lastIndexOf("/tests/") + "/tests/".length);
  const slash = after.indexOf("/");
  if (slash === -1 || ALLOWED_SUBDIRS.has(after.slice(0, slash))) return [];
  return [
    `Rust test nested in tests/ subdirectory: ${path} — keep tests/ flat (only fixtures/helpers/common subdirs allowed)`,
  ];
}
