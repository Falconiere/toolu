/**
 * Golden cases (#265) for the module's flow: gating.bats, assembled.bats,
 * no-mocks.bats and the gate-entry scenarios of throw-literal.bats, plus Codex
 * patches, moves, linked worktrees, `CLAUDE_FILE_PATHS`, test layout and the
 * jscpd advisory (the repository's own real jscpd).
 */
import { symlinkSync } from "node:fs";
import { join } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { stubAstGrep } from "./cases-errors.ts";
import { pathWithout } from "./cases-path.ts";
import { ALIAS_PROJECT, TS_PROJECT, wrote, type TsCase } from "./cases-types.ts";
import { REPO_ROOT } from "./golden-harness.ts";

const MOCK = "Mocked test double";
const NESTED = "Test nested in __tests__/ subdirectory";
const DEP = { "src/dep.ts": "export const dep = 1;\n" };
const THREE =
  'import { thing } from "../other";\nexport const x = thing as Bar;\nexport function go() {\n  console.log("debug");\n}\n';

const testFile = (
  name: string,
  file: string,
  body: string,
  check: Pick<TsCase, "contains" | "absent" | "expect">,
  extra: Partial<TsCase> = {},
): TsCase => ({
  name,
  project: TS_PROJECT,
  commit: DEP,
  steps: wrote(file, body),
  ...check,
  ...extra,
});

/** The repository's real jscpd, where `bunx jscpd` looks first. */
function localJscpd(sb: Sandbox): void {
  sb.write("node_modules/.bin/.keep", "");
  symlinkSync(join(REPO_ROOT, "node_modules/.bin/jscpd"), sb.path("node_modules/.bin/jscpd"));
}

const DUPLICATED = `/** Sum. */\nexport function total(items: number[]): number {\n${Array.from({ length: 12 }, (_, i) => `  const step${String(i)} = items.length * ${String(i)} + items.length;\n`).join("")}  return items.length;\n}\n`;

export const FLOW_CASES: readonly TsCase[] = [
  {
    name: "gating: outside a TS project",
    project: {},
    steps: wrote("src/a.ts", 'console.log("x");\n'),
    expect: "silent",
  },
  {
    name: "gating: TS project with no lock file",
    project: { "tsconfig.json": "{}\n" },
    steps: wrote("foo.ts", 'console.log("x");\n'),
    expect: "silent",
  },
  {
    name: "gating: tsconfig present but untracked",
    project: { "bun.lock": "" },
    commit: {},
    setup: (sb) => sb.write("tsconfig.json", "{}\n"),
    steps: wrote("src/a.ts", 'console.log("x");\n'),
    expect: "silent",
  },
  {
    name: "gating: package manager not on PATH",
    project: { ...TS_PROJECT, "bun.lock": "", "pnpm-lock.yaml": "" },
    env: pathWithout("bun", "bunx"),
    steps: wrote("src/a.ts", 'console.log("x");\n'),
    expect: "silent",
  },
  {
    name: "DEV-1: no jq on PATH",
    project: TS_PROJECT,
    env: pathWithout("jq"),
    steps: wrote("src/a.ts", 'console.log("x");\n'),
    expect: "silent",
  },
  {
    name: "flow: a non-TS file is ignored",
    project: TS_PROJECT,
    steps: wrote("README.md", 'console.log("x");\n'),
    expect: "silent",
  },
  {
    name: "flow: a missing file is ignored",
    project: TS_PROJECT,
    steps: [{ tool: "Write", file: "src/gone.ts" }],
    expect: "silent",
  },
  {
    name: "flow: a relative path resolves against the hook's cwd",
    project: TS_PROJECT,
    steps: [{ write: { "src/a.ts": 'console.log("x");\n' }, file: "src/a.ts", relative: true }],
    expect: "advisory",
    contains: ["Forbidden console.log in src/a.ts"],
  },
  {
    name: "flow: CLAUDE_FILE_PATHS names the file for any tool",
    project: TS_PROJECT,
    steps: [
      {
        write: { "src/a.ts": 'console.log("x");\n' },
        tool: "Bash",
        input: { command: "true" },
        env: { CLAUDE_FILE_PATHS: "src/a.ts" },
      },
    ],
    expect: "advisory",
    contains: ["Forbidden console.log"],
  },
  {
    name: "flow: a file in a linked worktree is skipped",
    project: TS_PROJECT,
    setup: (sb) => {
      sb.git("worktree", "add", "-q", "-b", "side", "wt");
    },
    steps: wrote("wt/src/a.ts", 'console.log("x");\n'),
    expect: "silent",
  },
  {
    name: "assembled: three violations in fragment order",
    project: ALIAS_PROJECT,
    steps: wrote("src/bad.ts", THREE, "Edit"),
    expect: "advisory",
    contains: ["Forbidden ../ import", "Forbidden 'as' type assertion", "Forbidden console.log"],
  },
  {
    name: "assembled: fixing one of three keeps the gate failing",
    project: ALIAS_PROJECT,
    steps: [
      ...wrote("src/bad.ts", THREE, "Edit"),
      ...wrote("src/bad.ts", THREE.replace("console.log", "console.info"), "Edit"),
    ],
    expect: "advisory",
    contains: ["Forbidden ../ import"],
    absent: ["Forbidden console.log"],
  },
  {
    name: "assembled: ../ import without a @/ alias",
    project: TS_PROJECT,
    steps: wrote(
      "src/bad.ts",
      'import { thing } from "../other";\nexport const x = thing as Bar;\n',
      "Edit",
    ),
    expect: "advisory",
    contains: ["Forbidden 'as' type assertion"],
    absent: ["Forbidden ../ import"],
  },
  {
    name: "assembled: @ts-expect-error in src",
    project: ALIAS_PROJECT,
    steps: wrote(
      "src/bad.ts",
      "// @ts-expect-error legacy boundary\nexport const b = thing();\n",
      "Edit",
    ),
    expect: "advisory",
    contains: ["Forbidden suppression comment"],
  },
  {
    name: "assembled: clean file",
    project: ALIAS_PROJECT,
    steps: wrote(
      "src/good.ts",
      "/** Adds two numbers. */\nexport function add(a: number, b: number): number {\n  return a + b;\n}\n",
      "Edit",
    ),
    expect: "silent",
  },
  {
    name: "assembled: re-editing a failing file clean flips the gate",
    project: ALIAS_PROJECT,
    steps: [
      ...wrote("src/x.ts", "export function b(){ throw 42; }\n", "Edit"),
      ...wrote("src/x.ts", 'export function b(){ throw new Error("boom"); }\n', "Edit"),
    ],
    expect: "advisory",
    absent: ["QUALITY VIOLATION"],
  },
  {
    name: "gate: clearing one file keeps another's failure",
    project: TS_PROJECT,
    steps: [
      ...wrote("src/a.ts", 'console.log("a");\n', "Edit"),
      ...wrote("src/b.ts", 'console.log("b");\n', "Edit"),
      ...wrote("src/b.ts", 'console.info("b");\n', "Edit"),
      ...wrote("src/a.ts", 'console.info("a");\n', "Edit"),
    ],
    expect: "silent",
  },
  {
    name: "gate: deleting a failing file clears its entry",
    hosts: ["claude", "codex"],
    project: TS_PROJECT,
    steps: [
      ...wrote("src/bad.ts", 'console.log("bad");\n', "Edit"),
      { remove: ["src/bad.ts"], tool: "Delete", file: "src/bad.ts" },
    ],
    expect: "silent",
  },
  {
    name: "gate: a moved file clears its source entry and checks its target",
    hosts: ["codex"],
    project: TS_PROJECT,
    steps: [
      ...wrote("src/old.ts", 'console.log("bad");\n', "Edit"),
      {
        write: { "src/new.ts": 'console.info("ok");\n' },
        remove: ["src/old.ts"],
        rawPatch:
          "*** Begin Patch\n*** Update File: src/old.ts\n*** Move to: src/new.ts\n@@\n-a\n+b\n*** End Patch",
      },
    ],
    expect: "silent",
  },
  {
    name: "codex: a written file with a violation",
    hosts: ["codex"],
    project: TS_PROJECT,
    steps: wrote("src/bad.ts", 'console.log("bad");\n'),
    expect: "advisory",
    contains: ["Forbidden console.log"],
  },
  {
    name: "codex: one patch through ts-quality and rust-quality",
    hosts: ["codex"],
    project: { ...TS_PROJECT, "Cargo.toml": '[package]\nname = "fixture"\nversion = "0.1.0"\n' },
    register: ["rust-quality"],
    steps: [
      {
        write: {
          "src/bad.ts": 'console.log("bad");\n',
          "src/bad.rs": "#[allow(dead_code)]\nfn bad() {}\n",
        },
        patch: [
          { op: "update", path: "src/bad.ts", lines: ["-a", "+b"] },
          { op: "update", path: "src/bad.rs", lines: ["-a", "+b"] },
        ],
      },
    ],
    expect: "advisory",
    contains: ["Forbidden console.log", "Forbidden lint suppression"],
  },
  testFile("tests: a test outside __tests__", "src/foo.test.ts", 'test("x", () => {});\n', {
    expect: "advisory",
    contains: ["Test file outside __tests__/"],
  }),
  testFile(
    "tests: e2e specs keep their own layout",
    "src/app/e2e/login.spec.ts",
    'test("x", () => {});\n',
    { expect: "silent", absent: ["Test file outside"] },
  ),
  testFile(
    "tests: __tests__ nested in __tests__ is not co-located",
    "src/__tests__/__tests__/a.test.ts",
    'test("x", () => {});\n',
    { expect: "advisory", contains: ["Test not co-located with source"] },
  ),
  testFile(
    "tests: helpers and utils subdirectories are allowed",
    "src/__tests__/helpers/a.test.ts",
    'test("x", () => {});\n',
    { expect: "silent", absent: [NESTED] },
  ),
  testFile(
    "no-mocks: vi.mock",
    "src/__tests__/foo.test.ts",
    'import { dep } from "../dep";\nvi.mock("../dep");\ntest("uses dep", () => {\n  expect(dep).toBeDefined();\n});\n',
    { expect: "advisory", contains: [MOCK, "QUALITY VIOLATION"] },
  ),
  testFile(
    "no-mocks: jest.fn",
    "src/__tests__/foo.test.ts",
    'const handler = jest.fn();\ntest("calls handler", () => {\n  handler();\n  expect(handler).toBeDefined();\n});\n',
    { expect: "advisory", contains: [MOCK] },
  ),
  {
    ...testFile(
      "no-mocks: one hit per mock rule",
      "src/__tests__/foo.test.ts",
      'jest.mock("a");\nvi.fn();\nsinon.spy();\njest.fn();\nvi.mock("b");\n',
      { expect: "advisory", contains: [MOCK] },
    ),
    unordered: true,
  },
  {
    ...testFile(
      "no-mocks: a real JSON fixture passes",
      "src/__tests__/foo.test.ts",
      'import { readFileSync } from "node:fs";\nimport { join } from "node:path";\n\ntest("loads a real user fixture", () => {\n  const raw = readFileSync(join(__dirname, "fixtures/user.json"), "utf8");\n  expect(JSON.parse(raw).name).toBe("Ada");\n});\n',
      { expect: "silent", absent: [MOCK] },
    ),
    commit: { ...DEP, "src/__tests__/fixtures/user.json": '{"name":"Ada"}\n' },
  },
  testFile(
    "no-mocks: ts-mockito import",
    "src/__tests__/foo.test.ts",
    'import { mock, instance } from "ts-mockito";\n\ntest("uses ts-mockito", () => {\n  expect(instance(mock<{ x: number }>())).toBeDefined();\n});\n',
    { expect: "advisory", contains: ["Import from ts-mockito"] },
  ),
  testFile(
    "no-mocks: lang.ts.noMocks=false",
    "src/__tests__/foo.test.ts",
    'vi.mock("../dep");\ntest("x", () => {});\n',
    { expect: "silent", absent: [MOCK] },
    { config: { lang: { ts: { noMocks: false } } } },
  ),
  testFile(
    "no-mocks: __tests__/mocks is a nested test dir",
    "src/__tests__/mocks/handlers.test.ts",
    'test("noop", () => {\n  expect(true).toBe(true);\n});\n',
    { expect: "advisory", contains: [NESTED] },
  ),
  testFile(
    "no-mocks: __tests__/fixtures is allowed",
    "src/__tests__/fixtures/handlers.test.ts",
    'test("noop", () => {\n  expect(true).toBe(true);\n});\n',
    { expect: "silent", absent: [NESTED] },
  ),
  testFile(
    "no-mocks: e2e specs are exempt",
    "src/app/e2e/login.spec.ts",
    'vi.mock("../session");\ntest("logs in", () => {\n  expect(true).toBe(true);\n});\n',
    { expect: "silent", absent: [MOCK] },
  ),
  testFile(
    "no-mocks: sinon.stub",
    "src/__tests__/foo.test.ts",
    'import { dep } from "../dep";\nconst stub = sinon.stub(dep, "toString");\ntest("uses stub", () => {\n  expect(stub).toBeDefined();\n});\n',
    { expect: "advisory", contains: [MOCK] },
  ),
  testFile(
    "no-mocks: ast-grep crash",
    "src/__tests__/foo.test.ts",
    'test("clean", () => {\n  expect(true).toBe(true);\n});\n',
    { expect: "advisory", contains: ["ast-grep failed while scanning", "boom"] },
    { env: stubAstGrep("echo boom >&2\nexit 2") },
  ),
  testFile(
    "no-mocks: ast-grep exit 0 with empty output",
    "src/__tests__/foo.test.ts",
    'test("clean", () => {\n  expect(true).toBe(true);\n});\n',
    { expect: "advisory", contains: ["ast-grep failed while scanning", "empty output"] },
    { env: stubAstGrep("exit 0") },
  ),
  testFile(
    "no-mocks: ast-grep exit 0 with non-JSON output",
    "src/__tests__/foo.test.ts",
    'test("clean", () => {});\n',
    { expect: "advisory", contains: ["did not parse as the documented JSON array"] },
    { env: stubAstGrep("echo nope") },
  ),
  testFile(
    "no-mocks: ast-grep absent still checks ts-mockito",
    "src/__tests__/foo.test.ts",
    'import { mock } from "ts-mockito";\nvi.mock("x");\n',
    { expect: "advisory", contains: ["Import from ts-mockito"], absent: [MOCK] },
    { env: pathWithout("ast-grep", "sg") },
  ),
  {
    name: "duplication: jscpd finds a clone in the package",
    project: {
      ...TS_PROJECT,
      ".jscpd.json": '{"threshold":0,"minLines":5,"reporters":["console"]}\n',
      "packages/core/src/one.ts": DUPLICATED,
    },
    setup: localJscpd,
    steps: wrote("packages/core/src/two.ts", DUPLICATED),
    expect: "advisory",
    contains: ["Code duplication detected involving"],
  },
  {
    name: "duplication: a file name holding $ still matches jscpd's report",
    project: {
      ...TS_PROJECT,
      ".jscpd.json": '{"threshold":0,"minLines":5,"reporters":["console"]}\n',
      "packages/core/src/one.ts": DUPLICATED,
    },
    setup: localJscpd,
    steps: wrote("packages/core/src/$id.ts", DUPLICATED),
    expect: "advisory",
    contains: ["Code duplication detected involving"],
  },
  {
    name: "duplication: not under apps/ or packages/",
    project: {
      ...TS_PROJECT,
      ".jscpd.json": '{"threshold":0,"minLines":5,"reporters":["console"]}\n',
      "src/one.ts": DUPLICATED,
    },
    setup: localJscpd,
    steps: wrote("src/two.ts", DUPLICATED),
    expect: "silent",
  },
];
