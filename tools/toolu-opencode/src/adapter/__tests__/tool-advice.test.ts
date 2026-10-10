import { expect, test } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createToolAdviceStore } from "../tool-advice.ts";
import { createToolBeforeHandler } from "../tool-before.ts";
import { registerAstGrep } from "./ast-grep-fixture.ts";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const tmpBase = process.env.TMPDIR ?? "/tmp";

test("a real registry advisory reaches only its matching model-visible tool result", async () => {
  const root = await mkdtemp(join(tmpBase, "toolu-oc-advice-"));
  const configRoot = join(root, "state");
  registerAstGrep(root, configRoot);
  const advice = createToolAdviceStore();
  const before = createToolBeforeHandler(
    {
      repoRoot: REPO_ROOT,
      configRoot,
      permissionContext: { cwd: root, projectRoot: root, worktree: root },
      selectedPluginSpecs: new Set(["toolu@toolu", "ast-grep@toolu"]),
    },
    advice,
  );
  const call = { tool: "grep", sessionID: "session-a", callID: "call-a" };
  await before(call, { args: { pattern: "class Foo" } });
  const other = { title: "other", output: "other result", metadata: {} };
  await advice.after({ ...call, sessionID: "session-b", args: {} }, other);
  expect(other.output).toBe("other result");
  const result = { title: "grep", output: "original result\nother plugin note", metadata: {} };
  await advice.after({ ...call, args: { pattern: "class Foo" } }, result);
  expect(result.output).toContain("original result\nother plugin note");
  expect(result.output).toContain("toolu advisory");
  expect(result.output).toContain("ast-grep");
  const retry = { title: "grep", output: "retry result", metadata: {} };
  await advice.after({ ...call, args: {} }, retry);
  expect(retry.output).toBe("retry result");
});

test("a reused call ID drops old advice before redispatch, even when the next call is skipped", async () => {
  const root = await mkdtemp(join(tmpBase, "toolu-oc-advice-retry-"));
  const advice = createToolAdviceStore();
  const call = { tool: "grep", sessionID: "session-a", callID: "call-a" };
  advice.record(call, "old advice");
  const before = createToolBeforeHandler(
    {
      repoRoot: REPO_ROOT,
      configRoot: join(root, "state"),
      permissionContext: { cwd: root, projectRoot: root, worktree: root },
    },
    advice,
  );
  await before({ ...call, tool: "webfetch" }, { args: { url: "https://example.com" } });
  const output = { title: "grep", output: "new result", metadata: {} };
  await advice.after({ ...call, args: {} }, output);
  expect(output.output).toBe("new result");
});

test("pending advice expires, has a finite cap, and dispose clears it", async () => {
  const advice = createToolAdviceStore({ ttlMs: 1 });
  const stale = { tool: "bash", sessionID: "session-a", callID: "stale" };
  advice.record(stale, "stale advice");
  await Bun.sleep(5);
  const expired = { title: "bash", output: "done", metadata: {} };
  await advice.after({ ...stale, args: {} }, expired);
  expect(expired.output).toBe("done");

  const bounded = createToolAdviceStore();
  for (let i = 0; i < 257; i += 1) {
    bounded.record({ tool: "bash", sessionID: "session-a", callID: `call-${i}` }, `advice-${i}`);
  }
  const oldest = { title: "bash", output: "old", metadata: {} };
  await bounded.after({ tool: "bash", sessionID: "session-a", callID: "call-0", args: {} }, oldest);
  expect(oldest.output).toBe("old");
  const latest = { title: "bash", output: "new", metadata: {} };
  await bounded.after(
    { tool: "bash", sessionID: "session-a", callID: "call-256", args: {} },
    latest,
  );
  expect(latest.output).toContain("advice-256");
  bounded.record(stale, "discard on dispose");
  bounded.clear();
  const disposed = { title: "bash", output: "done", metadata: {} };
  await bounded.after({ ...stale, args: {} }, disposed);
  expect(disposed.output).toBe("done");
});

test("malformed after output reports advice delivery failure without claiming the tool was undone", async () => {
  const advice = createToolAdviceStore();
  const call = { tool: "bash", sessionID: "session-a", callID: "call-a" };
  advice.record(call, "be careful");
  const malformed = { title: "bash", output: "done", metadata: {} };
  Reflect.set(malformed, "output", 42);
  const failure = await advice.after({ ...call, args: {} }, malformed).then(
    () => "",
    (error: unknown) => (error instanceof Error ? error.message : String(error)),
  );
  expect(failure).toContain("toolu: advisory delivery failed after tool execution");
});
