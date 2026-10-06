/** `/statusline:setup` against real config files using shared fixture records. */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { materializeCaseValue, readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { entryArgv } from "@toolu/conformance/harness/entry-command";
import { run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import { fixtureString } from "./fixture-actions.ts";
import { PLUGIN } from "./harness.ts";

const CaseSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("setup"),
  scenario: z.enum([
    "create",
    "preserve",
    "idempotent",
    "refuse-custom",
    "force-custom",
    "malformed",
    "explicit",
    "default",
    "legacy-default",
    "legacy-explicit",
    "shell-prefixes",
    "shell-pipelines",
    "empty",
    "not-object",
    "wired-command",
  ]),
  initial: z.string().optional(),
  initialDoc: z.json().optional(),
  expectedExit: z.number().int(),
  expectedStdout: z.json().optional(),
  stdoutPrefix: z.json().optional(),
  expectedCommand: z.json().optional(),
  expectedDoc: z.json().optional(),
  backup: z.string().optional(),
  backupCommand: z.string().optional(),
  backupAbsent: z.boolean().optional(),
  flags: z.array(z.string()).optional(),
  alreadyPrefix: z.string().optional(),
  shells: z.array(z.string()).optional(),
  commands: z.array(z.string()).optional(),
  hookExpected: z.json().optional(),
  rendererExpected: z.json().optional(),
});
const cases = readCaseFile(
  resolve(import.meta.dir, "../../../../../fixtures/statusline/cases.json"),
)
  .filter((raw) => raw.kind === "setup")
  .map((raw) => CaseSchema.parse(raw));

function config(sb: Sandbox): { cfg: string; settings: string } {
  const cfg = sb.path("cfg");
  mkdirSync(cfg, { recursive: true });
  return { cfg, settings: join(cfg, "settings.json") };
}

function setup(sb: Sandbox, args: string[] = [], defaultDir = false) {
  return run([...entryArgv("statusline", "setup", PLUGIN), ...args], {
    env: { HOME: sb.home, CLAUDE_CONFIG_DIR: defaultDir ? undefined : sb.path("cfg") },
  });
}

function command(path: string): unknown {
  const doc: unknown = JSON.parse(readFileSync(path, "utf8"));
  if (typeof doc !== "object" || doc === null || !("statusLine" in doc)) return undefined;
  const line = doc.statusLine;
  return typeof line === "object" && line !== null && "command" in line ? line.command : undefined;
}

for (const c of cases) {
  test.concurrent(c.name, async () => {
    using sb = createSandbox();
    const { cfg, settings } = config(sb);
    if (c.scenario === "create" || c.scenario === "explicit") {
      const res = await setup(sb);
      expect(res.exitCode).toBe(c.expectedExit);
      if (c.expectedStdout !== undefined)
        expect(res.stdout).toBe(fixtureString(sb, c.expectedStdout));
      expect(command(settings)).toBe(fixtureString(sb, c.expectedCommand));
    } else if (c.scenario === "preserve") {
      writeFileSync(settings, z.string().parse(c.initial));
      const res = await setup(sb);
      expect(res.exitCode).toBe(c.expectedExit);
      expect(res.stdout).toStartWith(fixtureString(sb, c.stdoutPrefix));
      expect<unknown>(JSON.parse(readFileSync(settings, "utf8"))).toEqual(
        materializeCaseValue(sb, c.expectedDoc),
      );
      expect(readFileSync(`${settings}.bak`, "utf8")).toBe(z.string().parse(c.backup));
    } else if (c.scenario === "idempotent") {
      await setup(sb);
      const before = readFileSync(settings, "utf8");
      const res = await setup(sb);
      expect(res).toMatchObject({
        exitCode: c.expectedExit,
        stdout: fixtureString(sb, c.expectedStdout),
      });
      expect(readFileSync(settings, "utf8")).toBe(before);
    } else if (c.scenario === "refuse-custom") {
      writeFileSync(settings, z.string().parse(c.initial));
      const res = await setup(sb);
      expect(res.exitCode).toBe(c.expectedExit);
      expect(res.stdout).toStartWith(fixtureString(sb, c.stdoutPrefix));
      expect(readFileSync(settings, "utf8")).toBe(z.string().parse(c.initial));
      if (c.backupAbsent) expect(existsSync(`${settings}.bak`)).toBe(false);
    } else if (c.scenario === "force-custom") {
      for (const flag of z.array(z.string()).parse(c.flags)) {
        writeFileSync(settings, z.string().parse(c.initial));
        const res = await setup(sb, [flag]);
        expect(res.exitCode).toBe(c.expectedExit);
        expect(res.stdout).toStartWith(fixtureString(sb, c.stdoutPrefix));
        expect(command(settings)).toBe(fixtureString(sb, c.expectedCommand));
        expect(command(`${settings}.bak`)).toBe(c.backupCommand);
      }
    } else if (
      c.scenario === "malformed" ||
      c.scenario === "empty" ||
      c.scenario === "not-object"
    ) {
      writeFileSync(settings, z.string().parse(c.initial));
      const res = await setup(sb);
      expect(res.exitCode).toBe(c.expectedExit);
      if (c.stdoutPrefix !== undefined)
        expect(res.stdout).toStartWith(fixtureString(sb, c.stdoutPrefix));
      if (c.expectedStdout !== undefined)
        expect(res.stdout).toBe(fixtureString(sb, c.expectedStdout));
      if (c.scenario !== "empty")
        expect(readFileSync(settings, "utf8")).toBe(z.string().parse(c.initial));
    } else if (c.scenario === "default") {
      const res = await setup(sb, [], true);
      expect(res.exitCode).toBe(c.expectedExit);
      expect(command(join(sb.home, ".claude/settings.json"))).toBe(
        fixtureString(sb, c.expectedCommand),
      );
    } else if (c.scenario === "legacy-default") {
      const home = join(sb.home, ".claude");
      mkdirSync(home, { recursive: true });
      const homeSettings = join(home, "settings.json");
      writeFileSync(homeSettings, z.string().parse(c.initial));
      const res = await setup(sb, [], true);
      expect(res).toMatchObject({
        exitCode: c.expectedExit,
        stdout: fixtureString(sb, c.expectedStdout),
      });
      expect<unknown>(JSON.parse(readFileSync(homeSettings, "utf8"))).toEqual(c.expectedDoc);
      expect(readFileSync(`${homeSettings}.bak`, "utf8")).toBe(z.string().parse(c.initial));
      expect((await setup(sb, [], true)).stdout).toStartWith(z.string().parse(c.alreadyPrefix));
    } else if (c.scenario === "legacy-explicit") {
      writeFileSync(
        settings,
        JSON.stringify(materializeCaseValue(sb, z.json().parse(c.initialDoc))),
      );
      const res = await setup(sb);
      expect(res.exitCode).toBe(c.expectedExit);
      expect(res.stdout).toStartWith(fixtureString(sb, c.stdoutPrefix));
      expect(command(settings)).toBe(fixtureString(sb, c.expectedCommand));
    } else if (c.scenario === "shell-prefixes") {
      const published = fixtureString(sb, c.expectedCommand);
      for (const shell of z.array(z.string()).parse(c.shells)) {
        writeFileSync(
          settings,
          JSON.stringify({ statusLine: { command: `${shell} ${published}` } }),
        );
        const res = await setup(sb);
        expect(res.exitCode).toBe(c.expectedExit);
        expect(res.stdout).toStartWith(fixtureString(sb, c.stdoutPrefix));
        expect(command(settings)).toBe(published);
      }
    } else if (c.scenario === "shell-pipelines") {
      for (const custom of z.array(z.string()).parse(c.commands)) {
        const body = JSON.stringify({ statusLine: { command: custom } });
        writeFileSync(settings, body);
        const res = await setup(sb);
        expect(res.exitCode).toBe(c.expectedExit);
        expect(res.stdout).toStartWith(z.string().parse(c.stdoutPrefix));
        expect(readFileSync(settings, "utf8")).toBe(body);
      }
    } else {
      await setup(sb);
      const hook = await run(entryArgv("statusline", "session-start", PLUGIN), {
        env: { HOME: sb.home, CLAUDE_CONFIG_DIR: cfg, TOOLU_HOST_OVERRIDE: "claude" },
        stdin: "{}",
      });
      expect<unknown>(hook).toMatchObject(z.record(z.string(), z.json()).parse(c.hookExpected));
      const wired = command(settings);
      expect(typeof wired).toBe("string");
      const res = await run(["sh", "-c", String(wired)], {
        env: { HOME: sb.home, CLAUDE_CONFIG_DIR: cfg },
        stdin: "{}",
      });
      expect<unknown>(res).toMatchObject(z.record(z.string(), z.json()).parse(c.rendererExpected));
    }
  });
}
