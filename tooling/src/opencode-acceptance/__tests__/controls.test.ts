import { expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { contractPaths } from "../../opencode-host/results.ts";
import { ProbeResultsSchema, readJson } from "../../opencode-host/schema.ts";
import { hostEvidence, type AcceptanceCheck } from "../checks.ts";
import { CONTROLS, replaceExactly, runControl, selectControls, stageControl } from "../controls.ts";
import { acceptanceChecks } from "../families.ts";

// Every regression control applies to a fresh stage of the real package (#362 AC-1), so a
// source change that moves an edit target fails here, long before CI's live run.

function withStage(fn: (stage: string) => void): void {
  const work = mkdtempSync(join(tmpdir(), "toolu-controls-test-"));
  try {
    fn(stageControl(work));
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

const read = (stage: string, rel: string): string => readFileSync(join(stage, rel), "utf8");

test.concurrent("each control names a registered check", () => {
  const ids = new Set(
    acceptanceChecks(readJson(contractPaths().results, ProbeResultsSchema)).map((c) => c.id),
  );
  expect(CONTROLS.map((control) => control.check).filter((id) => !ids.has(id))).toEqual([]);
  expect(CONTROLS.map((control) => control.id)).toEqual([
    "control.v2-entry",
    "control.invalid-skill-name",
    "control.missing-context",
    "control.missing-post-tool",
  ]);
});

test.concurrent("the stage resolves the checkout's dependencies", () => {
  withStage((stage) => {
    expect(existsSync(join(stage, "node_modules/zod/package.json"))).toBe(true);
    expect(existsSync(join(stage, "src/plugin/hooks.ts"))).toBe(true);
    expect(existsSync(join(stage, "../../node_modules/@toolu/core/package.json"))).toBe(true);
  });
});

test.concurrent("the V2 control replaces the entry with Plugin.define and permission.hook", () => {
  withStage((stage) => {
    expect(read(stage, "src/plugin/toolu.ts")).not.toContain("permission.hook");
    CONTROLS[0]?.apply(stage);
    const entry = read(stage, "src/plugin/toolu.ts");
    expect(entry).toContain("Plugin.define");
    expect(entry).toContain('ctx.permission.hook("evaluate"');
  });
});

test.concurrent("the skill-name control renames brainstorm-brainstorm everywhere it is defined", () => {
  withStage((stage) => {
    CONTROLS[1]?.apply(stage);
    expect(existsSync(join(stage, "generated/skills/brainstorm-brainstorm"))).toBe(false);
    expect(read(stage, "generated/skills/brainstorm--brainstorm/SKILL.md")).toContain(
      'name: "brainstorm--brainstorm"',
    );
    const catalog = read(stage, "generated/opencode.toolu.json");
    expect(catalog).toContain('"id": "brainstorm--brainstorm"');
    expect(catalog).not.toContain('"brainstorm-brainstorm"');
  });
});

test.concurrent("the hook controls each remove exactly their key from hooks.ts", () => {
  withStage((stage) => {
    CONTROLS[2]?.apply(stage);
    CONTROLS[3]?.apply(stage);
    const hooks = read(stage, "src/plugin/hooks.ts");
    expect(hooks).not.toContain('"experimental.chat.system.transform"');
    expect(hooks).not.toContain('"tool.execute.after"');
    expect(hooks).toContain('"tool.execute.before": enforcement.before,');
  });
});

test.concurrent("an edit whose target moved or repeats throws instead of doing nothing", () => {
  using sb = createSandbox();
  sb.write("a.ts", "one two two\n");
  expect(() => replaceExactly(sb.path("a.ts"), "three", "x")).toThrow(
    'has 0 of "three", expected 1',
  );
  expect(() => replaceExactly(sb.path("a.ts"), "two", "x")).toThrow('has 2 of "two", expected 1');
  expect(() => replaceExactly(sb.path("missing.ts"), "a", "b")).toThrow("does not exist");
  replaceExactly(sb.path("a.ts"), "two", "2", 2);
  expect(sb.read("a.ts")).toBe("one 2 2\n");
});

test.concurrent("a narrowed run selects only the controls it names", () => {
  expect(selectControls([])).toHaveLength(4);
  expect(selectControls(["control.missing-context", "entry.npm-root"]).map((c) => c.id)).toEqual([
    "control.missing-context",
  ]);
});

// Control verdicts over real stages, with checks that read the staged files instead of starting a host.

const CTX = { bin: "", cacheRoot: "", tarball: "" };

/** A check that passes only while the staged hooks.ts still wires tool.execute.after. */
function afterWired(): AcceptanceCheck {
  return {
    id: "posttool.edit",
    family: "test",
    plugins: ["toolu"],
    evidence: hostEvidence(),
    run: () => {
      const stage = process.env.TOOLU_ACCEPTANCE_PACKAGE ?? "";
      const wired = readFileSync(join(stage, "src/plugin/hooks.ts"), "utf8").includes(
        '"tool.execute.after"',
      );
      return Promise.resolve({ pass: wired, observed: { wired } });
    },
  };
}

function fixed(pass: boolean): AcceptanceCheck {
  return { ...afterWired(), run: () => Promise.resolve({ pass, observed: {} }) };
}

const POST_TOOL = CONTROLS[3];

test("a control is detected only when its check passes unbroken and fails after the edit", async () => {
  if (POST_TOOL === undefined) throw new Error("missing control");
  const result = await runControl(POST_TOOL, afterWired(), CTX);
  expect(result.detected).toBe(true);
  expect(result.observed).toEqual({ pristine: "pass", broken: { wired: false } });
  expect(process.env.TOOLU_ACCEPTANCE_PACKAGE).toBeUndefined();
});

test("a check that still passes after the edit is a missed control", async () => {
  if (POST_TOOL === undefined) throw new Error("missing control");
  expect((await runControl(POST_TOOL, fixed(true), CTX)).detected).toBe(false);
});

test("a check that already fails on the unbroken stage proves nothing", async () => {
  if (POST_TOOL === undefined) throw new Error("missing control");
  const result = await runControl(POST_TOOL, fixed(false), CTX);
  expect(result.detected).toBe(false);
  expect(result.error).toContain("fails on the unbroken stage");
});

test("an edit that does not apply is a missed control, not a detection", async () => {
  const moved = {
    id: "control.moved",
    regression: "x",
    check: "posttool.edit",
    apply: (stage: string) =>
      replaceExactly(join(stage, "src/plugin/hooks.ts"), "no such text", ""),
  };
  const result = await runControl(moved, afterWired(), CTX);
  expect(result.detected).toBe(false);
  expect(result.error).toContain('has 0 of "no such text"');
});

test("a second control while one is running is refused, not run against the wrong package", async () => {
  if (POST_TOOL === undefined) throw new Error("missing control");
  const nested: AcceptanceCheck = {
    ...afterWired(),
    run: async () => {
      const inner = await runControl(POST_TOOL, afterWired(), CTX);
      return {
        pass: true,
        observed: { innerError: inner.error ?? "", innerDetected: inner.detected },
      };
    },
  };
  const outer = await runControl(POST_TOOL, nested, CTX);
  const broken = z
    .object({ innerError: z.string(), innerDetected: z.boolean() })
    .parse(outer.observed.broken);
  expect(broken.innerError).toContain("a control is already running");
  expect(broken.innerDetected).toBe(false);
  expect(process.env.TOOLU_ACCEPTANCE_PACKAGE).toBeUndefined();
});
