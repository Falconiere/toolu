/**
 * Golden cases (#266) for the module's flow: dispatch.bats and assembled.bats,
 * plus tool and path handling, the project and tool checks, linked worktrees
 * and Codex patches, deletes and moves.
 */
import { pathWithout } from "./cases-path.ts";
import { PY_PROJECT, assignments, pyCase, wrote, type PyCase } from "./cases-types.ts";

const BARE = "try:\n    pass\nexcept:\n    pass\n";
const SCOPED = "try:\n    pass\nexcept ValueError:\n    pass\n";
const SUPPRESSED = "Forbidden suppression";

const edit = (file: string, body: string) => wrote(file, body, "Edit");

/** assembled.bats' 2-violation fixture: over a 5-line limit, with a bare except. */
const TWO = `def big():\n    try:\n        pass\n    except:\n${assignments(1, 6, "        ")}`;

/** A legacy single-slot failure another hook recorded (dispatch.bats). */
const RUST_GATE = `${JSON.stringify(
  {
    status: "failing",
    reason: "Rust violation",
    source: "rust-quality-hook",
    file: "/p/x.rs",
    violations: "bad rust\n",
    updatedAt: "2026-01-01T00:00:00Z",
  },
  null,
  2,
)}\n`;

const DISPATCH: readonly PyCase[] = [
  {
    name: "dispatch: no-op outside a Python project",
    project: {},
    steps: wrote("main.py", BARE),
    expect: "silent",
  },
  pyCase("dispatch: Edit extracts the file path", edit("bad.py", BARE), {
    expect: "advisory",
    contains: [SUPPRESSED],
  }),
  pyCase("dispatch: MultiEdit extracts the file path", wrote("bad.py", BARE, "MultiEdit"), {
    expect: "advisory",
    contains: [SUPPRESSED],
  }),
  pyCase(
    "dispatch: re-editing a failing file clean clears its entry",
    [...edit("bad.py", BARE), ...edit("bad.py", SCOPED)],
    { expect: "silent" },
  ),
  pyCase(
    "dispatch: deleting a failing file clears its entry",
    [...edit("bad.py", BARE), { remove: ["bad.py"], tool: "Delete", file: "bad.py" }],
    { expect: "silent" },
    { hosts: ["claude", "codex"] },
  ),
  pyCase(
    "dispatch: clearing one file keeps another's failure",
    [...edit("a.py", BARE), ...edit("b.py", BARE), ...edit("b.py", SCOPED), ...edit("a.py", SCOPED)],
    { expect: "silent" },
  ),
  pyCase(
    "dispatch: another hook's failure survives a python fail and clear",
    [...edit("a.py", BARE), ...edit("a.py", SCOPED)],
    { expect: "silent" },
    { setup: (sb) => sb.write(".claude/tmp/quality-gate-status.json", RUST_GATE) },
  ),
];

const ASSEMBLED: readonly PyCase[] = [
  pyCase(
    "assembled: two violations in fragment order",
    wrote("bad.py", TWO),
    { expect: "advisory", contains: ["exceeds 5-line limit", "bare except"] },
    { config: { lang: { python: { maxFileLines: 5 } } } },
  ),
  pyCase(
    "assembled: fixing one of two keeps the gate failing",
    [
      ...wrote("bad.py", `def big():\n    except_marker = 1\n${assignments(1, 6, "    ")}    # noqa\n`),
      ...wrote("bad.py", `def big():\n    except_marker = 1\n${assignments(1, 6, "    ")}`),
    ],
    { expect: "advisory", contains: ["exceeds 5-line limit"], absent: ["blanket # noqa"] },
    { config: { lang: { python: { maxFileLines: 5 } } } },
  ),
  pyCase(
    "assembled: clean file",
    wrote("good.py", 'def read_it():\n    """Read and return a constant."""\n    return 1\n'),
    { expect: "silent" },
  ),
  pyCase(
    "assembled: re-editing a failing file clean flips the gate",
    [
      ...edit(
        "x.py",
        "def helper():\n    except_bad = 1\n    return except_bad\n\n\ntry:\n    pass\nexcept:\n    pass\n",
      ),
      ...edit("x.py", 'def helper():\n    """Do a thing."""\n    return 1\n'),
    ],
    { expect: "silent" },
  ),
];

const FLOW: readonly PyCase[] = [
  pyCase("flow: a non-.py file is ignored", wrote("README.md", BARE), { expect: "silent" }),
  pyCase("flow: a missing file is ignored", [{ tool: "Write", file: "gone.py" }], {
    expect: "silent",
  }),
  pyCase(
    "flow: a relative path resolves against the hook's cwd",
    [{ write: { "src/a.py": BARE }, file: "src/a.py", relative: true }],
    { expect: "advisory", contains: [`${SUPPRESSED} in src/a.py`] },
  ),
  pyCase(
    "flow: CLAUDE_FILE_PATHS names the file for any tool",
    [
      {
        write: { "src/a.py": BARE },
        tool: "Bash",
        input: { command: "true" },
        env: { CLAUDE_FILE_PATHS: "src/a.py" },
      },
    ],
    { expect: "advisory", contains: [SUPPRESSED] },
  ),
  pyCase(
    "flow: a Bash call without CLAUDE_FILE_PATHS is ignored",
    [{ write: { "src/a.py": BARE }, tool: "Bash", file: "src/a.py", input: { command: "true" } }],
    { expect: "silent" },
  ),
  pyCase("flow: python3 not on PATH", wrote("bad.py", BARE), { expect: "silent" }, {
    env: pathWithout("python3"),
  }),
  pyCase("DEV-1: no jq on PATH", wrote("bad.py", BARE), { expect: "silent" }, {
    env: pathWithout("jq"),
  }),
  pyCase(
    "flow: malformed thresholds fall back to the defaults",
    wrote("big.py", assignments(1, 401)),
    { expect: "advisory", contains: ["exceeds 400-line limit"] },
    { config: { lang: { python: { maxFileLines: "abc", maxFnLines: 0 } } } },
  ),
  pyCase(
    "flow: a numeric-string threshold is honoured",
    wrote("big.py", assignments(1, 11)),
    { expect: "advisory", contains: ["exceeds 10-line limit"] },
    { config: { lang: { python: { maxFileLines: "10" } } } },
  ),
  pyCase(
    "flow: a file in a linked worktree is checked",
    wrote("wt/bad.py", BARE),
    { expect: "advisory", contains: [SUPPRESSED] },
    { setup: (sb) => sb.git("worktree", "add", "-q", "-b", "side", "wt") },
  ),
  pyCase("codex: a written file with a violation", wrote("bad.py", BARE), {
    expect: "advisory",
    contains: [SUPPRESSED],
  }, { hosts: ["codex"] }),
  pyCase(
    "codex: a moved file clears its source entry and checks its target",
    [
      ...edit("old.py", BARE),
      {
        write: { "new.py": SCOPED },
        remove: ["old.py"],
        rawPatch:
          "*** Begin Patch\n*** Update File: old.py\n*** Move to: new.py\n@@\n-a\n+b\n*** End Patch",
      },
    ],
    { expect: "silent" },
    { hosts: ["codex"] },
  ),
  {
    name: "codex: one patch through python-quality and rust-quality",
    hosts: ["codex"],
    project: { ...PY_PROJECT, "Cargo.toml": '[package]\nname = "fixture"\nversion = "0.1.0"\n' },
    register: ["rust-quality"],
    steps: [
      {
        write: {
          "tests/test_bad.py": "from unittest.mock import patch\n\n\ndef test_x():\n    assert patch\n",
          "src/bad.rs": "#[allow(dead_code)]\nfn bad() {}\n",
        },
        patch: [
          { op: "update", path: "tests/test_bad.py", lines: ["-a", "+b"] },
          { op: "update", path: "src/bad.rs", lines: ["-a", "+b"] },
        ],
      },
    ],
    expect: "advisory",
    contains: ["no-mocks: mock import", "Forbidden lint suppression"],
  },
];

export const FLOW_CASES: readonly PyCase[] = [...DISPATCH, ...ASSEMBLED, ...FLOW];
