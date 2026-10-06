import { expect, test } from "bun:test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { configExists, loadConfig, mergeConfig, type LoadedConfig } from "../config-load.ts";

const REPO = resolve(import.meta.dir, "../../../../..");
const FIXTURES = join(REPO, "fixtures/config");
const EXAMPLE = join(REPO, "plugins/toolu/settings/toolu.config.example.json");

function fixture(name: string): string {
  return readFileSync(join(FIXTURES, name), "utf8");
}

type Placed = { user?: string; project?: string };

/** Write raw config text where the claude host reads it, then load it. */
function load(sb: Sandbox, placed: Placed, host: "claude" | "codex" = "claude") {
  const warnings: string[] = [];
  for (const scope of ["user", "project"] as const) {
    const text = placed[scope];
    if (text === undefined) continue;
    const dir = sb.configDir(host, scope);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "toolu.config.json"), text);
  }
  const env = { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project };
  const config = loadConfig({ env, host, warn: (m) => warnings.push(m) });
  return { config, warnings };
}

function section(config: LoadedConfig, key: string): unknown {
  return config.data[key];
}

test.concurrent("no config file: empty data, valid, silent", () => {
  using sb = createSandbox();
  const { config, warnings } = load(sb, {});
  expect(config.data).toEqual({});
  expect(config.invalid).toBeUndefined();
  expect(warnings).toEqual([]);
  expect(config.files.user).toBe(join(sb.home, ".claude", "toolu.config.json"));
  expect(config.files.project).toBe(join(sb.project, ".claude", "toolu.config.json"));
});

test.concurrent("user and project files deep-merge like jq '$u * $p'", () => {
  using sb = createSandbox();
  const { config, warnings } = load(sb, {
    user: fixture("edge-values.json"),
    project: fixture("merge-project.json"),
  });
  expect(warnings).toEqual([]);
  expect(section(config, "lang")).toMatchObject({
    ts: { maxFileLines: "120", maxFnLines: 42, noMocks: "false" },
  });
  // Arrays are replaced, never concatenated; sibling keys survive.
  expect(section(config, "docsSync")).toMatchObject({ mode: "block", surfaces: ["only.md"] });
  expect(section(config, "gates")).toMatchObject({
    preset: "chaotic",
    pushReview: { mode: "block" },
    sweep: false,
  });
});

test.concurrent("codex reads CODEX_HOME-style user root and .codex project dir", () => {
  using sb = createSandbox();
  const { config } = load(sb, { project: fixture("docs-strict.json") }, "codex");
  expect(config.files.project).toBe(join(sb.project, ".codex", "toolu.config.json"));
  expect(section(config, "gates")).toEqual({ preset: "strict" });
});

test.concurrent("mergeConfig: non-objects on the project side replace", () => {
  expect(mergeConfig({ a: { b: 1 } }, { a: null })).toEqual({ a: null });
  expect(mergeConfig({ a: 1 }, { a: { b: 1 } })).toEqual({ a: { b: 1 } });
  expect(mergeConfig({ a: [1, 2] }, { a: [3] })).toEqual({ a: [3] });
  expect(mergeConfig({ a: { b: 1, c: 2 } }, { a: { c: 3 } })).toEqual({ a: { b: 1, c: 3 } });
});

test.concurrent("mergeConfig keeps a __proto__ key as data", () => {
  const merged = mergeConfig({}, JSON.parse('{"__proto__":{"polluted":true}}'));
  expect(Object.getPrototypeOf(merged)).toBe(Object.prototype);
  expect(Object.keys(merged ?? {})).toEqual(["__proto__"]);
});

for (const [label, text] of [
  ["truncated", '{"gates":'],
  ["empty", ""],
  ["null", "null"],
  ["false", "false"],
] as const) {
  test.concurrent(`malformed project file (${label}) is ignored with one warning`, () => {
    using sb = createSandbox();
    const { config, warnings } = load(sb, { user: fixture("docs-strict.json"), project: text });
    expect(config.invalid).toBeUndefined();
    expect(section(config, "gates")).toEqual({ preset: "strict" });
    expect(warnings).toEqual([`malformed JSON in ${config.files.project ?? ""}; ignoring`]);
  });
}

for (const [name, reason] of [
  ["fail-closed-unknown-key.json", "unknown top-level key 'nope'"],
  ["fail-closed-version-2.json", "unsupported version 2 (supported: 1)"],
  ["fail-closed-array.json", "top level is not a JSON object"],
] as const) {
  test.concurrent(`${name} fails closed and names the file`, () => {
    using sb = createSandbox();
    const { config, warnings } = load(sb, {
      user: fixture("docs-strict.json"),
      project: fixture(name),
    });
    expect(config.invalid).toBe(`${config.files.project ?? ""}: ${reason}`);
    expect(config.data).toEqual({});
    expect(warnings).toEqual([`${config.invalid ?? ""}; failing closed (every gate blocks)`]);
  });
}

test.concurrent("a string version fails closed; a missing version is 1", () => {
  using sb = createSandbox();
  expect(load(sb, { project: '{"version":"1"}' }).config.invalid).toContain(
    'unsupported version "1"',
  );
  using other = createSandbox();
  expect(load(other, { project: '{"hooks":{"pre-tools":false}}' }).config.invalid).toBeUndefined();
});

test.concurrent("the repo's own comemory config and the shipped example are valid", () => {
  using sb = createSandbox();
  const own = load(sb, { project: fixture("repo-own-comemory.json") });
  expect(own.config.invalid).toBeUndefined();
  expect(own.warnings).toEqual([]);
  using other = createSandbox();
  const example = load(other, { user: readFileSync(EXAMPLE, "utf8") });
  expect(example.config.invalid).toBeUndefined();
  expect(section(example.config, "prBabysit")).toMatchObject({ dispatch: "herdr" });
});

test.concurrent("a directory at the config path counts as absent", () => {
  using sb = createSandbox();
  mkdirSync(join(sb.project, ".claude", "toolu.config.json"), { recursive: true });
  const { config, warnings } = load(sb, {});
  expect(config.data).toEqual({});
  expect(warnings).toEqual([]);
});

test.concurrent("configExists is a stat-only check on either file", () => {
  using sb = createSandbox();
  const env = { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project };
  expect(configExists({ env, host: "claude" })).toBe(false);
  sb.writeConfig("claude", "project", { version: 1 });
  expect(configExists({ env, host: "claude" })).toBe(true);
  expect(configExists({ env, host: "codex" })).toBe(false);
  sb.writeConfig("codex", "user", { version: 1 });
  expect(configExists({ env, host: "codex" })).toBe(true);
});
