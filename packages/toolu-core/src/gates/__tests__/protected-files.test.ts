/**
 * AC-2, AC-3 (#260): the native protected-files gate over real sandboxes and
 * the shipped settings. Every #283 item 1–3 write fixture and one command per
 * writer kind asks on Claude and blocks on Codex, naming the target; a pattern
 * target is checked as every path it expands to; a dynamic target by its text.
 */
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import type { Decision } from "../../decision/decision.ts";
import type { HostName } from "../../host/host-name.ts";
import type { RegistryHookEvent } from "../../registry/registry-types.ts";
import { protectedFilesModule } from "../protected-files.ts";
import { gateCtx, gateEnv, SHIPPED_SETTINGS, shellEvent, toolEvent } from "./gate-harness.ts";

const FIXTURES = resolve(import.meta.dir, "../../../../../fixtures/shell/issue-283.json");
const Fixtures = z.object({
  cases: z.array(
    z.looseObject({ id: z.string(), item: z.number(), kind: z.string(), command: z.string() }),
  ),
});
const WRITE_FIXTURES = Fixtures.parse(JSON.parse(readFileSync(FIXTURES, "utf8"))).cases.filter(
  (c) => c.item <= 3 && c.kind === "bash_write_targets",
);

const gate = protectedFilesModule();

function decide(
  sb: Sandbox,
  event: RegistryHookEvent,
  host: HostName = "claude",
): Promise<Decision> {
  return gate.run(event, gateCtx(sb, host, gateEnv(sb, SHIPPED_SETTINGS)));
}

function reasonOf(decision: Decision): string {
  return decision.kind === "ask" || decision.kind === "deny" ? decision.reason : "";
}

describe("#283 items 1-3 write fixtures", () => {
  test("the fixture file has every write example of items 1-3", () => {
    expect(WRITE_FIXTURES.map((c) => c.id)).toEqual([
      "283-1a",
      "283-1b",
      "283-1c",
      "283-1d",
      "283-1e",
      "283-1f",
      "283-1g",
      "283-2a",
      "283-2b",
      "283-2c",
      "283-3a",
      "283-3b",
      "283-3e",
    ]);
  });
  for (const fixture of WRITE_FIXTURES) {
    for (const host of ["claude", "codex"] as const) {
      test.concurrent(`${fixture.id} ${fixture.command} [${host}]`, async () => {
        using sb = createSandbox({ git: true });
        const decision = await decide(sb, shellEvent(sb, fixture.command), host);
        expect(decision.kind).toBe(host === "claude" ? "ask" : "deny");
        const target = fixture.item === 2 && fixture.id !== "283-2c" ? "apps/api/.env" : ".env";
        expect(reasonOf(decision)).toContain(`would WRITE to ${target},`);
      });
    }
  }
});

const WRITERS: readonly [string, string][] = [
  ["echo x 2>.env", ".env"],
  ["echo x | tee -a .env", ".env"],
  ["sed -i 's/a/b/' .env.local", ".env.local"],
  ["perl -i -pe 's/a/b/' .env", ".env"],
  ["mv x .env", ".env"],
  ["install -m 644 x .env", ".env"],
  ["cp -t apps/api src/.env", "apps/api/.env"],
  ["dd if=/dev/zero of=.env", ".env"],
  ["python3 -c \"open('.env','w')\"", ".env"],
  ["echo x > $HOME/.env", "$HOME/.env"],
  ["echo x > .en[v]", ".env"],
];

describe("one command per writer kind", () => {
  for (const [command, target] of WRITERS) {
    for (const host of ["claude", "codex"] as const) {
      test.concurrent(`${command} [${host}]`, async () => {
        using sb = createSandbox({ git: true, files: { ".env": "x" } });
        const decision = await decide(sb, shellEvent(sb, command), host);
        expect(decision.kind).toBe(host === "claude" ? "ask" : "deny");
        expect(reasonOf(decision)).toContain(`would WRITE to ${target},`);
      });
    }
  }
});

describe("commands that write nothing protected stay silent", () => {
  for (const command of [
    "cat .env.example",
    "echo x 2>&1",
    "echo x > /tmp/scratch.txt",
    'echo x > "$OUT"',
    "echo x > .en[x]",
    "git commit -m 'clean up .env handling'",
    "cat <<'EOF'\necho hi > .env\nEOF",
  ]) {
    test.concurrent(command, async () => {
      using sb = createSandbox({ git: true, files: { ".env": "x" } });
      expect(await decide(sb, shellEvent(sb, command))).toEqual({ kind: "allow" });
    });
  }
});

test.concurrent("a pattern with no existing match is checked as written", async () => {
  using sb = createSandbox({ git: true });
  expect((await decide(sb, shellEvent(sb, "echo x > .env[.]*"))).kind).toBe("allow");
  expect((await decide(sb, shellEvent(sb, "echo x > .env.lo[c]al"))).kind).toBe("ask");
});

test.concurrent("edit tools check file_path; other tools and empty input are allowed", async () => {
  using sb = createSandbox({ git: true });
  for (const tool of ["Edit", "Write", "MultiEdit"]) {
    expect((await decide(sb, toolEvent(sb, tool, { file_path: sb.path(".env") }))).kind).toBe(
      "ask",
    );
  }
  expect(await decide(sb, toolEvent(sb, "Read", { file_path: ".env" }))).toEqual({ kind: "allow" });
  expect(await decide(sb, toolEvent(sb, "Edit", { file_path: "" }))).toEqual({ kind: "allow" });
  expect(await decide(sb, toolEvent(sb, "Edit", { file_path: 5 }))).toEqual({ kind: "allow" });
  expect(await decide(sb, toolEvent(sb, "Bash", {}))).toEqual({ kind: "allow" });
});

test.concurrent("a missing settings directory or list allows", async () => {
  using sb = createSandbox({ git: true });
  const env = gateEnv(sb, join(sb.root, "no-settings"));
  expect(await gate.run(shellEvent(sb, "echo x > .env"), gateCtx(sb, "claude", env))).toEqual({
    kind: "allow",
  });
});

test.concurrent("modes: block denies, advise advises, off allows, bad envelope blocks", async () => {
  using sb = createSandbox({ git: true });
  const event = toolEvent(sb, "Edit", { file_path: ".env" });
  const withMode = async (config: object) => {
    sb.writeConfig("claude", "project", config);
    return decide(sb, event);
  };
  expect((await withMode({ version: 1, gates: { protectedFiles: { mode: "block" } } })).kind).toBe(
    "deny",
  );
  const advise = await withMode({ version: 1, gates: { protectedFiles: { mode: "advise" } } });
  expect(advise).toMatchObject({ kind: "advisory" });
  expect(await withMode({ version: 1, gates: { protectedFiles: { mode: "off" } } })).toEqual({
    kind: "allow",
  });
  expect((await withMode({ version: 2 })).kind).toBe("deny");
});

test.concurrent("a pattern that matches hundreds of files still finds the protected one", async () => {
  const files: Record<string, string> = { "zz.oxlintrc.json": "x" };
  for (let i = 0; i < 300; i += 1) files[`f${String(i).padStart(3, "0")}.txt`] = "x";
  using sb = createSandbox({ git: true, files });
  const decision = await decide(sb, shellEvent(sb, "echo x | tee *"));
  expect(decision.kind).toBe("ask");
  expect(reasonOf(decision)).toContain("would WRITE to zz.oxlintrc.json,");
});

test.concurrent("an absolute path is made repo-relative from the hook's cwd", async () => {
  using sb = createSandbox({ git: true });
  const event = toolEvent(sb, "Edit", { file_path: sb.path(".env") });
  const env = gateEnv(sb, SHIPPED_SETTINGS);
  const decision = await gate.run(event, gateCtx(sb, "claude", env, sb.project));
  expect(reasonOf(decision)).toContain("This is a secrets file");
});
