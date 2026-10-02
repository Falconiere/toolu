import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { contractPaths, provisionedSdkVersion, resultDrift } from "../results.ts";
import { ProbeResultsSchema, readJson, type ProbeResults } from "../schema.ts";

// Real data: the committed live evidence from `bun run probe:opencode-host` (#335).

const committed = readJson(contractPaths({}).results, ProbeResultsSchema);

function probe(id: string): { verdict: string; observed: Record<string, unknown> } {
  const found = committed.probes.find((p) => p.id === id);
  if (found === undefined) throw new Error(`committed results lack ${id}`);
  return found;
}

test("AC-1: the pinned host loads the typed plugin through local-file and config discovery", () => {
  expect(committed.host).toMatchObject({
    cli: "opencode-ai",
    cliVersion: "1.18.34",
    provisionedSdkVersion: "1.18.34",
  });
  expect(probe("load.local-file")).toMatchObject({
    verdict: "supported",
    observed: { loaded: true, optionsDelivered: false },
  });
  expect(probe("load.config-file")).toMatchObject({
    verdict: "supported",
    observed: { loaded: true, optionsDelivered: true, hooksActive: true },
  });
});

test("AC-2: before-throw denies every tool class without side effects and the reason reaches the model", () => {
  for (const id of ["deny.bash", "deny.write", "deny.apply-patch", "deny.mcp"]) {
    expect(probe(id)).toMatchObject({
      verdict: "supported",
      observed: { sideEffect: false, errorReachedModel: true },
    });
  }
  expect(probe("deny.task-child")).toMatchObject({
    verdict: "supported",
    observed: { childHookInvoked: true, sideEffect: false },
  });
});

test("AC-2: permission composition, context delivery and tool failure verdicts", () => {
  expect(probe("permission.ask-hook")).toMatchObject({
    verdict: "unsupported",
    observed: { asked: true, hookInvoked: false },
  });
  expect(probe("permission.config-deny")).toMatchObject({
    verdict: "supported",
    observed: { bashOffered: false, sideEffect: false },
  });
  expect(probe("permission.order").verdict).toBe("supported");
  for (const id of ["context.system", "context.prompt", "context.compaction"]) {
    expect(probe(id)).toMatchObject({ verdict: "supported", observed: { reachedModel: true } });
  }
  expect(probe("post.bash-exit")).toMatchObject({ verdict: "supported", observed: { exit: 3 } });
  expect(probe("post.tool-error")).toMatchObject({
    verdict: "unsupported",
    observed: { afterInvoked: false, errorReachedModel: true },
  });
});

test("AC-3: the loader fails open on init errors and invokes exported helpers", () => {
  expect(probe("load.init-throw")).toMatchObject({
    verdict: "unsupported",
    observed: { toolRanUnguarded: true },
  });
  expect(probe("load.helper-export")).toMatchObject({
    verdict: "unsupported",
    observed: { helperInvokedAsPlugin: true, promptFailed: true },
  });
});

test("AC-1: the plugin receives exactly the documented PluginInput keys", () => {
  expect(probe("load.local-file").observed.inputKeys).toBe(
    "$,client,directory,experimental_workspace,project,serverUrl,worktree",
  );
});

test("live results that match the committed file report no drift; a changed verdict is named", () => {
  expect(resultDrift(committed, committed)).toEqual([]);
  const live: ProbeResults = structuredClone(committed);
  const changed = live.probes.find((p) => p.id === "deny.bash");
  if (changed === undefined) throw new Error("deny.bash missing");
  changed.verdict = "unsupported";
  changed.observed = { ...changed.observed, sideEffect: true };
  const before = committed.probes.find((p) => p.id === "deny.bash");
  expect(resultDrift(committed, live)).toEqual([
    `deny.bash: supported ${JSON.stringify(before?.observed)} -> unsupported ${JSON.stringify(changed.observed)}`,
  ]);
});

test("drift names host version changes, probes missing from the committed file, and count changes", () => {
  const live: ProbeResults = structuredClone(committed);
  live.host.cliVersion = "9.9.9";
  const stale: ProbeResults = {
    ...structuredClone(committed),
    probes: committed.probes.filter((p) => p.id !== "ui.toast"),
  };
  expect(resultDrift(stale, live)).toEqual([
    `host.cliVersion: ${committed.host.cliVersion} -> 9.9.9`,
    "ui.toast: missing from committed results",
    `probe count: ${committed.probes.length - 1} -> ${committed.probes.length}`,
  ]);
});

test("the host-provisioned SDK version is read from the project's .opencode install, or null", () => {
  using sb = createSandbox();
  expect(provisionedSdkVersion(sb.project)).toBeNull();
  sb.write(".opencode/node_modules/@opencode-ai/plugin/package.json", {
    name: "@opencode-ai/plugin",
    version: "1.18.34",
  });
  expect(provisionedSdkVersion(sb.project)).toBe("1.18.34");
});
