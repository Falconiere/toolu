/**
 * #260: the native mcp-blocker over real settings directories and config
 * layers: prefix and exact entries, redirect hints, comments, config
 * `mcp.<server>: false`, the modes, Codex, and the no-list-no-config fast path.
 */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import type { Decision } from "../../decision/decision.ts";
import type { HostName } from "../../host/host-name.ts";
import { mcpBlockerModule } from "../mcp-blocker.ts";
import { gateCtx, gateEnv, toolEvent } from "./gate-harness.ts";

const gate = mcpBlockerModule();

function settings(sb: Sandbox, list: string | undefined): string {
  const dir = join(sb.root, "settings");
  mkdirSync(dir, { recursive: true });
  if (list !== undefined) writeFileSync(join(dir, "mcp-blocklist.txt"), list);
  return dir;
}

function call(sb: Sandbox, tool: string, list: string | undefined, host: HostName = "claude") {
  const ctx = gateCtx(sb, host, gateEnv(sb, settings(sb, list)));
  return gate.run(toolEvent(sb, tool, {}), ctx);
}

function text(decision: Decision): string {
  if (decision.kind === "ask" || decision.kind === "deny") return decision.reason;
  return decision.kind === "advisory" ? decision.message : "";
}

test.concurrent("listed, prefixed and exact servers ask; others and comments are silent", async () => {
  using sb = createSandbox({ git: true });
  const listed = await call(sb, "mcp__exampleblocked__search", "exampleblocked\n");
  expect(listed.kind).toBe("ask");
  expect(text(listed)).toContain("listed in settings/mcp-blocklist.txt");
  expect(text(listed)).not.toContain("toolu config");
  expect((await call(sb, "mcp__claude_ai_Canva__x", "claude_ai_\n")).kind).toBe("ask");
  expect((await call(sb, "mcp__friend_ai_Canva__x", "claude_ai_\n")).kind).toBe("allow");
  expect((await call(sb, "mcp__other__x", "exampleblocked\n")).kind).toBe("allow");
  expect((await call(sb, "mcp__exampleblocked__x", "  exampleblocked  \n")).kind).toBe("ask");
  expect((await call(sb, "mcp__claude_ai_Atlassian__x", "# claude_ai_Atlassian\n")).kind).toBe(
    "allow",
  );
});

test.concurrent("a redirect hint names the replacement", async () => {
  using sb = createSandbox({ git: true });
  const decision = await call(
    sb,
    "mcp__someserver__do",
    "someserver -> use the host's native Jira tools instead\n",
  );
  expect(text(decision)).toContain("Use instead: use the host's native Jira tools instead");
});

test.concurrent("only mcp__<server>__<tool> names are inspected", async () => {
  using sb = createSandbox({ git: true });
  for (const tool of ["mcp__exampleblocked", "Bash", "exampleblocked__x", "mcp__"]) {
    expect((await call(sb, tool, "exampleblocked\n")).kind).toBe("allow");
  }
});

test.concurrent("Codex cannot prompt: a listed server is blocked", async () => {
  using sb = createSandbox({ git: true });
  const decision = await call(sb, "mcp__exampleblocked__search", "exampleblocked\n", "codex");
  expect(decision.kind).toBe("deny");
  expect(text(decision)).toContain('"exampleblocked"');
});

test.concurrent("config mcp.<server>: false blocks; other values and shapes do not", async () => {
  using sb = createSandbox({ git: true });
  sb.writeConfig("claude", "user", { version: 1, mcp: { someserver: false, other: "false" } });
  const blocked = await call(sb, "mcp__someserver__do", undefined);
  expect(blocked.kind).toBe("ask");
  expect(text(blocked)).toContain("disabled in your toolu config (mcp.someserver=false)");
  expect((await call(sb, "mcp__other__do", undefined)).kind).toBe("allow");
  sb.writeConfig("claude", "user", { version: 1, mcp: "broken-not-an-object" });
  expect((await call(sb, "mcp__someserver__do", undefined)).kind).toBe("allow");
});

test.concurrent("modes and the fail-closed envelope", async () => {
  using sb = createSandbox({ git: true });
  const mode = async (value: string) => {
    sb.writeConfig("claude", "project", { version: 1, gates: { mcpBlocker: { mode: value } } });
    return call(sb, "mcp__exampleblocked__x", "exampleblocked\n");
  };
  expect((await mode("block")).kind).toBe("deny");
  const advise = await mode("advise");
  expect(advise.kind).toBe("advisory");
  expect(text(advise)).toContain("NOT stopped");
  expect((await mode("off")).kind).toBe("allow");
  sb.writeConfig("claude", "project", { version: 2 });
  expect((await call(sb, "mcp__exampleblocked__x", "exampleblocked\n")).kind).toBe("deny");
});

test.concurrent("no blocklist and no config allows without reading config", async () => {
  using sb = createSandbox({ git: true });
  expect(await call(sb, "mcp__exampleblocked__x", undefined)).toEqual({ kind: "allow" });
});

test.concurrent("an invalid envelope still honours mcp.<server>: false, and blocks", async () => {
  using sb = createSandbox({ git: true });
  sb.writeConfig("claude", "user", { version: 2, mcp: { someserver: false } });
  sb.writeConfig("claude", "project", { version: 1, mcp: { other: false } });
  const blocked = await call(sb, "mcp__someserver__do", undefined);
  expect(blocked.kind).toBe("deny");
  expect(text(blocked)).toContain("disabled in your toolu config (mcp.someserver=false)");
  expect((await call(sb, "mcp__other__do", undefined)).kind).toBe("deny");
  expect((await call(sb, "mcp__third__do", undefined)).kind).toBe("allow");
});
