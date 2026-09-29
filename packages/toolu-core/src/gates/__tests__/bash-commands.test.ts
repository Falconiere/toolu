/**
 * bash-commands (#261): rule scope per simple command. The bats-parity and
 * #283 `bash_commands_decide` fixtures run through `bashCommandsDecide` in
 * `shell/__tests__` (`tsDecide`); these pin what those fixtures do not reach,
 * and the module's delivery per mode and host on real settings and config.
 */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { MAX_SHELL_INPUT } from "../../shell/shell-parse.ts";
import { analyzeShell } from "../../shell/shell-parse.ts";
import { bashCommandsDecide, bashCommandsModule } from "../bash-commands.ts";
import { gateCtx, gateEnv, shellEvent, toolEvent } from "./gate-harness.ts";

const SHIPPED_DENY = [
  "node -e",
  "node -p",
  "node --eval",
  "node --print",
  "bun -e",
  "bun --eval",
  "cargo test",
];

function decide(command: string, allow: string[], deny: string[]): string {
  const verdict = bashCommandsDecide(analyzeShell(command), { allow, deny });
  if (verdict.kind === "unknown") return `unknown:${verdict.why}`;
  return verdict.kind === "allow" ? "allow" : `deny:${verdict.rule}`;
}

test("an allow rule overrides a deny only on the command the deny matched (#283 item 4)", () => {
  expect(decide("ls && node -e 1", ["ls"], ["node -e"])).toBe("deny:node -e");
  expect(decide("ls && node -e 1", ["node -e"], ["node -e"])).toBe("allow");
  expect(decide("node -e 1 && node -e 2", ["node -e"], ["node -e"])).toBe("allow");
});

test("the reported rule is the first denylist rule that some command matches", () => {
  expect(decide("cargo test && node -e 1", [], SHIPPED_DENY)).toBe("deny:node -e");
  expect(decide("cargo test && echo ok", [], SHIPPED_DENY)).toBe("deny:cargo test");
});

test("a single-token rule is a substring of each command's text, heredoc bodies excluded", () => {
  expect(decide("echo x > biome.txt", [], ["biome"])).toBe("deny:biome");
  expect(decide("npx biome check", [], ["biome"])).toBe("deny:biome");
  expect(decide("cat <<EOF\nbiome\nEOF", [], ["biome"])).toBe("allow");
  expect(decide("cat <<EOF\n$(biome check)\nEOF", [], ["biome"])).toBe("deny:biome");
});

test("a line that cannot be analyzed is a guardrail hit, whatever the rules", () => {
  const oversize = `ls # ${"x".repeat(MAX_SHELL_INPUT)}`;
  expect(analyzeShell(oversize).unknown).toBe(true);
  expect(decide(oversize, [], ["node -e"])).toStartWith("unknown:oversize:");
  expect(decide(oversize, ["ls"], ["node -e"])).toStartWith("unknown:oversize:");
});

function settings(sb: Sandbox, deny: string, allow = ""): string {
  const dir = join(sb.root, "settings");
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "bash-denylist.txt"), `${deny}\n`);
  writeFileSync(join(dir, "bash-allowlist.txt"), `${allow}\n`);
  return dir;
}

async function run(sb: Sandbox, host: "claude" | "codex", command: string, mode?: string) {
  if (mode !== undefined) {
    sb.writeConfig(host, "project", { version: 1, gates: { bashCommands: { mode } } });
  }
  const ctx = gateCtx(sb, host, gateEnv(sb, settings(sb, "node -e")));
  return bashCommandsModule().run(shellEvent(sb, command), ctx);
}

test.concurrent("each mode delivers the match its own way", async () => {
  using sb = createSandbox();
  expect(await run(sb, "claude", "node -e 1", "block")).toEqual({
    kind: "deny",
    reason: "Command blocked by deny rule: node -e",
  });
  expect(await run(sb, "claude", "node -e 1", "advise")).toEqual({
    kind: "advisory",
    message:
      "Command matches deny rule \"node -e\" (plugins/toolu/settings/bash-denylist.txt). The command was not stopped — gates.bashCommands.mode is 'advise'.",
  });
  const ask = await run(sb, "claude", "node -e 1", "ask");
  expect(ask.kind).toBe("ask");
  expect(ask.kind === "ask" ? ask.reason : "").toContain('the deny rule "node -e"');
  expect(await run(sb, "claude", "node -e 1", "off")).toEqual({ kind: "allow" });
});

test.concurrent("a guardrail's ask blocks on Codex, which cannot prompt", async () => {
  using sb = createSandbox();
  expect((await run(sb, "codex", "node -e 1", "ask")).kind).toBe("deny");
});

test.concurrent("no deny list, or a tool that is not a shell, allows", async () => {
  using sb = createSandbox();
  const ctx = gateCtx(sb, "claude", gateEnv(sb, join(sb.root, "missing")));
  expect(await bashCommandsModule().run(shellEvent(sb, "node -e 1"), ctx)).toEqual({
    kind: "allow",
  });
  const listed = gateCtx(sb, "claude", gateEnv(sb, settings(sb, "node -e")));
  const edit = toolEvent(sb, "Edit", { file_path: sb.path("a.ts") });
  expect(await bashCommandsModule().run(edit, listed)).toEqual({ kind: "allow" });
});

test.concurrent("an oversize line is delivered as a guardrail hit through the mode", async () => {
  using sb = createSandbox();
  const oversize = `node -e 1 # ${"x".repeat(MAX_SHELL_INPUT)}`;
  const block = await run(sb, "claude", oversize, "block");
  expect(block.kind === "deny" ? block.reason : "").toStartWith(
    "Command blocked: it could not be analyzed against the deny rules (oversize:",
  );
  const ask = await run(sb, "claude", oversize, "ask");
  expect(ask.kind === "ask" ? ask.reason : "").toContain("toolu could not analyze (oversize:");
  const advise = await run(sb, "claude", oversize, "advise");
  expect(advise.kind === "advisory" ? advise.message : "").toContain("The command was not stopped");
});
