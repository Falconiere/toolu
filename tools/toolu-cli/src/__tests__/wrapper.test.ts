/**
 * The npm wrapper (`toolu-plugins`, #438): it runs the native binary's
 * `toolu plugins`, never takes itself for that binary, and without one names
 * the install commands instead of downloading anything.
 */
import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { z } from "zod";
import { requiredBuiltTooluBinary } from "@toolu/conformance/harness/entry-command";
import { FIXED_DIRS } from "../resolve";

const WRAPPER = resolve(import.meta.dir, "../cli.ts");
const NPM = resolve(import.meta.dir, "../../npm/package.json");
const REPO_ROOT = resolve(import.meta.dir, "../../../..");

const manifest = z
  .looseObject({ version: z.string(), bin: z.record(z.string(), z.string()) })
  .parse(JSON.parse(readFileSync(NPM, "utf8")));

interface Sandbox {
  readonly root: string;
  readonly bin: string;
  [Symbol.dispose](): void;
}

function sandbox(): Sandbox {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "toolu-plugins-wrapper-")));
  const bin = join(root, "bin");
  mkdirSync(bin);
  return { root, bin, [Symbol.dispose]: () => rmSync(root, { recursive: true, force: true }) };
}

function script(path: string, body: string): void {
  writeFileSync(path, `#!/bin/sh\n${body}\n`);
  chmodSync(path, 0o755);
}

function wrapper(args: readonly string[], env: Record<string, string>, cwd: string) {
  const res = spawnSync(process.execPath, [WRAPPER, ...args], { cwd, env: { TERM: "dumb", ...env }, encoding: "utf8" });
  return { code: res.status, signal: res.signal, stdout: res.stdout, stderr: res.stderr };
}

describe("the published manifest", () => {
  test("declares toolu-plugins and no toolu bin", () => {
    expect(manifest.bin).toEqual({ "toolu-plugins": "dist/cli.js" });
    expect(Object.keys(manifest.bin)).not.toContain("toolu");
  });
});

/** A native binary in a fixed install directory would be found before the sandbox's PATH. */
const fixedNative = FIXED_DIRS.some((dir) => existsSync(join(dir, "toolu")));

describe("running the native binary", () => {
  test.skipIf(fixedNative)("a toolu on PATH that is the wrapper itself is never run; the install commands are named", () => {
    using sb = sandbox();
    symlinkSync(WRAPPER, join(sb.bin, "toolu"));
    const run = wrapper(["install", "--dry-run", "--host", "claude"], { PATH: `${sb.bin}:/usr/bin:/bin`, HOME: sb.root }, sb.root);
    expect(run.code).toBe(69);
    expect(run.stderr).toContain("the native toolu binary was not found (looked at ");
    expect(run.stderr).toContain(`, ${join(sb.bin, "toolu")}, `);
    expect(run.stderr).toContain("curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash");
    expect(run.stderr).toContain("brew install falconiere/tap/toolu");
  });

  test("a native binary without plugins is too old to run", () => {
    using sb = sandbox();
    const old = join(sb.bin, "old");
    script(old, '[ "$1" = "--hook-protocol" ] && { echo 2; exit 0; }; exit 64');
    const run = wrapper(["list"], { PATH: "/usr/bin:/bin", HOME: sb.root, TOOLU_BIN: old }, sb.root);
    expect(run.code).toBe(69);
    expect(run.stderr).toContain(`${old} has no \`toolu plugins\`; it predates this package`);
  });

  test("with the native binary it runs toolu plugins from this repository and from an empty directory", () => {
    using sb = sandbox();
    const env = { PATH: "/usr/bin:/bin", HOME: sb.root, TOOLU_BIN: requiredBuiltTooluBinary() };
    for (const cwd of [REPO_ROOT, sb.root]) {
      const run = wrapper(["install", "--dry-run", "--host", "claude", "jev"], env, cwd);
      expect(run.stderr).toBe("");
      expect(run.code).toBe(0);
      expect(run.stdout).toBe("Planned commands (nothing was run):\n  claude plugin install jev@toolu\n");
    }
    const usage = wrapper(["skills"], env, sb.root);
    expect(usage.code).toBe(64);
    expect(usage.stderr).toContain("skills");
  });

  test("the words, the exit status and a killing signal pass through", () => {
    using sb = sandbox();
    const native = join(sb.bin, "native");
    const log = join(sb.root, "argv.log");
    script(
      native,
      `case "$1" in --hook-protocol) echo 2; exit 0 ;; esac
[ "$3" = --help ] && exit 0
echo "$*" >> ${log}
case "$*" in *" list "*) kill -TERM $$ ;; esac
exit 3`,
    );
    const env = { PATH: "/usr/bin:/bin", HOME: sb.root, TOOLU_BIN: native };
    const status = wrapper(["install", "jev"], env, sb.root);
    expect(status.code).toBe(3);
    const catalog = resolve(REPO_ROOT, ".claude-plugin/marketplace.json");
    expect(readFileSync(log, "utf8")).toBe(`plugins install --catalog ${catalog} jev\n`);
    const killed = wrapper(["--host", "codex", "list"], env, sb.root);
    expect(killed.code).toBe(143);
  });

  test("--version answers without a native binary", () => {
    using sb = sandbox();
    const run = wrapper(["--version"], { PATH: "/usr/bin:/bin", HOME: sb.root }, sb.root);
    expect(run.code).toBe(0);
    expect(run.stdout).toBe(`${manifest.version}\n`);
  });
});
