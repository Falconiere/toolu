/** Golden cases (#267) for the line rules: suppression.bats, unsafe.bats and docs.bats, plus the verbose-doc advisory. */
import { RUST_PROJECT, bats, wrote, type RsCase } from "./cases-types.ts";

const SUPPRESSION = "Forbidden lint suppression";
const TEST_HINT = "only the file-level #![allow(...)] header is accepted";
const UNSAFE = "Forbidden unsafe code";
const DOCS = "missing a /// doc";

const rule = (
  suite: string,
  title: string,
  file: string,
  body: string,
  check: Pick<RsCase, "contains" | "absent" | "expect">,
): RsCase => ({
  name: `${suite}: ${title}`,
  from: bats(suite, `rust-quality: ${title}`),
  project: RUST_PROJECT,
  steps: wrote(file, body),
  ...check,
});

const IT_ADDS = "#[test]\nfn it_adds() {\n    assert_eq!(1 + 1, 2);\n}\n";
const LONG_DOC = `${Array.from({ length: 13 }, (_, i) => `/// line ${String(i + 1)}\n`).join("")}pub fn documented() -> u8 {\n    1\n}\n`;

export const RULE_CASES: readonly RsCase[] = [
  rule(
    "suppression",
    "cfg_attr(allow) lint suppression is flagged",
    "src/bad.rs",
    "#[cfg_attr(test, allow(dead_code))]\nfn helper() {}\n",
    { expect: "advisory", contains: [SUPPRESSION] },
  ),
  rule(
    "suppression",
    "#[allow] mentioned in a line comment is NOT flagged",
    "src/ok.rs",
    "// historically this used #[allow(dead_code)] elsewhere; removed now.\nfn helper() -> u8 {\n    1\n}\n",
    { expect: "silent", absent: [SUPPRESSION] },
  ),
  rule(
    "suppression",
    "file-level #![allow] header in a module-sibling test is NOT flagged",
    "src/api/tests/stats.rs",
    `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]\n\n${IT_ADDS}`,
    { expect: "silent", absent: [SUPPRESSION] },
  ),
  rule(
    "suppression",
    "file-level #![allow] header in a crate-root integration test is NOT flagged",
    "tests/integration.rs",
    "#![allow(clippy::unwrap_used)]\n\n#[test]\nfn it_runs() {\n    assert!(true);\n}\n",
    { expect: "silent", absent: [SUPPRESSION] },
  ),
  rule(
    "suppression",
    "inner #![cfg_attr(..., allow(...))] in a test file is NOT flagged",
    "src/api/tests/stats.rs",
    `#![cfg_attr(miri, allow(clippy::unwrap_used))]\n\n${IT_ADDS}`,
    { expect: "silent", absent: [SUPPRESSION] },
  ),
  rule(
    "suppression",
    "per-item #[allow] inside a test file IS still flagged",
    "src/api/tests/stats.rs",
    `#[allow(dead_code)]\nfn helper() {}\n\n${IT_ADDS}`,
    { expect: "advisory", contains: [SUPPRESSION, TEST_HINT] },
  ),
  rule(
    "suppression",
    "file-level #![allow] header outside tests/ IS still flagged",
    "src/lib.rs",
    "#![allow(clippy::unwrap_used)]\n\npub fn add(a: u8, b: u8) -> u8 {\n    a + b\n}\n",
    { expect: "advisory", contains: [SUPPRESSION], absent: [TEST_HINT] },
  ),
  {
    name: "suppression: #[expect] is flagged",
    project: RUST_PROJECT,
    steps: wrote("src/e.rs", "#[expect(unused)]\nfn helper() {}\n"),
    expect: "advisory",
    contains: [SUPPRESSION],
  },
  rule(
    "unsafe",
    "unsafe block in src/ is flagged",
    "src/u.rs",
    "pub fn f() {\n    unsafe {\n        do_thing();\n    }\n}\n",
    { expect: "advisory", contains: [UNSAFE] },
  ),
  rule(
    "unsafe",
    "unsafe mentioned only in a comment is NOT flagged",
    "src/ok.rs",
    "// we deliberately avoid unsafe { } here; use safe wrappers instead.\npub fn f() -> u8 {\n    1\n}\n",
    { expect: "advisory", absent: [UNSAFE] },
  ),
  rule(
    "unsafe",
    "unsafe inside a multi-line block comment is NOT flagged",
    "src/ok.rs",
    "/*\nlegacy implementation:\nunsafe {\n    do_thing();\n}\n*/\npub fn f() -> u8 {\n    1\n}\n",
    { expect: "advisory", absent: [UNSAFE] },
  ),
  {
    name: "unsafe: an unsafe fn and an inline block comment",
    project: RUST_PROJECT,
    steps: wrote("src/u.rs", "/* unsafe { */ fn safe() {}\nunsafe fn raw() {}\n"),
    expect: "advisory",
    contains: [UNSAFE],
  },
  {
    name: "unsafe: an identifier ending in unsafe is not flagged",
    project: RUST_PROJECT,
    steps: wrote("src/u.rs", "fn f() {\n    let not_unsafe {\n    };\n}\n"),
    expect: "silent",
    absent: [UNSAFE],
  },
  rule(
    "docs",
    "pub(crate) fn without /// doc emits a docs advisory",
    "src/api.rs",
    "pub(crate) fn do_thing() -> u32 {\n    1\n}\n",
    { expect: "advisory", contains: [DOCS], absent: ["QUALITY VIOLATION"] },
  ),
  rule(
    "docs",
    "pub fn without /// doc emits a non-blocking docs advisory",
    "src/api.rs",
    "pub fn do_thing() -> u32 {\n    1\n}\n",
    { expect: "advisory", contains: [DOCS], absent: ["QUALITY VIOLATION"] },
  ),
  {
    name: "docs: attributes and blank lines sit between the doc and the item",
    project: RUST_PROJECT,
    steps: wrote(
      "src/api.rs",
      "/// Documented.\n\n#[inline]\npub fn a() {}\n//! inner\npub struct B;\npub enum C {}\n",
    ),
    expect: "advisory",
    contains: ["7: pub enum C {}"],
  },
  {
    name: "docs: more than three undocumented items show the first three",
    project: RUST_PROJECT,
    steps: wrote("src/api.rs", "pub fn a() {}\npub fn b() {}\npub const C: u8 = 1;\npub mod d;\n"),
    expect: "advisory",
    contains: ["3: pub const C: u8 = 1;"],
    absent: ["4: pub mod d;"],
  },
  {
    name: "docs: a long doc block is verbose",
    project: RUST_PROJECT,
    steps: wrote("src/api.rs", LONG_DOC),
    expect: "advisory",
    contains: ["1: doc block is 13 lines — trim to the essentials"],
  },
  {
    name: "docs: undocumented and verbose together",
    project: RUST_PROJECT,
    steps: wrote("src/api.rs", `pub fn bare() {}\n${LONG_DOC}`),
    expect: "advisory",
    contains: [DOCS, "Verbose doc comment"],
  },
  {
    name: "docs: a doc block at the end of the file",
    project: RUST_PROJECT,
    steps: wrote(
      "src/api.rs",
      `fn x() {}\n${Array.from({ length: 13 }, () => "//! tail\n").join("")}`,
    ),
    expect: "advisory",
    contains: ["2: doc block is 13 lines"],
  },
  {
    name: "docs: outside src/ there is no advisory",
    project: RUST_PROJECT,
    steps: wrote("examples/demo.rs", "pub fn bare() {}\n"),
    expect: "silent",
  },
];
