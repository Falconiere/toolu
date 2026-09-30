/**
 * `/statusline:setup` against a real config dir, the resulting settings.json
 * parsed back off disk (ported from setup.bats).
 */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { SETUP } from "./harness.ts";

const CUSTOM = '{\n  "statusLine": { "type": "command", "command": "my-custom-bar" }\n}\n';

/** A config dir under the sandbox and its settings.json path. */
function config(sb: Sandbox): { cfg: string; settings: string } {
  const cfg = sb.path("cfg");
  mkdirSync(cfg, { recursive: true });
  return { cfg, settings: join(cfg, "settings.json") };
}

/** Run setup with `CLAUDE_CONFIG_DIR` at the sandbox's `cfg`, or unset for `defaultDir`. */
function setup(sb: Sandbox, args: string[] = [], defaultDir = false) {
  return run([process.execPath, SETUP, ...args], {
    env: { HOME: sb.home, CLAUDE_CONFIG_DIR: defaultDir ? undefined : sb.path("cfg") },
  });
}

function command(path: string): unknown {
  const doc: unknown = JSON.parse(readFileSync(path, "utf8"));
  if (typeof doc !== "object" || doc === null || !("statusLine" in doc)) return undefined;
  const line = doc.statusLine;
  return typeof line === "object" && line !== null && "command" in line ? line.command : undefined;
}

test.concurrent("setup: creates settings.json when absent", async () => {
  using sb = createSandbox();
  const { cfg, settings } = config(sb);
  const res = await setup(sb);
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toBe(
    `CREATED wrote ${settings} with the statusLine wired. Restart the session to see it.\n`,
  );
  expect(command(settings)).toBe(`"${cfg}/statusline/statusline.sh"`);
});

test.concurrent("setup: adds statusLine to an existing settings.json, preserving other keys", async () => {
  using sb = createSandbox();
  const { cfg, settings } = config(sb);
  writeFileSync(settings, '{\n  "theme": "dark"\n}\n');
  const res = await setup(sb);
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toStartWith(`WIRED added statusLine to ${settings}`);
  expect(JSON.parse(readFileSync(settings, "utf8"))).toEqual({
    theme: "dark",
    statusLine: { type: "command", command: `"${cfg}/statusline/statusline.sh"` },
  });
  expect(readFileSync(`${settings}.bak`, "utf8")).toBe('{\n  "theme": "dark"\n}\n');
});

test.concurrent("setup: is idempotent — second run is a no-op", async () => {
  using sb = createSandbox();
  const { settings } = config(sb);
  await setup(sb);
  const before = readFileSync(settings, "utf8");
  const res = await setup(sb);
  expect(res).toMatchObject({
    exitCode: 0,
    stdout: "ALREADY statusLine already points at the statusline plugin — nothing to do.\n",
  });
  expect(readFileSync(settings, "utf8")).toBe(before);
});

test.concurrent("setup: refuses to clobber a custom statusLine without --force", async () => {
  using sb = createSandbox();
  const { settings } = config(sb);
  writeFileSync(settings, CUSTOM);
  const res = await setup(sb);
  expect(res.exitCode).toBe(3);
  expect(res.stdout).toStartWith("REFUSED a different statusLine is already set");
  expect(readFileSync(settings, "utf8")).toBe(CUSTOM);
  expect(existsSync(`${settings}.bak`)).toBe(false);
});

test.concurrent("setup: --force replaces a custom statusLine (after backing it up)", async () => {
  using sb = createSandbox();
  const { cfg, settings } = config(sb);
  for (const flag of ["--force", "-f"]) {
    writeFileSync(settings, CUSTOM);
    const res = await setup(sb, [flag]);
    expect(res.exitCode).toBe(0);
    expect(res.stdout).toStartWith("WIRED ");
    expect(command(settings)).toBe(`"${cfg}/statusline/statusline.sh"`);
    expect(command(`${settings}.bak`)).toBe("my-custom-bar");
  }
});

test.concurrent("setup: refuses to touch unparseable settings.json", async () => {
  using sb = createSandbox();
  const { settings } = config(sb);
  writeFileSync(settings, "not json {{{");
  const res = await setup(sb);
  expect(res.exitCode).toBe(1);
  expect(res.stdout).toStartWith(`ERROR could not parse ${settings}: `);
  expect(readFileSync(settings, "utf8")).toBe("not json {{{");
});

test.concurrent("setup: wires an explicit config dir as a quoted path, not a bare ~", async () => {
  using sb = createSandbox();
  const { cfg, settings } = config(sb);
  expect((await setup(sb)).exitCode).toBe(0);
  expect(command(settings)).toBe(`"${cfg}/statusline/statusline.sh"`);
});

test.concurrent("setup: the default config dir is wired as ~/.claude", async () => {
  using sb = createSandbox();
  const res = await setup(sb, [], true);
  expect(res.exitCode).toBe(0);
  expect(command(join(sb.home, ".claude/settings.json"))).toBe(
    "~/.claude/statusline/statusline.sh",
  );
});

test.concurrent("setup: upgrades the pre-Bun bash command, then is a no-op", async () => {
  using sb = createSandbox();
  const home = join(sb.home, ".claude");
  mkdirSync(home, { recursive: true });
  const settings = join(home, "settings.json");
  const legacy = `{"statusLine":{"type":"command","command":"bash ~/.claude/statusline/statusline.sh"},"a":1}`;
  writeFileSync(settings, legacy);
  const res = await setup(sb, [], true);
  expect(res).toMatchObject({
    exitCode: 0,
    stdout: `WIRED updated statusLine in ${settings} to run the Bun statusline directly (backup: settings.json.bak). Restart the session to see it.\n`,
  });
  expect(JSON.parse(readFileSync(settings, "utf8"))).toEqual({
    statusLine: { type: "command", command: "~/.claude/statusline/statusline.sh" },
    a: 1,
  });
  expect(readFileSync(`${settings}.bak`, "utf8")).toBe(legacy);
  expect((await setup(sb, [], true)).stdout).toStartWith("ALREADY ");
});

test.concurrent("setup: upgrades the quoted pre-Bun command of an explicit config dir", async () => {
  using sb = createSandbox();
  const { cfg, settings } = config(sb);
  writeFileSync(
    settings,
    JSON.stringify({
      statusLine: { type: "command", command: `bash "${cfg}/statusline/statusline.sh"` },
    }),
  );
  const res = await setup(sb);
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toStartWith(`WIRED updated statusLine in ${settings}`);
  expect(command(settings)).toBe(`"${cfg}/statusline/statusline.sh"`);
});

test.concurrent("setup: an empty settings.json is treated as absent", async () => {
  using sb = createSandbox();
  const { settings } = config(sb);
  writeFileSync(settings, "");
  const res = await setup(sb);
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toStartWith("CREATED ");
});

test.concurrent("setup: refuses a settings.json that is not an object", async () => {
  using sb = createSandbox();
  const { settings } = config(sb);
  writeFileSync(settings, "[1]");
  const res = await setup(sb);
  expect(res).toMatchObject({
    exitCode: 1,
    stdout: `ERROR ${settings} is not a JSON object — not touching it\n`,
  });
  expect(readFileSync(settings, "utf8")).toBe("[1]");
});

test.concurrent("setup: the wired command runs the published renderer by path", async () => {
  using sb = createSandbox();
  const { cfg, settings } = config(sb);
  await setup(sb);
  const hook = await run([process.execPath, join(SETUP, "../session-start.js")], {
    env: { HOME: sb.home, CLAUDE_CONFIG_DIR: cfg, TOOLU_HOST_OVERRIDE: "claude" },
    stdin: "{}",
  });
  expect(hook).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
  const wired = command(settings);
  expect(typeof wired).toBe("string");
  const res = await run(["sh", "-c", String(wired)], {
    env: { HOME: sb.home, CLAUDE_CONFIG_DIR: cfg },
    stdin: "{}",
  });
  expect(res).toMatchObject({
    exitCode: 0,
    stdout: "\x1b[36mClaude\x1b[0m\x1b[2m | \x1b[0m\x1b[35mctx:0/0\x1b[0m",
  });
});
