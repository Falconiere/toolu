// Real-data tests for the config CLI: pure transforms, a real save→load round-trip
// against a temp file, and one real subprocess invocation. No mocks; never touches
// the user's real ~/.config (every call passes --path / an explicit temp path).

import { afterAll, describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";

import { DEFAULT_CONFIG, loadConfig } from "../config.ts";
import { addRoot, rmRoot, runCli, saveConfig, setKey } from "../config-cli.ts";

const dir = mkdtempSync(join(tmpdir(), "toolu-dash-cli-"));
afterAll(() => rmSync(dir, { recursive: true, force: true }));
const CLI = join(import.meta.dir, "..", "config-cli.ts");

describe("pure transforms (AC-3 CLI)", () => {
  test("addRoot expands ~ and dedupes", () => {
    const once = addRoot(DEFAULT_CONFIG, "~/Projects");
    expect(once.roots).toEqual([join(homedir(), "Projects")]);
    const twice = addRoot(once, "~/Projects");
    expect(twice.roots.length).toBe(1);
  });

  test("rmRoot removes a ~-expanded root", () => {
    const withRoot = addRoot(DEFAULT_CONFIG, "~/Projects");
    expect(rmRoot(withRoot, "~/Projects").roots).toEqual([]);
  });

  test("setKey coerces numbers and booleans; rejects unknown keys", () => {
    expect(setKey(DEFAULT_CONFIG, "pollMs", "1000").pollMs).toBe(1000);
    expect(setKey(DEFAULT_CONFIG, "open", "true").open).toBe(true);
    expect(() => setKey(DEFAULT_CONFIG, "scanDepth", "deep")).toThrow();
    expect(() => setKey(DEFAULT_CONFIG, "bogus", "x")).toThrow();
  });
});

describe("runCli round-trip on a real file", () => {
  test("add-root then set then get persist and reload", () => {
    const path = join(dir, "dashboard.json");
    runCli(["add-root", "~/work/repos"], path);
    runCli(["set", "activeWithinHours", "24"], path);
    expect(existsSync(path)).toBe(true);
    const { config } = loadConfig(path);
    expect(config.roots).toEqual([join(homedir(), "work", "repos")]);
    expect(config.activeWithinHours).toBe(24);
  });

  test("rm-root persists removal", () => {
    const path = join(dir, "rm.json");
    saveConfig(path, { ...DEFAULT_CONFIG, roots: ["/a", "/b"] });
    runCli(["rm-root", "/a"], path);
    expect(loadConfig(path).config.roots).toEqual(["/b"]);
  });

  test("unknown command throws", () => {
    expect(() => runCli(["frobnicate"], join(dir, "x.json"))).toThrow();
  });

  test("set rejects non-finite and negative numeric values", () => {
    const path = join(dir, "reject.json");
    expect(() => runCli(["set", "scanDepth", "Infinity"], path)).toThrow(
      /non-negative finite/,
    );
    expect(() => runCli(["set", "pollMs", "-5"], path)).toThrow(/non-negative finite/);
    // The rejected sets must not have written a config file.
    expect(existsSync(path)).toBe(false);
  });

  test("set persists a valid non-negative numeric value", () => {
    const path = join(dir, "valid.json");
    runCli(["set", "pollMs", "1000"], path);
    expect(existsSync(path)).toBe(true);
    expect(loadConfig(path).config.pollMs).toBe(1000);
  });
});

describe("real subprocess", () => {
  test("`get --path <file>` prints the effective config JSON", () => {
    const path = join(dir, "sub.json");
    saveConfig(path, { ...DEFAULT_CONFIG, pollMs: 777 });
    const out = execFileSync("bun", ["run", CLI, "get", "--path", path], { encoding: "utf8" });
    expect(JSON.parse(out).pollMs).toBe(777);
  });

  test("`--path` with no file argument errors non-zero (argv parse, not runCli)", () => {
    let status = 0;
    let stderr = "";
    try {
      // `--path` is the final arg with no value following it.
      execFileSync("bun", ["run", CLI, "set", "pollMs", "1000", "--path"], {
        encoding: "utf8",
        // Hermetic: a bare default path would land under XDG_CONFIG_HOME, never ~/.config.
        env: { ...process.env, XDG_CONFIG_HOME: dir },
      });
    } catch (err) {
      const e = err as { status?: number; stderr?: string };
      status = e.status ?? 1;
      stderr = e.stderr ?? "";
    }
    expect(status).not.toBe(0);
    expect(stderr).toContain("--path requires a file argument");
  });
});
