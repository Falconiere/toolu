/**
 * Golden cases (#267) for the module's flow: dispatch.bats and assembled.bats,
 * plus gating, Codex patches, deletes and moves, `CLAUDE_FILE_PATHS`, linked
 * worktrees, the clippy hint and the unsafe exemption list.
 */
import { join } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { pathWithout } from "./cases-path.ts";
import { RUST_PROJECT, bats, wrote, type RsCase } from "./cases-types.ts";

const ALLOW = "#[allow(dead_code)]\nfn helper() {}\n";
const CLEAN = "fn helper() {}\n";
const SUPPRESSION = "Forbidden lint suppression";
const LEGACY_TS_GATE = `${JSON.stringify(
  {
    status: "failing",
    reason: "TS violation",
    source: "ts-quality-hook",
    file: "/p/x.ts",
    violations: "bad ts\n",
    updatedAt: "2026-01-01T00:00:00Z",
  },
  null,
  2,
)}\n`;

/** `fn main` over the limit with an `.unwrap()`: the assembled.bats two-violation fixture. */
const TWO_VIOLATIONS = `fn main() {\n    let x = thing().unwrap();\n${Array.from({ length: 10 }, (_, i) => `    let v${String(i + 1)} = ${String(i + 1)};\n`).join("")}}\n`;
const ONE_VIOLATION = TWO_VIOLATIONS.replace("    let x = thing().unwrap();\n", "");

const INLINE_CFG_TEST =
  "pub fn add(a: u8, b: u8) -> u8 {\n    a + b\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn it_adds() {\n        assert_eq!(super::add(1, 2), 3);\n    }\n}\n";

const UNSAFE = "pub fn f() {\n    unsafe {\n        do_thing();\n    }\n}\n";

/** A settings directory holding `rust-unsafe-exemptions.txt` with `lines`. */
function exemptions(lines: string): (sb: Sandbox) => { TOOLU_SETTINGS_DIR: string } {
  return (sb) => {
    sb.write("settings/rust-unsafe-exemptions.txt", lines);
    return { TOOLU_SETTINGS_DIR: join(sb.project, "settings") };
  };
}

export const FLOW_CASES: readonly RsCase[] = [
  {
    name: "gating: outside a Rust project",
    from: bats("dispatch", "rust-quality: no-op outside a Rust project (no Cargo.toml)"),
    project: {},
    steps: wrote("src/bad.rs", ALLOW),
    expect: "silent",
  },
  {
    name: "gating: cargo not on PATH",
    project: RUST_PROJECT,
    env: pathWithout("cargo"),
    steps: wrote("src/bad.rs", ALLOW),
    expect: "silent",
  },
  {
    name: "DEV-1: no jq on PATH",
    project: RUST_PROJECT,
    env: pathWithout("jq"),
    steps: wrote("src/bad.rs", ALLOW),
    expect: "silent",
  },
  {
    name: "flow: a non-Rust file is ignored",
    project: RUST_PROJECT,
    steps: wrote("src/notes.txt", ALLOW),
    expect: "silent",
  },
  {
    name: "flow: a missing file is ignored",
    project: RUST_PROJECT,
    steps: [{ tool: "Write", file: "src/gone.rs" }],
    expect: "silent",
  },
  {
    name: "flow: Edit names the file",
    from: bats(
      "dispatch",
      "rust-quality: Edit tool extracts file path and flags violations (regression)",
    ),
    project: RUST_PROJECT,
    steps: wrote("src/bad.rs", ALLOW, "Edit"),
    expect: "advisory",
    contains: [SUPPRESSION],
  },
  {
    name: "flow: MultiEdit names the file",
    from: bats(
      "dispatch",
      "rust-quality: MultiEdit extracts file path and flags violations (regression)",
    ),
    project: RUST_PROJECT,
    steps: wrote("src/bad.rs", ALLOW, "MultiEdit"),
    expect: "advisory",
    contains: [SUPPRESSION],
  },
  {
    name: "flow: a relative path resolves against the hook's cwd",
    project: RUST_PROJECT,
    steps: [{ write: { "src/bad.rs": ALLOW }, file: "src/bad.rs", relative: true }],
    expect: "advisory",
    contains: [`${SUPPRESSION} (#[allow]/#[expect]/cfg_attr allow) in src/bad.rs`],
  },
  {
    name: "flow: CLAUDE_FILE_PATHS names the file for any tool",
    project: RUST_PROJECT,
    steps: [
      {
        write: { "src/bad.rs": ALLOW },
        tool: "Bash",
        input: { command: "true" },
        env: { CLAUDE_FILE_PATHS: "src/bad.rs" },
      },
    ],
    expect: "advisory",
    contains: [SUPPRESSION],
  },
  {
    name: "flow: a file in a linked worktree is still checked",
    project: RUST_PROJECT,
    setup: (sb) => {
      sb.git("worktree", "add", "-q", "-b", "side", "wt");
    },
    steps: wrote("wt/src/bad.rs", ALLOW),
    expect: "advisory",
    contains: [SUPPRESSION],
  },
  {
    name: "gate: re-editing a failing file clean clears it",
    from: [
      ...bats("dispatch", "rust-quality: gate is cleared when the failing file is re-edited clean"),
      ...bats(
        "assembled",
        "rust assembled: re-editing a failing file clean flips the gate to passing",
      ),
    ],
    project: RUST_PROJECT,
    steps: [...wrote("src/x.rs", ALLOW, "Edit"), ...wrote("src/x.rs", CLEAN, "Edit")],
    expect: "silent",
  },
  {
    name: "gate: deleting a failing file clears its entry",
    from: bats("dispatch", "rust-quality: deleting a failing file clears its gate entry"),
    hosts: ["claude", "codex"],
    project: RUST_PROJECT,
    steps: [
      ...wrote("src/bad.rs", ALLOW, "Edit"),
      { remove: ["src/bad.rs"], tool: "Delete", file: "src/bad.rs" },
    ],
    expect: "silent",
  },
  {
    name: "gate: clearing one file keeps another's failure",
    from: bats(
      "dispatch",
      "rust-quality: clearing one file does not clobber another file's failure",
    ),
    project: RUST_PROJECT,
    steps: [
      ...wrote("src/a.rs", "#[allow(dead_code)]\nfn a() {}\n", "Edit"),
      ...wrote("src/b.rs", "#[allow(dead_code)]\nfn b() {}\n", "Edit"),
      ...wrote("src/b.rs", "fn b() {}\n", "Edit"),
      ...wrote("src/a.rs", "fn a() {}\n", "Edit"),
    ],
    expect: "silent",
  },
  {
    name: "gate: clearing one file keeps another's failure (midway)",
    project: RUST_PROJECT,
    steps: [
      ...wrote("src/a.rs", "#[allow(dead_code)]\nfn a() {}\n", "Edit"),
      ...wrote("src/b.rs", "#[allow(dead_code)]\nfn b() {}\n", "Edit"),
      ...wrote("src/b.rs", "fn b() {}\n", "Edit"),
    ],
    expect: "silent",
  },
  {
    name: "gate: another hook's failure survives a rust fail and clear",
    from: bats(
      "dispatch",
      "rust-quality: failing gate owned by another hook survives a rust fail->clear cycle",
    ),
    project: RUST_PROJECT,
    setup: (sb) => sb.write(".claude/tmp/quality-gate-status.json", LEGACY_TS_GATE),
    steps: [
      ...wrote("src/a.rs", "#[allow(dead_code)]\nfn a() {}\n", "Edit"),
      ...wrote("src/a.rs", "fn a() {}\n", "Edit"),
    ],
    expect: "silent",
  },
  {
    name: "gate: a moved file clears its source entry and checks its target",
    hosts: ["codex"],
    project: RUST_PROJECT,
    steps: [
      ...wrote("src/old.rs", ALLOW, "Edit"),
      {
        write: { "src/new.rs": CLEAN },
        remove: ["src/old.rs"],
        rawPatch:
          "*** Begin Patch\n*** Update File: src/old.rs\n*** Move to: src/new.rs\n@@\n-a\n+b\n*** End Patch",
      },
    ],
    expect: "silent",
  },
  {
    name: "codex: a written file with a violation",
    hosts: ["codex"],
    project: RUST_PROJECT,
    steps: wrote("src/bad.rs", ALLOW),
    expect: "advisory",
    contains: [SUPPRESSION],
  },
  {
    name: "codex: one patch over two Rust files",
    hosts: ["codex"],
    project: RUST_PROJECT,
    steps: [
      {
        write: { "src/a.rs": ALLOW, "src/b.rs": UNSAFE },
        patch: [
          { op: "update", path: "src/a.rs", lines: ["-a", "+b"] },
          { op: "update", path: "src/b.rs", lines: ["-a", "+b"] },
        ],
      },
    ],
    expect: "advisory",
    contains: [SUPPRESSION, "Forbidden unsafe code"],
  },
  {
    name: "assembled: two violations in rule order",
    from: bats("assembled", "rust assembled == monolith on a 2-violation fixture (oracle diff)"),
    project: RUST_PROJECT,
    config: { lang: { rust: { maxFileLines: 5 } } },
    steps: wrote("src/bad.rs", TWO_VIOLATIONS),
    expect: "advisory",
    contains: ["exceeds 5-line limit", ".unwrap()"],
  },
  {
    name: "assembled: fixing one of two keeps the gate failing",
    from: bats("assembled", "rust assembled keeps gate failing after fixing one of two violations"),
    project: RUST_PROJECT,
    config: { lang: { rust: { maxFileLines: 5 } } },
    steps: [...wrote("src/bad.rs", TWO_VIOLATIONS), ...wrote("src/bad.rs", ONE_VIOLATION)],
    expect: "advisory",
    contains: ["exceeds 5-line limit"],
    absent: [".unwrap()"],
  },
  {
    name: "assembled: inline cfg(test) mod raises no placement violation",
    from: bats(
      "assembled",
      "rust assembled: inline #[cfg(test)] mod produces no test-placement violation",
    ),
    project: RUST_PROJECT,
    steps: wrote("src/lib.rs", INLINE_CFG_TEST),
    expect: "advisory",
    absent: ["test file outside tests/"],
  },
  {
    name: "assembled: clean file writes no failure",
    from: bats(
      "assembled",
      "rust assembled: clean file writes no failure (gate passing) and == monolith",
    ),
    project: RUST_PROJECT,
    steps: wrote(
      "src/good.rs",
      '/// Reads a file and returns its contents.\npub fn read_it() -> Result<String, std::io::Error> {\n    let s = std::fs::read_to_string("x")?;\n    Ok(s)\n}\n',
    ),
    expect: "silent",
  },
  {
    name: "assembled: every rule family in order",
    project: RUST_PROJECT,
    config: { lang: { rust: { maxFileLines: 5, maxFnLines: 3, maxImplLines: 4 } } },
    steps: wrote(
      "src/all.rs",
      '#[allow(dead_code)]\n#[cfg_attr(test, automock)]\ntrait Service {\n    fn call(&self) -> u8;\n}\npub struct S;\nimpl S {\n    pub fn go(&self) -> u8 {\n        let a = thing().unwrap();\n        let b = other().expect("b");\n        unsafe { raw() };\n        if a > b { panic!("x") }\n        todo!()\n    }\n}\n',
    ),
    expect: "advisory",
    contains: [
      "exceeds 5-line limit",
      SUPPRESSION,
      "Forbidden unsafe code",
      "Impl block too large",
    ],
  },
  {
    name: "size: a clippy config names clippy in the hint",
    project: { ...RUST_PROJECT, "clippy.toml": "" },
    config: { lang: { rust: { maxFileLines: 3 } } },
    steps: wrote(
      "src/big.rs",
      "pub const A: u8 = 1;\npub const B: u8 = 2;\npub const C: u8 = 3;\npub const D: u8 = 4;\n",
    ),
    expect: "advisory",
    contains: ["(clippy enforces complexity here)"],
  },
  {
    name: "unsafe: a crate on the exemption list is not flagged",
    project: RUST_PROJECT,
    env: exemptions("# FFI crates\n\nffi_sys\n"),
    steps: wrote("src/ffi_sys/lib.rs", UNSAFE),
    expect: "advisory",
    absent: ["Forbidden unsafe code"],
  },
  {
    name: "unsafe: a crate missing from the exemption list is flagged",
    project: RUST_PROJECT,
    env: exemptions("# FFI crates\nother_sys\n"),
    steps: wrote("src/ffi_sys/lib.rs", UNSAFE),
    expect: "advisory",
    contains: ["Forbidden unsafe code"],
  },
];
