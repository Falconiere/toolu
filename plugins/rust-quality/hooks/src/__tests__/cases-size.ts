/** Golden cases (#267) for file, function and impl size: size.bats, plus the unreadable-count and default-limit edges. */
import { RUST_PROJECT, bats, constLines, wrote, type RsCase } from "./cases-types.ts";

const FN_LONG = "Function too long";
const IMPL_LONG = "Impl block too large";

type Limits = { maxFileLines?: number; maxFnLines?: number; maxImplLines?: number };

const size = (
  title: string,
  limits: Limits,
  file: string,
  body: string,
  check: Pick<RsCase, "contains" | "absent">,
  expect: RsCase["expect"] = "advisory",
): RsCase => ({
  name: `size: ${title}`,
  from: bats("size", `rust-quality: ${title}`),
  project: RUST_PROJECT,
  config: { lang: { rust: limits } },
  steps: wrote(file, body),
  expect,
  ...check,
});

const LONG_BRANCHES =
  "fn long_with_branches(x: u8) -> u8 {\n    let mut acc = 0;\n    if x > 0 {\n        acc += 1;\n    } else {\n        acc += 2;\n    }\n    acc += 3;\n    acc += 4;\n    acc\n}\n";

export const SIZE_CASES: readonly RsCase[] = [
  size(
    "project config lowers maxFileLines, flags a file the default would not",
    { maxFileLines: 10 },
    "src/big.rs",
    constLines(15),
    { contains: ["exceeds 10-line limit"] },
  ),
  size(
    "over-limit message flags approximate size on unterminated /*",
    { maxFileLines: 2 },
    "src/m.rs",
    'let s = "/*";\nlet a = 1;\nlet b = 2;\nlet c = 3;\n',
    { contains: ["exceeds 2-line limit", "size approximated"] },
  ),
  size(
    "pub(crate) const fn is subject to the fn-length limit",
    { maxFnLines: 3 },
    "src/m.rs",
    "pub(crate) const fn big() -> u8 {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    a + b + c\n}\n",
    { contains: [FN_LONG] },
  ),
  size(
    "long fn with inner if/else is measured to its real close (regression)",
    { maxFnLines: 5 },
    "src/long.rs",
    LONG_BRANCHES,
    { contains: [FN_LONG] },
  ),
  size(
    "short impl method followed by other items is NOT flagged",
    { maxFnLines: 5 },
    "src/m.rs",
    "pub struct Foo;\nimpl Foo {\n    fn short(&self) -> u8 {\n        1\n    }\n    const A: u8 = 1;\n    const B: u8 = 2;\n    const C: u8 = 3;\n    const D: u8 = 4;\n    const E: u8 = 5;\n    const F: u8 = 6;\n}\n",
    { absent: [FN_LONG] },
  ),
  size(
    "long method inside an impl IS flagged",
    { maxFnLines: 5 },
    "src/m.rs",
    "pub struct Foo;\nimpl Foo {\n    fn long_method(&self, x: u8) -> u8 {\n        let mut acc = 0;\n        if x > 0 {\n            acc += 1;\n        } else {\n            acc += 2;\n        }\n        acc += 3;\n        acc\n    }\n}\n",
    { contains: [FN_LONG] },
  ),
  size(
    "long fn with unbalanced brace in a string is still measured",
    { maxFnLines: 5 },
    "src/m.rs",
    "fn unbalanced_string(x: u8) -> u8 {\n    let open = \"{\";\n    let close = '{';\n    if x > 0 {\n        return 1;\n    }\n    let a = 2;\n    let b = 3;\n    a + b\n}\n",
    { contains: [FN_LONG] },
  ),
  size(
    "pub(in path) fn is subject to the fn-length limit",
    { maxFnLines: 3 },
    "src/m.rs",
    "pub(in crate::foo) fn big() -> u8 {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    a + b + c\n}\n",
    { contains: [FN_LONG] },
  ),
  size(
    "oversized impl block is flagged (brace-depth)",
    { maxImplLines: 6 },
    "src/m.rs",
    `pub struct Foo;\nimpl Foo {\n${Array.from({ length: 8 }, (_, i) => `    pub const V${String(i + 1)}: u8 = ${String(i + 1)};\n`).join("")}}\n`,
    { contains: [IMPL_LONG] },
  ),
  size(
    "small impl with an inner-control-flow method is NOT flagged",
    { maxImplLines: 20 },
    "src/m.rs",
    "pub struct Foo;\nimpl Foo {\n    fn pick(&self, x: u8) -> u8 {\n        if x > 0 {\n            1\n        } else {\n            2\n        }\n    }\n}\n",
    { absent: [IMPL_LONG] },
  ),
  size(
    "long fn in a COLOCATED test under src/ is NOT flagged",
    { maxFnLines: 3 },
    "src/store/tests/big.rs",
    "#[test]\nfn table_driven() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    assert_eq!(a + b + c, 6);\n}\n",
    { absent: ["Function too long in"] },
    "silent",
  ),
  size(
    "oversized impl in a COLOCATED test under src/ is NOT flagged",
    { maxImplLines: 6 },
    "src/store/tests/big.rs",
    `#[test]\nfn anchor() { assert!(true); }\nstruct Fixture;\nimpl Fixture {\n${Array.from({ length: 8 }, (_, i) => `    const V${String(i + 1)}: u8 = ${String(i + 1)};\n`).join("")}}\n`,
    { absent: ["Impl block too large in"] },
    "silent",
  ),
  {
    name: "size: default limits leave a small file alone",
    project: RUST_PROJECT,
    steps: wrote("src/small.rs", LONG_BRANCHES),
    expect: "silent",
  },
  {
    name: "size: a trait fn declaration ends at its semicolon",
    project: RUST_PROJECT,
    config: { lang: { rust: { maxFnLines: 3 } } },
    steps: wrote(
      "src/t.rs",
      "trait T {\n    fn a(&self) -> u8;\n    fn b(&self) -> u8;\n    fn c(&self) -> u8;\n    fn d(&self) -> u8;\n    fn e(&self) -> u8;\n}\n",
    ),
    expect: "silent",
  },
  {
    name: "size: async unsafe extern fn heads are measured",
    project: RUST_PROJECT,
    config: { lang: { rust: { maxFnLines: 2 } } },
    steps: wrote(
      "src/x.rs",
      'pub async fn a() {\n    one();\n    two();\n    three();\n}\npub unsafe extern "C" fn b() {\n    one();\n}\n',
    ),
    expect: "advisory",
    contains: [FN_LONG],
  },
  {
    name: "size: an unsafe impl block is measured",
    project: RUST_PROJECT,
    config: { lang: { rust: { maxImplLines: 2 } } },
    steps: wrote(
      "src/x.rs",
      "struct S;\nunsafe impl Send for S {\n    // one\n    // two\n}\nimpl<T> From<T> for S {\n    fn from(_: T) -> S { S }\n}\n",
    ),
    expect: "advisory",
    contains: [IMPL_LONG],
  },
];
