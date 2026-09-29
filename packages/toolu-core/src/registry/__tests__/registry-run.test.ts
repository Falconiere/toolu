/**
 * `runRegistry` over real bundled modules (#257): per-module isolation (AC-5),
 * stop after deny/block (AC-6), bash fallback and re-import (AC-8), installed
 * gating (AC-2), shadowing with one gate write per event (AC-10) and byte-order
 * execution (AC-11).
 */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, utimesSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import type { Decision } from "../../decision/decision.ts";
import { runRegistry, type ModuleOutcome } from "../registry-run.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry-types.ts";
import { buildModule, type FixtureModule } from "./module-bundles.ts";

const PRE = "cfg/toolu/pre-tools.d";
const POST = "cfg/toolu/post-tools.d";

function hookEvent(sb: Sandbox, type: RegistryHookEvent["type"]): RegistryHookEvent {
  const base = {
    sessionId: "s1",
    cwd: sb.project,
    projectRoot: sb.project,
    worktree: sb.project,
    toolCallId: "t1",
    toolName: "Edit",
    toolInput: { file_path: sb.path("src/a.ts") },
  };
  return type === "shell/pre"
    ? { ...base, type, toolName: "Bash", command: "ls" }
    : { ...base, type };
}

function context(sb: Sandbox, raw: Record<string, unknown> = {}): RegistryContext {
  return {
    host: "claude",
    env: { HOME: sb.home },
    configRoot: sb.path("cfg"),
    projectRoot: sb.project,
    raw: { marker: sb.path("ran.log"), ...raw },
  };
}

/** Bundle `[file, behaviour]` pairs into `dir`; spec and name come from the file name. */
async function install(
  sb: Sandbox,
  dir: string,
  modules: [string, FixtureModule["behavior"], Partial<FixtureModule>?][],
): Promise<void> {
  const event = dir === PRE ? "tool/pre" : "tool/post";
  await Promise.all(
    modules.map(([file, behavior, override]) => {
      const [spec = "", name = ""] = file.slice(0, -".js".length).split("__");
      return buildModule(sb.path(`${dir}/${file}`), { spec, name, event, behavior, ...override });
    }),
  );
}

function ran(sb: Sandbox): string[] {
  const log = sb.path("ran.log");
  return existsSync(log) ? readFileSync(log, "utf8").trim().split("\n") : [];
}

function summary(outcomes: ModuleOutcome[]): string[] {
  return outcomes.map((o) =>
    o.status === "decision"
      ? `${o.entry.file}:${o.decision.kind}`
      : `${o.entry.file}:${o.status === "error" ? "error" : o.reason}`,
  );
}

test.concurrent("a failing module is reported and never blocks unrelated modules", async () => {
  using sb = createSandbox();
  await install(sb, PRE, [
    ["a@t__first.js", "advisory"],
    ["b@t__throws.js", "throw"],
    ["c@t__rejects.js", "reject"],
    ["e@t__mismatch.js", "advisory", { spec: "other@t" }],
    ["f@t__invalid.js", "invalid"],
    ["g@t__last.js", "advisory"],
  ]);
  writeFileSync(sb.path(`${PRE}/d@t__syntax.js`), "export default {;\n");
  writeFileSync(sb.path(`${PRE}/h@t__noexport.js`), "export const x = 1;\n");
  const warnings: string[] = [];
  const outcomes = await runRegistry(hookEvent(sb, "tool/pre"), context(sb), {
    warn: (line) => warnings.push(line),
  });
  expect(summary(outcomes)).toEqual([
    "a@t__first.js:advisory",
    "b@t__throws.js:error",
    "c@t__rejects.js:error",
    "d@t__syntax.js:error",
    "e@t__mismatch.js:error",
    "f@t__invalid.js:error",
    "g@t__last.js:advisory",
    "h@t__noexport.js:error",
  ]);
  expect(ran(sb)).toEqual(["first", "throws", "rejects", "invalid", "last"]);
  expect(warnings).toHaveLength(6);
  for (const [i, file] of ["b", "c", "d", "e", "f", "h"].entries()) {
    expect(warnings[i]).toStartWith(`toolu-registry: module ${file}@t__`);
    expect(warnings[i]).toEndWith("; output skipped");
  }
  expect(warnings[0]).toContain("throws exploded");
  expect(warnings[3]).toContain("contract mismatch");
  expect(warnings[4]).toContain("invalid decision");
});

for (const type of ["tool/pre", "shell/pre"] as const) {
  test.concurrent(`a deny ends the walk on ${type}; ask and advisory do not`, async () => {
    using sb = createSandbox();
    await install(sb, PRE, [
      ["a@t__one.js", "advisory"],
      ["b@t__two.js", "ask"],
      ["c@t__three.js", "deny"],
      ["d@t__four.js", "advisory"],
    ]);
    const outcomes = await runRegistry(hookEvent(sb, type), context(sb));
    expect(summary(outcomes)).toEqual([
      "a@t__one.js:advisory",
      "b@t__two.js:ask",
      "c@t__three.js:deny",
    ]);
    expect(ran(sb)).toEqual(["one", "two", "three"]);
  });
}

test.concurrent("a post_block ends the post-tool walk; a deny there does not", async () => {
  using sb = createSandbox();
  await install(sb, POST, [
    ["a@t__one.js", "deny"],
    ["b@t__two.js", "block"],
    ["c@t__three.js", "advisory"],
  ]);
  const outcomes = await runRegistry(hookEvent(sb, "tool/post"), context(sb));
  expect(summary(outcomes)).toEqual(["a@t__one.js:deny", "b@t__two.js:post_block"]);
  expect(ran(sb)).toEqual(["one", "two"]);
});

test.concurrent("modules of a plugin that is not installed are skipped", async () => {
  using sb = createSandbox();
  sb.write("home/.claude/plugins/installed_plugins.json", { version: 2, plugins: { "a@t": [] } });
  await install(sb, PRE, [
    ["a@t__kept.js", "advisory"],
    ["b@t__gone.js", "deny"],
  ]);
  const ctx = { ...context(sb), env: { HOME: sb.path("home") } };
  const outcomes = await runRegistry(hookEvent(sb, "tool/pre"), ctx);
  expect(summary(outcomes)).toEqual(["a@t__kept.js:advisory", "b@t__gone.js:inactive"]);
  expect(ran(sb)).toEqual(["kept"]);
});

test.concurrent("a bash entry goes to the fallback, or is skipped without one", async () => {
  using sb = createSandbox();
  mkdirSync(sb.path(PRE), { recursive: true });
  writeFileSync(sb.path(`${PRE}/z@t__legacy.sh`), "exit 0\n");
  const seen: string[] = [];
  const fallback = (entry: { file: string }): Promise<Decision> => {
    seen.push(entry.file);
    return Promise.resolve({ kind: "advisory", message: "from bash" });
  };
  const event = hookEvent(sb, "tool/pre");
  expect(summary(await runRegistry(event, context(sb), { fallback }))).toEqual([
    "z@t__legacy.sh:advisory",
  ]);
  expect(seen).toEqual(["z@t__legacy.sh"]);
  expect(summary(await runRegistry(event, context(sb)))).toEqual(["z@t__legacy.sh:bash"]);
});

test.concurrent("a module re-registered with new bytes is imported afresh", async () => {
  using sb = createSandbox();
  await install(sb, PRE, [["a@t__mod.js", "advisory"]]);
  const event = hookEvent(sb, "tool/pre");
  expect(summary(await runRegistry(event, context(sb)))).toEqual(["a@t__mod.js:advisory"]);
  await install(sb, PRE, [["a@t__mod.js", "deny"]]);
  const later = new Date(Date.now() + 5000);
  utimesSync(sb.path(`${PRE}/a@t__mod.js`), later, later);
  expect(summary(await runRegistry(event, context(sb)))).toEqual(["a@t__mod.js:deny"]);
});

test.concurrent("a spec's .js module shadows its stale .sh, so the gate is written once", async () => {
  using sb = createSandbox({ git: true });
  const gate = sb.path(".claude/tmp/quality-gate-status.json");
  mkdirSync(join(gate, ".."), { recursive: true });
  await install(sb, POST, [["q@t__new.js", "gate"]]);
  writeFileSync(sb.path(`${POST}/q@t__old.sh`), "exit 0\n");
  const calls: string[] = [];
  const fallback = (entry: { file: string }): Promise<Decision> => {
    calls.push(entry.file);
    return Promise.resolve({ kind: "allow" });
  };
  const outcomes = await runRegistry(hookEvent(sb, "tool/post"), context(sb, { gate }), {
    fallback,
  });
  expect(summary(outcomes)).toEqual(["q@t__new.js:advisory", "q@t__old.sh:shadowed"]);
  expect(calls).toEqual([]);
  const doc: unknown = JSON.parse(readFileSync(gate, "utf8"));
  expect(doc).toMatchObject({ status: "failing", source: "new" });
  expect(Object.keys((doc as { entries: object }).entries)).toEqual(["src/a.ts"]);
  const telemetry = readFileSync(sb.path(".claude/tmp/telemetry/main.jsonl"), "utf8");
  expect(telemetry.trim().split("\n")).toHaveLength(1);
  expect(telemetry).toContain('"event":"gate_fail"');
});

test.concurrent("un-namespaced files are warned about and never run", async () => {
  using sb = createSandbox();
  mkdirSync(sb.path(PRE), { recursive: true });
  writeFileSync(sb.path(`${PRE}/nosep.js`), "throw new Error('ran');\n");
  const warnings: string[] = [];
  const outcomes = await runRegistry(hookEvent(sb, "tool/pre"), context(sb), {
    warn: (line) => warnings.push(line),
  });
  expect(outcomes).toEqual([]);
  expect(warnings).toEqual([
    "toolu-registry: registry module nosep.js lacks <plugin-spec>__<name> namespace; skipped",
  ]);
});

test.concurrent("an absent registry directory runs nothing", async () => {
  using sb = createSandbox();
  expect(await runRegistry(hookEvent(sb, "tool/post"), context(sb))).toEqual([]);
});
