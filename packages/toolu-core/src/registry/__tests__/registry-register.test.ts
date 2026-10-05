/**
 * `registerModules` beyond bash parity (#257): atomic writes and a read-only
 * registry (AC-11), a missing bundle, invalid names, and the bundled
 * SessionStart entry spawned the way a host runs it (AC-9).
 */
import { expect, test } from "bun:test";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  readlinkSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { bundlePath, pluginRoot } from "@toolu/conformance/harness/entry-command";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { registerModules } from "../registry-register.ts";

const BUNDLE = bundlePath(pluginRoot("toolu"), "sample");
const FIXTURE = join(import.meta.dir, "fixtures", "register-fixture.ts");
const IS_ROOT = process.getuid?.() === 0;

test.concurrent("writes atomically and leaves no tmp file; a second run is a no-op", () => {
  using sb = createSandbox();
  const env = { HOME: sb.home, TOOLU_CONFIG_DIR: sb.path("cfg") };
  const modules = [{ name: "m", event: "tool/pre" as const, bundle: BUNDLE }];
  const first = registerModules("a@t", modules, { env });
  const target = sb.path("cfg/toolu/pre-tools.d/a@t__m.js");
  expect(first.written).toEqual([target]);
  expect(readdirSync(sb.path("cfg/toolu/pre-tools.d"))).toEqual(["a@t__m.js"]);
  expect(readFileSync(target).equals(readFileSync(BUNDLE))).toBe(true);
  expect(registerModules("a@t", modules, { env })).toEqual({
    written: [],
    unchanged: [target],
    pruned: [],
    failed: [],
  });
});

test.concurrent.skipIf(IS_ROOT)("a read-only registry keeps the old copy intact", () => {
  using sb = createSandbox();
  const dir = sb.path("cfg/toolu/post-tools.d");
  mkdirSync(dir, { recursive: true });
  const target = join(dir, "a@t__m.js");
  writeFileSync(target, "old\n");
  chmodSync(dir, 0o555);
  try {
    const result = registerModules("a@t", [{ name: "m", event: "tool/post", bundle: BUNDLE }], {
      env: { HOME: sb.home, TOOLU_CONFIG_DIR: sb.path("cfg") },
    });
    expect(result.written).toEqual([]);
    expect(result.failed.map((f) => f.path)).toEqual([target]);
    expect(readFileSync(target, "utf8")).toBe("old\n");
    expect(readdirSync(dir)).toEqual(["a@t__m.js"]);
  } finally {
    chmodSync(dir, 0o755);
  }
});

test.concurrent("a missing bundle keeps the registered copy and still syncs the rest", () => {
  using sb = createSandbox();
  const env = { HOME: sb.home, TOOLU_CONFIG_DIR: sb.path("cfg") };
  const kept = sb.write("cfg/toolu/post-tools.d/a@t__gone.js", "still enforcing\n");
  const result = registerModules(
    "a@t",
    [
      { name: "gone", event: "tool/post", bundle: sb.path("nope.js") },
      { name: "ok", event: "tool/post", bundle: BUNDLE },
    ],
    { env },
  );
  expect(result.failed).toHaveLength(1);
  expect(result.failed[0]?.error).toStartWith("bundle unreadable");
  expect(readFileSync(kept, "utf8")).toBe("still enforcing\n");
  expect(result.written).toEqual([sb.path("cfg/toolu/post-tools.d/a@t__ok.js")]);
  expect(result.pruned).toEqual([]);
});

test.concurrent("an invalid spec or name throws before anything is written", () => {
  using sb = createSandbox();
  const env = { HOME: sb.home, TOOLU_CONFIG_DIR: sb.path("cfg") };
  const bad = () =>
    registerModules(
      "a@t",
      [
        { name: "fine", event: "tool/pre", bundle: BUNDLE },
        { name: "../escape", event: "tool/pre", bundle: BUNDLE },
      ],
      { env },
    );
  expect(bad).toThrow(TypeError);
  expect(existsSync(sb.path("cfg/toolu"))).toBe(false);
});

async function buildEntry(outDir: string): Promise<string> {
  const result = await Bun.build({
    entrypoints: [FIXTURE],
    target: "bun",
    format: "esm",
    sourcemap: "none",
    outdir: outDir,
  });
  expect(result.success).toBe(true);
  return join(outDir, "register-fixture.js");
}

test.concurrent("the bundled SessionStart entry is silent on stdout and exits 0", async () => {
  using sb = createSandbox();
  const entry = await buildEntry(sb.path("dist"));
  const env = { HOME: sb.home, TOOLU_CONFIG_DIR: sb.path("cfg"), FIXTURE_BUNDLE: BUNDLE };
  const stdin = JSON.stringify({ hook_event_name: "SessionStart", source: "startup" });
  const ok = await run([process.execPath, entry], { env, stdin });
  expect(ok).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
  expect(existsSync(sb.path("cfg/toolu/post-tools.d/fixture@toolu__fixture.js"))).toBe(true);

  const missing = await run([process.execPath, entry], {
    env: { ...env, FIXTURE_BUNDLE: sb.path("absent.js") },
    stdin,
  });
  expect(missing).toMatchObject({ exitCode: 0, stdout: "" });
  expect(missing.stderr).toContain("toolu-registry: register fixture@toolu:");
  expect(missing.stderr).toContain("bundle unreadable");
});

test.concurrent("a symlink planted at the tmp path is replaced, never written through", () => {
  using sb = createSandbox();
  const dir = sb.path("cfg/toolu/pre-tools.d");
  mkdirSync(dir, { recursive: true });
  const victim = sb.write("victim.txt", "untouched\n");
  const tmp = join(dir, `a@t__m.js.tmp.${String(process.pid)}`);
  symlinkSync(victim, tmp);
  const result = registerModules("a@t", [{ name: "m", event: "tool/pre", bundle: BUNDLE }], {
    env: { HOME: sb.home, TOOLU_CONFIG_DIR: sb.path("cfg") },
  });
  expect(result.written).toEqual([join(dir, "a@t__m.js")]);
  expect(readFileSync(victim, "utf8")).toBe("untouched\n");
  expect(readFileSync(join(dir, "a@t__m.js")).equals(readFileSync(BUNDLE))).toBe(true);
  expect(() => readlinkSync(tmp)).toThrow();
});

test.concurrent("an invalid module name inside the hook is reported and still exits 0", async () => {
  using sb = createSandbox();
  const entry = await buildEntry(sb.path("dist"));
  const res = await run([process.execPath, entry], {
    env: {
      HOME: sb.home,
      TOOLU_CONFIG_DIR: sb.path("cfg"),
      FIXTURE_BUNDLE: BUNDLE,
      FIXTURE_NAME: "../escape",
    },
    stdin: "{}",
  });
  expect(res).toMatchObject({ exitCode: 0, stdout: "" });
  expect(res.stderr).toBe(
    'toolu-registry: register fixture@toolu: registry: invalid module name "../escape"\n',
  );
  expect(existsSync(sb.path("cfg/toolu"))).toBe(false);
});
