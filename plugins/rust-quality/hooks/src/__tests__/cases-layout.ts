/**
 * Golden cases (#267) for test layout: tests.bats (the inline `#[cfg(test)]`
 * rule, its bodyless-decl exemption and attribute bound, and test placement
 * under `tests/`), plus the flat-`tests/` subdirectory rule.
 */
import { RUST_PROJECT, bats, wrote, type RsCase } from "./cases-types.ts";

const INLINE = "Inline #[cfg(test)]";
const OUTSIDE = "Rust test file outside tests/";
const ADD = "pub fn add(a: u8, b: u8) -> u8 {\n    a + b\n}\n\n";
const TEST_BODY = "fn it_works() {\n    assert_eq!(1, 1);\n}\n";
const MOD_BODY =
  "mod tests {\n    #[test]\n    fn it_adds() {\n        assert_eq!(super::add(1, 2), 3);\n    }\n}\n";

type Check = Pick<RsCase, "contains" | "absent">;

const layout = (title: string, file: string, body: string, check: Check, name = title): RsCase => ({
  name: `tests: ${name}`,
  from: bats("tests", `rust-quality: ${title}`),
  project: RUST_PROJECT,
  steps: wrote(file, body),
  expect: "advisory",
  ...check,
});

const attrs = (n: number): string =>
  Array.from({ length: n }, (_, i) => `#[path = "tests/p${String(i + 1)}.rs"]\n`).join("");

export const LAYOUT_CASES: readonly RsCase[] = [
  layout("test file (#[test]) outside tests/ is flagged", "src/foo.rs", `#[test]\n${TEST_BODY}`, {
    contains: [OUTSIDE],
  }),
  {
    ...layout(
      "integration test under tests/ is accepted",
      "tests/integration_test.rs",
      `#[test]\n${TEST_BODY}`,
      { absent: ["test file outside"] },
    ),
    expect: "silent",
  },
  layout(
    "bare #[rstest] test attribute outside tests/ is flagged",
    "src/foo.rs",
    `#[rstest]\n${TEST_BODY}`,
    {
      contains: [OUTSIDE],
    },
  ),
  layout(
    "#[test_log::test] attribute outside tests/ is flagged",
    "src/foo.rs",
    `#[test_log::test]\n${TEST_BODY}`,
    { contains: [OUTSIDE] },
  ),
  layout(
    "#[test_case(...)] attribute outside tests/ is flagged",
    "src/foo.rs",
    "#[test_case(1)]\nfn it_works(x: u8) {\n    assert_eq!(x, x);\n}\n",
    { contains: [OUTSIDE] },
  ),
  layout(
    "#[cfg(test)] is not caught by the widened test-attr rule",
    "src/lib.rs",
    `${ADD}#[cfg(test)]\n${MOD_BODY}`,
    { contains: [INLINE], absent: [OUTSIDE] },
  ),
  layout(
    "inline #[cfg(all(test, ...))] is flagged",
    "src/lib.rs",
    `${ADD}#[cfg(all(test, feature = "unit"))]\n${MOD_BODY}`,
    { contains: [INLINE] },
  ),
  layout(
    '#[cfg(not(test))] and feature="test-x" are NOT flagged as inline cfg(test)',
    "src/m.rs",
    '#[cfg(not(test))]\npub fn prod_only() -> u8 {\n    1\n}\n\n#[cfg(feature = "test-utils")]\npub fn helper() -> u8 {\n    2\n}\n',
    { absent: [INLINE] },
    "cfg(not(test)) and a test- feature string are not inline cfg(test)",
  ),
  layout(
    "canonical inline #[cfg(test)] mod produces one violation, not two",
    "src/lib.rs",
    `${ADD}#[cfg(test)]\n${MOD_BODY}`,
    { contains: [INLINE], absent: [OUTSIDE] },
    "canonical inline cfg(test) mod, one violation (lib.rs)",
  ),
  {
    ...layout(
      "#[bench] and #[wasm_bindgen_test] do not trigger tests/ placement",
      "benches/speed.rs",
      "#[bench]\nfn bench_it(b: &mut Bencher) {\n    b.iter(|| 1 + 1);\n}\n",
      { absent: ["test file outside"] },
      "#[bench] does not trigger tests/ placement",
    ),
    expect: "silent",
  },
  {
    name: "tests: #[wasm_bindgen_test] does not trigger tests/ placement",
    from: bats(
      "tests",
      "rust-quality: #[bench] and #[wasm_bindgen_test] do not trigger tests/ placement",
    ),
    project: RUST_PROJECT,
    steps: wrote(
      "src/wasm_checks.rs",
      `#[wasm_bindgen_test]\nfn browser_works() {\n    assert_eq!(1, 1);\n}\n`,
    ),
    expect: "silent",
    absent: ["test file outside"],
  },
  layout(
    "single-line bodyless #[cfg(test)] mod tests; passes",
    "src/foo.rs",
    `${ADD}#[cfg(test)] mod tests;\n`,
    { absent: [INLINE] },
  ),
  layout(
    "rustfmt two-line bodyless #[cfg(test)] / mod tests; passes",
    "src/foo.rs",
    `${ADD}#[cfg(test)]\nmod tests;\n`,
    { absent: [INLINE] },
  ),
  layout(
    "two-line bodyless #[cfg(test)] / pub mod foo_tests; passes",
    "src/foo.rs",
    `${ADD}#[cfg(test)]\npub mod foo_tests;\n`,
    { absent: [INLINE] },
  ),
  layout(
    "#[path]-wired bodyless #[cfg(test)] mod tests; passes",
    "src/api/stats.rs",
    `${ADD}#[cfg(test)]\n#[path = "tests/stats.rs"]\nmod tests;\n`,
    { absent: [INLINE] },
  ),
  layout(
    "a run of attributes before the bodyless mod decl still passes",
    "src/foo.rs",
    `${ADD}#[cfg(test)]\n#[path = "tests/foo.rs"]\n#[cfg_attr(miri, ignore)]\npub mod foo_tests;\n`,
    { absent: [INLINE] },
  ),
  layout(
    "a decl behind the full attribute bound is exempt",
    "src/foo.rs",
    `${ADD}#[cfg(test)]\n${attrs(8)}mod tests;\n`,
    { absent: [INLINE] },
  ),
  layout(
    "a decl behind more attributes than the bound is still flagged",
    "src/foo.rs",
    `${ADD}#[cfg(test)]\n${attrs(9)}mod tests;\n`,
    { contains: [INLINE] },
  ),
  layout(
    "#[path]-wired #[cfg(test)] mod tests { with a body still fails",
    "src/foo.rs",
    `${ADD}#[cfg(test)]\n#[path = "tests/foo.rs"]\n${MOD_BODY}`,
    { contains: [INLINE] },
  ),
  layout(
    "single-line #[cfg(test)] mod tests { with a body still fails",
    "src/foo.rs",
    `${ADD}#[cfg(test)] ${MOD_BODY}`,
    { contains: [INLINE] },
  ),
  layout(
    "two-line #[cfg(test)] / mod tests { with a body still fails",
    "src/foo.rs",
    `${ADD}#[cfg(test)]\n${MOD_BODY}`,
    { contains: [INLINE] },
  ),
  layout(
    "#[cfg(all(test, ...))] followed by bodyless mod tests; still fails",
    "src/foo.rs",
    `${ADD}#[cfg(all(test, feature = "unit"))]\nmod tests;\n`,
    { contains: [INLINE] },
  ),
  layout(
    "#[cfg(test)] only in a doc comment does NOT suppress placement enforcement",
    "src/foo.rs",
    `/// Example usage: annotate with #[cfg(test)] in your own crate.\n#[tokio::test]\nasync ${TEST_BODY}`,
    { contains: [OUTSIDE], absent: [INLINE] },
  ),
  layout(
    "cfg(all(test)) with no comma is still flagged",
    "src/allnocomma.rs",
    "#[cfg(all(test))]\nmod tests {\n    fn t() {}\n}\n",
    { contains: [INLINE] },
  ),
  layout(
    "cfg(all(test , feature)) with space before comma is still flagged",
    "src/allspace.rs",
    '#[cfg(all(test , feature = "x"))]\nmod tests;\n',
    { contains: [INLINE] },
  ),
  {
    ...layout(
      "bodyless decl with trailing // comment is exempt",
      "src/trailing.rs",
      "#[cfg(test)]\nmod tests; // unit tests live in tests/\nfn real() {}\n",
      { absent: [INLINE] },
    ),
    expect: "silent",
  },
  {
    name: "tests: cfg(test) as the last line fails closed",
    project: RUST_PROJECT,
    steps: wrote("src/tail.rs", `${ADD}#[cfg(test)]\n`),
    expect: "advisory",
    contains: [INLINE],
  },
  {
    name: "tests: a nested tests/ subdirectory is flagged",
    project: RUST_PROJECT,
    steps: wrote("tests/deep/it_test.rs", `#[test]\n${TEST_BODY}`),
    expect: "advisory",
    contains: ["Rust test nested in tests/ subdirectory"],
  },
  {
    name: "tests: tests/common is an allowed subdirectory",
    project: RUST_PROJECT,
    steps: wrote("tests/common/helpers_test.rs", `#[test]\n${TEST_BODY}`),
    expect: "silent",
  },
  {
    name: "tests: a _test.rs name outside tests/ is flagged",
    project: RUST_PROJECT,
    steps: wrote("src/parse_test.rs", "fn check() {}\n"),
    expect: "advisory",
    contains: [OUTSIDE],
  },
];
