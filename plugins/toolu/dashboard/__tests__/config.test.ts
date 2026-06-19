// Real-data tests for config loading: writes actual JSON files to a temp dir and
// loads them — no mocks. Covers missing file, malformed JSON, ~/$HOME expansion,
// and partial-merge-over-defaults.

import { afterAll, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";

import { DEFAULT_CONFIG, expandHome, loadConfig } from "../config.ts";

const dir = mkdtempSync(join(tmpdir(), "toolu-dash-config-"));
afterAll(() => rmSync(dir, { recursive: true, force: true }));

function writeConfig(name: string, body: string): string {
  const p = join(dir, name);
  writeFileSync(p, body);
  return p;
}

describe("loadConfig", () => {
  test("missing file → defaults, no warning", () => {
    const { config, warning } = loadConfig(join(dir, "does-not-exist.json"));
    expect(warning).toBeUndefined();
    expect(config).toEqual(DEFAULT_CONFIG);
  });

  test("malformed JSON → defaults + warning", () => {
    const p = writeConfig("bad.json", "{ this is not json ");
    const { config, warning } = loadConfig(p);
    expect(warning).toBeDefined();
    expect(warning).toContain("malformed");
    expect(config).toEqual(DEFAULT_CONFIG);
  });

  test("non-object JSON → defaults + warning", () => {
    const p = writeConfig("arr.json", "[1,2,3]");
    const { config, warning } = loadConfig(p);
    expect(warning).toBeDefined();
    expect(config).toEqual(DEFAULT_CONFIG);
  });

  test("~ and $HOME in roots are expanded to absolute home paths", () => {
    const home = homedir();
    const p = writeConfig(
      "roots.json",
      JSON.stringify({ roots: ["~/Projects", "$HOME/work", "${HOME}/x", "/abs/keep"] }),
    );
    const { config } = loadConfig(p);
    expect(config.roots).toEqual([
      join(home, "Projects"),
      `${home}/work`,
      `${home}/x`,
      "/abs/keep",
    ]);
  });

  test("partial config merges over defaults; unknown keys ignored", () => {
    const p = writeConfig(
      "partial.json",
      JSON.stringify({ scanDepth: 5, open: true, bogus: "ignored" }),
    );
    const { config, warning } = loadConfig(p);
    expect(warning).toBeUndefined();
    expect(config.scanDepth).toBe(5);
    expect(config.open).toBe(true);
    expect(config.activeWithinHours).toBe(DEFAULT_CONFIG.activeWithinHours);
    expect(config.pollMs).toBe(DEFAULT_CONFIG.pollMs);
    expect(config).not.toHaveProperty("bogus");
  });

  test("wrong-typed field falls back to its default", () => {
    const p = writeConfig("wrongtype.json", JSON.stringify({ scanDepth: "deep", port: 8080 }));
    const { config } = loadConfig(p);
    expect(config.scanDepth).toBe(DEFAULT_CONFIG.scanDepth);
    expect(config.port).toBe(8080);
  });

  test("out-of-range numerics are floored (no busy-loop / negative depth)", () => {
    const p = writeConfig("range.json", JSON.stringify({ pollMs: -5, scanDepth: -3, port: -1 }));
    const { config } = loadConfig(p);
    expect(config.pollMs).toBe(1); // floored at 1 so setInterval can't busy-loop
    expect(config.scanDepth).toBe(0);
    expect(config.port).toBe(0);
  });

  test("empty file → defaults, no warning", () => {
    const p = writeConfig("empty.json", "   \n");
    const { config, warning } = loadConfig(p);
    expect(warning).toBeUndefined();
    expect(config).toEqual(DEFAULT_CONFIG);
  });
});

describe("expandHome", () => {
  test("bare ~ → home", () => {
    expect(expandHome("~")).toBe(homedir());
  });
  test("non-home path unchanged", () => {
    expect(expandHome("/var/tmp")).toBe("/var/tmp");
  });
  test("$HOME at the end of a path is expanded", () => {
    expect(expandHome("/foo/$HOME")).toBe(`/foo/${homedir()}`);
    expect(expandHome("/a/${HOME}")).toBe(`/a/${homedir()}`);
  });
});
