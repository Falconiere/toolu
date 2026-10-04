/** Pinned-host proof of selected Rust quality checks on native OpenCode tools (#354). */
import { join } from "node:path";
import { runHost, toolStates } from "./host-run.ts";
import { finalMessages, type ScenarioContext } from "./scenario.ts";
import { session, SMOKE_RUN_TIMEOUT_MS, type PretoolScenario } from "./pretool-shared.ts";
import { disabledQuality, qualityGate, qualityProject } from "./quality-smoke-shared.ts";
import { prepareSdk, verdict } from "./scenarios-posttool-smoke.ts";

const FILES = {
  "Cargo.toml": '[package]\nname = "rust-quality-smoke"\nversion = "0.1.0"\nedition = "2021"\n',
};
const PROJECT = qualityProject(["rust-quality"], FILES);
const UNWRAP = '//! Loader.\n\n/// Load a value.\npub fn load() -> u32 {\n    "1".parse().unwrap()\n}\n';
const EXPECT = UNWRAP.replace(".unwrap()", '.expect("number")');
const SUPPRESSED = "#[allow(dead_code)]\nfn unused() {}\n";
const MOCKED =
  "use mockall::predicate;\n\n#[test]\nfn it_works() {\n    assert!(predicate::eq(1).eval(&1));\n}\n";
const CLEAN = "//! Clean.\n\n/// Answer.\npub fn answer() -> u32 {\n    42\n}\n";
const NOTES = '"1".parse().unwrap()\n';

/** `text` as `+` lines of an `*** Add File` section. */
function added(text: string): string[] {
  return text
    .trimEnd()
    .split("\n")
    .map((line) => `+${line}`);
}

async function editQuality(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: PROJECT,
    scripts: (project) => ({
      "rsquality.edit": [
        { tool: "write", args: { filePath: join(project, "bad.rs"), content: SUPPRESSED } },
        {
          tool: "bash",
          args: {
            command: "touch commit-marker && git commit --allow-empty -m 'fix: invalid'",
            description: "commit after invalid Rust edit",
          },
        },
        {
          tool: "bash",
          args: {
            command: "touch push-marker && git push origin main",
            description: "push after invalid Rust edit",
          },
        },
        {
          tool: "edit",
          args: { filePath: join(project, "bad.rs"), oldString: SUPPRESSED, newString: CLEAN },
        },
        { tool: "write", args: { filePath: join(project, "notes.md"), content: SUPPRESSED } },
      ],
    }),
  });
  await prepareSdk(s, ctx.cacheRoot);
  const host = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:rsquality.edit"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(host);
  const messages = finalMessages(s, "tool");
  const deniedBash = states.filter((state) => state.tool === "bash" && state.status === "error");
  const observed = {
    writeCompleted: states.some((state) => state.tool === "write" && state.status === "completed"),
    diagnosticVisible: messages.some((message) => message.includes("Forbidden lint suppression")),
    commitDenied:
      deniedBash.length === 2 && /quality gate failing/i.test(deniedBash[0]?.error ?? ""),
    pushDenied: /quality gate failing/i.test(deniedBash[1]?.error ?? ""),
    markersAbsent: !s.exists("commit-marker") && !s.exists("push-marker"),
    editCompleted: states.some((state) => state.tool === "edit" && state.status === "completed"),
    recovered: qualityGate(s)?.status === "passing" && s.sb.read("bad.rs") === CLEAN,
    unrelatedIgnored:
      s.sb.read("notes.md") === SUPPRESSED &&
      !(messages.at(-1)?.includes("QUALITY VIOLATION") ?? false),
    hostSucceeded: host.exitCode === 0,
  };
  return verdict(observed, { messages, states, stderr: host.stderr });
}

function seedPatch(project: string): string {
  return [
    "*** Begin Patch",
    `*** Add File: ${join(project, "src/old.rs")}`,
    ...added(UNWRAP),
    `*** Add File: ${join(project, "src/removed.rs")}`,
    ...added(UNWRAP),
    "*** End Patch",
  ].join("\n");
}

function patchText(project: string): string {
  return [
    "*** Begin Patch",
    `*** Update File: ${join(project, "src/old.rs")}`,
    `*** Move to: ${join(project, "src/moved.rs")}`,
    "@@",
    " pub fn load() -> u32 {",
    '-    "1".parse().unwrap()',
    '+    "1".parse().expect("number")',
    " }",
    `*** Delete File: ${join(project, "src/removed.rs")}`,
    `*** Add File: ${join(project, "tests/added.rs")}`,
    ...added(MOCKED),
    `*** Add File: ${join(project, "src/notes.md")}`,
    ...added(NOTES),
    "*** End Patch",
  ].join("\n");
}

async function patchQuality(ctx: ScenarioContext) {
  using s = session(ctx, {
    model: "gpt-5-probe",
    files: PROJECT,
    scripts: (project) => ({
      "rsquality.patch": [
        { tool: "apply_patch", args: { patchText: seedPatch(project) } },
        { tool: "apply_patch", args: { patchText: patchText(project) } },
      ],
    }),
  });
  await prepareSdk(s, ctx.cacheRoot);
  const host = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:rsquality.patch"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(host);
  const messages = finalMessages(s, "tool");
  const entries = qualityGate(s)?.entries ?? {};
  const moved = join(s.sb.project, "src/moved.rs");
  const expected = [moved, join(s.sb.project, "tests/added.rs")].toSorted();
  const observed = {
    seedFailed: messages.some(
      (message) =>
        message.includes(`.unwrap() in ${join(s.sb.project, "src/old.rs")}`) &&
        message.includes(`.unwrap() in ${join(s.sb.project, "src/removed.rs")}`),
    ),
    patchCompleted: states.some(
      (state) => state.tool === "apply_patch" && state.status === "completed",
    ),
    moveApplied:
      !s.exists("src/old.rs") && s.exists("src/moved.rs") && s.sb.read("src/moved.rs") === EXPECT,
    deleteApplied: !s.exists("src/removed.rs"),
    addApplied: s.exists("tests/added.rs") && s.sb.read("tests/added.rs") === MOCKED,
    unrelatedApplied: s.exists("src/notes.md") && s.sb.read("src/notes.md") === NOTES,
    exactGateEntries:
      qualityGate(s)?.status === "failing" &&
      JSON.stringify(Object.keys(entries).toSorted()) === JSON.stringify(expected) &&
      expected.every((path) => entries[path]?.source === "rust-quality-hook"),
    bothVisible: messages.some(
      (message) =>
        message.includes(`.expect() in ${moved}`) &&
        message.includes("no-mocks: mockall/faux import"),
    ),
    hostSucceeded: host.exitCode === 0,
  };
  return verdict(observed, { messages, states, stderr: host.stderr });
}

export const RUST_QUALITY_SCENARIOS: PretoolScenario[] = [
  { id: "rsquality.edit", run: editQuality },
  { id: "rsquality.patch", run: patchQuality },
  {
    id: "rsquality.disabled",
    run: (ctx) =>
      disabledQuality(ctx, {
        id: "rsquality.disabled",
        files: FILES,
        file: "bad.rs",
        content: SUPPRESSED,
      }),
  },
];
