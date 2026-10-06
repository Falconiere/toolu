import { afterAll, beforeAll, expect, test } from "bun:test";
import { cpSync, mkdtempSync, mkdirSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { entryArgv } from "@toolu/conformance/harness/entry-command";
import { nativeTooluAdvice } from "../native-toolu.ts";

const root = resolve(import.meta.dir, "../../../../..");
const binary = join(root, "target/debug/toolu");
const wrapper = join(root, "tools/toolu-cli/npm/dist/cli.js");
const temps: string[] = [];

beforeAll(() => {
  const built = spawnSync("cargo", ["build", "-p", "toolu-cli"], { cwd: root, timeout: 120_000 });
  if (built.status !== 0) throw new Error(`cargo build failed: ${built.stderr.toString()}`);
  const packed = spawnSync(process.execPath, ["run", "--cwd", "tools/toolu-cli", "build"], {
    cwd: root,
    timeout: 120_000,
  });
  if (packed.status !== 0) throw new Error(`npm wrapper build failed: ${packed.stderr.toString()}`);
}, 240_000);

afterAll(() => {
  for (const dir of temps) rmSync(dir, { recursive: true, force: true });
});

function sandbox(): { home: string; config: string; bin: string; env: Record<string, string> } {
  const dir = mkdtempSync(join(tmpdir(), "toolu-native-"));
  temps.push(dir);
  const home = join(dir, "home with 'quote'");
  const config = join(dir, "config");
  const bin = join(dir, "bin");
  mkdirSync(home);
  mkdirSync(config);
  mkdirSync(bin);
  return {
    home,
    config,
    bin,
    env: { HOME: home, TOOLU_CONFIG_DIR: config, PATH: `/usr/bin:/bin` },
  };
}

function stage(source: string, dir: string): string {
  const path = join(dir, "toolu");
  cpSync(source, path);
  return path;
}

function hook(plugin: string, env: Record<string, string>, input: unknown): string {
  const argv = entryArgv(plugin, "check-binary");
  const run = spawnSync(argv[0] ?? "", argv.slice(1), {
    env,
    input: JSON.stringify(input),
    encoding: "utf8",
  });
  expect(run.status).toBe(0);
  return run.stdout;
}

test("native toolu first on the non-login shell PATH is silent and executable", () => {
  const box = sandbox();
  stage(binary, box.bin);
  box.env.PATH = `${box.bin}:/usr/bin:/bin`;
  expect(nativeTooluAdvice({ env: box.env, sessionId: "native" })).toBeUndefined();
  const run = spawnSync("/bin/sh", ["-c", "toolu --version"], { env: box.env });
  expect(run.status).toBe(0);
  expect(run.stdout.toString()).toStartWith("toolu ");
});

test("~/.local/bin outside PATH is named once and the absolute path runs", () => {
  const box = sandbox();
  const local = join(box.home, ".local/bin");
  mkdirSync(local, { recursive: true });
  stage(binary, local);
  const first = nativeTooluAdvice({ env: box.env, sessionId: "local" });
  expect(first).toContain(".local/bin/toolu");
  expect(nativeTooluAdvice({ env: box.env, sessionId: "local" })).toBeUndefined();
  const quoted = first?.split("native binary for this session: ")[1]?.split(". Use")[0];
  if (quoted === undefined) throw new Error("missing absolute-path advice");
  const run = spawnSync("/bin/sh", ["-c", `${quoted} --version`], { env: box.env });
  expect(run.status).toBe(0);
  expect(run.stdout.toString()).toStartWith("toolu ");
});

test("no native binary emits one advisory with both install commands", () => {
  const box = sandbox();
  const first = nativeTooluAdvice({ env: box.env, sessionId: "missing" });
  expect(first).toContain("curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash");
  expect(first).toContain("brew install falconiere/tap/toolu");
  expect(nativeTooluAdvice({ env: box.env, sessionId: "missing" })).toBeUndefined();
});

test("two real plugin SessionStart bundles share one Codex notice", () => {
  const box = sandbox();
  const env = { ...box.env, TOOLU_HOST_OVERRIDE: "codex" };
  const first = hook("jev", env, { hook_event_name: "SessionStart", session_id: "shared" });
  const second = hook("pr-babysit", env, { hook_event_name: "SessionStart", session_id: "shared" });
  const context = JSON.parse(first) as {
    hookSpecificOutput: { hookEventName: string; additionalContext: string };
  };
  expect(context.hookSpecificOutput.hookEventName).toBe("SessionStart");
  expect(context.hookSpecificOutput.additionalContext).toContain(
    "brew install falconiere/tap/toolu",
  );
  expect(second).toBe("");
});

test("OpenCode bootstrap identity deduplicates plugin entries without stdin session_id", () => {
  const box = sandbox();
  const env = { ...box.env, TOOLU_HOST_OVERRIDE: "opencode", TOOLU_SESSION_ID: "open-code-run" };
  const input = { hook_event_name: "SessionStart", source: "startup" };
  expect(hook("jev", env, input)).toContain("additionalContext");
  expect(hook("pr-babysit", env, input)).toBe("");
});

test("the real npm wrapper first on PATH is rejected in favor of a native absolute path", () => {
  const box = sandbox();
  stage(wrapper, box.bin);
  const node = Bun.which("node", { PATH: process.env.PATH ?? "" });
  if (node === null) throw new Error("node is required for the real npm wrapper test");
  symlinkSync(node, join(box.bin, "node"));
  box.env.PATH = `${box.bin}:/usr/bin:/bin`;
  const local = join(box.home, ".local/bin");
  mkdirSync(local, { recursive: true });
  const path = stage(binary, local);
  const advice = nativeTooluAdvice({ env: box.env, sessionId: "wrapper" });
  expect(advice).toContain(".local/bin/toolu");
  const shell = spawnSync("/bin/sh", ["-c", "command -v toolu"], { env: box.env });
  expect(shell.stdout.toString().trim()).toBe(join(box.bin, "toolu"));
  expect(dirname(path)).toBe(local);
});
