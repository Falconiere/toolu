/**
 * Golden cases (#267) for the two ast-grep rule sets: error-handling.bats and
 * no-mocks.bats, plus the ast-grep-absent, unparsable-output and excerpt-cap
 * edges.
 */
import { pathWithout, stubAstGrep } from "./cases-path.ts";
import { RUST_PROJECT, bats, wrote, type RsCase } from "./cases-types.ts";

const CRASH = "echo boom >&2\nexit 2";
const SERVICE = "trait Service {\n    fn call(&self) -> u8;\n}\n";
const AUTOMOCK = `#[cfg_attr(test, automock)]\n${SERVICE}`;
const MOCKALL_TEST =
  "use mockall::predicate::*;\n\n#[test]\nfn it_works() {\n    assert_eq!(1, 1);\n}\n";
const MOCK_BODY =
  "    pub MyMock {}\n    impl Foo for MyMock {\n        fn bar(&self) -> u8;\n    }\n";
const FOO = "pub trait Foo {\n    fn bar(&self) -> u8;\n}\n\n";
const NO_MOCKS_OFF = { lang: { rust: { noMocks: false } } };
const VIOLATION = "QUALITY VIOLATION";

type Extra = Omit<RsCase, "name" | "from" | "project" | "steps">;

const errors = (title: string, file: string, body: string, extra: Extra): RsCase => ({
  name: `error-handling: ${title}`,
  from: bats("error-handling", `rust-quality: ${title}`),
  project: RUST_PROJECT,
  steps: wrote(file, body),
  ...extra,
});

const mocks = (title: string, file: string, body: string, extra: Extra): RsCase => ({
  name: `no-mocks: ${title}`,
  from: bats("no-mocks", `rust-quality no-mocks: ${title}`),
  project: RUST_PROJECT,
  steps: wrote(file, body),
  ...extra,
});

export const ERROR_CASES: readonly RsCase[] = [
  errors(
    ".unwrap() in src/ is flagged",
    "src/bad.rs",
    "fn main() {\n    let x = some_result().unwrap();\n}\n",
    {
      expect: "advisory",
      contains: [".unwrap()"],
    },
  ),
  errors(
    ".expect() in src/ is flagged",
    "src/bad.rs",
    'fn main() {\n    let y = thing.expect("nope");\n}\n',
    {
      expect: "advisory",
      contains: [".expect()"],
    },
  ),
  errors(
    "panic!/todo!/unimplemented! in src/ is flagged",
    "src/bad.rs",
    'fn main() {\n    panic!("explode");\n    todo!();\n    unimplemented!();\n}\n',
    { expect: "advisory", contains: ["panic!/todo!/unimplemented!"] },
  ),
  errors(
    "clean Result-based code produces no error output",
    "src/good.rs",
    'fn main() -> Result<(), std::io::Error> {\n    let _ = std::fs::read_to_string("x")?;\n    Ok(())\n}\n',
    { expect: "silent", absent: [VIOLATION] },
  ),
  errors(
    "ast-grep crash is surfaced as a violation, not silent pass",
    "src/good.rs",
    "fn helper() {}\n",
    {
      env: stubAstGrep(CRASH),
      expect: "advisory",
      contains: ["ast-grep failed", "boom", "exit 2"],
    },
  ),
  errors("gate-status file is written on violation", "src/bad.rs", "fn main() { panic!(); }\n", {
    expect: "advisory",
    contains: [VIOLATION],
  }),
  errors(
    "unreachable! in src/ is flagged",
    "src/bad.rs",
    'fn f(x: u8) -> u8 {\n    match x {\n        0 => 1,\n        _ => unreachable!("never"),\n    }\n}\n',
    { expect: "advisory", contains: ["unreachable!"] },
  ),
  errors(
    ".unwrap() in a COLOCATED test under src/ is not flagged",
    "src/store/tests/migrate.rs",
    "#[test]\nfn migration_applies() {\n    let conn = open().unwrap();\n    assert_eq!(conn.version(), 14);\n}\n",
    { expect: "silent", absent: [".unwrap()"] },
  ),
  errors(
    ".unwrap() in a NON-test file under a src/.../tests/ path is still flagged",
    "src/store/tests/helper.rs",
    "pub fn open_or_die() -> Conn {\n    connect().unwrap()\n}\n",
    { expect: "advisory", contains: [".unwrap()"] },
  ),
  {
    name: "error-handling: ast-grep absent skips the rule sets",
    project: RUST_PROJECT,
    env: pathWithout("ast-grep", "sg"),
    steps: wrote("src/bad.rs", "fn main() {\n    let x = some_result().unwrap();\n}\n"),
    expect: "silent",
  },
  {
    name: "error-handling: unparsable ast-grep output is a failure",
    project: RUST_PROJECT,
    env: stubAstGrep("echo 'not json'"),
    config: NO_MOCKS_OFF,
    steps: wrote("src/good.rs", "fn helper() {}\n"),
    expect: "advisory",
    contains: ["did not parse as the documented JSON array"],
  },
  {
    name: "error-handling: empty ast-grep output is no hits",
    project: RUST_PROJECT,
    env: stubAstGrep("exit 0"),
    config: NO_MOCKS_OFF,
    steps: wrote("src/good.rs", "fn helper() {}\n"),
    expect: "silent",
  },
  {
    name: "error-handling: excerpts stop at five per rule",
    project: RUST_PROJECT,
    steps: wrote(
      "src/many.rs",
      `fn main() {\n${Array.from({ length: 6 }, (_, i) => `\tlet v${String(i + 1)} = r${String(i + 1)}().unwrap();\t\n`).join("")}}\n`,
    ),
    expect: "advisory",
    contains: ["r5().unwrap()"],
    absent: ["r6().unwrap()"],
  },
  mocks(
    "src/ fixture with #[cfg_attr(test, automock)] fails the gate",
    "src/service.rs",
    AUTOMOCK,
    {
      expect: "advisory",
      contains: ["no-mocks", VIOLATION],
    },
  ),
  mocks(
    "src/lib.rs with mock! { ... } fails the gate",
    "src/lib.rs",
    `${FOO}mock! {\n${MOCK_BODY}}\n`,
    {
      expect: "advisory",
      contains: ["no-mocks", "mock! { ... }"],
    },
  ),
  mocks(
    "src/lib.rs with mock!(...) paren-delimiter form fails the gate",
    "src/lib.rs",
    `${FOO}mock!(\n${MOCK_BODY});\n`,
    { expect: "advisory", contains: ["no-mocks", "mock! { ... } mock definition"] },
  ),
  mocks(
    "tests/integration_test.rs with use mockall::predicate::*; fails the gate",
    "tests/integration_test.rs",
    MOCKALL_TEST,
    { expect: "advisory", contains: ["no-mocks", "mockall/faux import"] },
  ),
  mocks(
    "tests/ fixture with use faux::...; fails the gate",
    "tests/faux_test.rs",
    "use faux::create;\n\n#[test]\nfn it_works() {\n    assert_eq!(1, 1);\n}\n",
    { expect: "advisory", contains: ["no-mocks", "mockall/faux import"] },
  ),
  mocks(
    "*_tests.rs basename outside tests/ is still scoped for mock imports",
    "integration_tests.rs",
    "use mockall::automock;\n\n#[test]\nfn it_works() {\n    assert_eq!(1, 1);\n}\n",
    { expect: "advisory", contains: ["no-mocks"] },
  ),
  {
    ...mocks(
      "real-data test fixture (reads a real fixture file from tests/data/) passes",
      "tests/real_data_test.rs",
      'use std::fs;\n\n#[test]\nfn reads_real_fixture() {\n    let s = fs::read_to_string("tests/data/sample.txt").expect("fixture file must exist");\n    assert_eq!(s.trim(), "hello world");\n}\n',
      { expect: "silent", absent: ["no-mocks"] },
    ),
    commit: { "tests/data/sample.txt": "hello world\n" },
  },
  mocks("lang.rust.noMocks=false lets a src/ automock fixture pass", "src/service.rs", AUTOMOCK, {
    config: NO_MOCKS_OFF,
    expect: "silent",
    absent: ["no-mocks"],
  }),
  mocks(
    "lang.rust.noMocks=false lets a tests/ mockall-import fixture pass",
    "tests/integration_test.rs",
    MOCKALL_TEST,
    { config: NO_MOCKS_OFF, expect: "silent", absent: ["no-mocks"] },
  ),
  mocks(
    "ast-grep crash while scanning for mocks is surfaced, not silent",
    "src/service.rs",
    SERVICE,
    {
      env: stubAstGrep(CRASH),
      expect: "advisory",
      contains: ["ast-grep failed while scanning", "no-mocks rules could not be verified", "boom"],
    },
  ),
  mocks("ast-grep exit 0 with empty stdout is surfaced, not silent", "src/service.rs", SERVICE, {
    env: stubAstGrep("exit 0"),
    expect: "advisory",
    contains: [
      "ast-grep failed while scanning",
      "empty output",
      "no-mocks rules could not be verified",
    ],
  }),
  {
    name: "no-mocks: a bare #[automock] in src/ fails the gate",
    project: RUST_PROJECT,
    steps: wrote("src/service.rs", `#[automock]\n${SERVICE}`),
    expect: "advisory",
    contains: ["#[automock] mock definition"],
  },
  {
    name: "no-mocks: a mock import in a non-test src/ file is not scanned",
    project: RUST_PROJECT,
    steps: wrote("src/uses.rs", "use mockall::predicate::*;\nfn helper() {}\n"),
    expect: "silent",
  },
];
