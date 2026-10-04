import { expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { contractPaths } from "../../opencode-host/results.ts";
import { ProbeResultsSchema, readJson } from "../../opencode-host/schema.ts";
import { CONTROLS, replaceExactly, selectControls, stageControl } from "../controls.ts";
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
