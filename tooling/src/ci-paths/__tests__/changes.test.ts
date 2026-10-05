import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { CI_PATHS_FILE, loadRepoCiPaths, matchesAny } from "../config.ts";

// The `changes` job (#458) against real git repos seeded from this checkout's
// own files, real event payloads and a real GITHUB_OUTPUT file.

const ROOT = resolve(import.meta.dir, "../../../..");
const SCRIPT = join(ROOT, "tooling/src/ci-changes.ts");
const config = loadRepoCiPaths(ROOT);

function tracked(): string[] {
  const res = Bun.spawnSync(["git", "-C", ROOT, "ls-files", "-z"]);
  return res.stdout.toString().split("\0").filter(Boolean);
}

const RELEASE_ONLY = tracked().filter((path) => matchesAny(config.releaseOnly.paths, path));
const SEEDED = [
  CI_PATHS_FILE,
  ".github/workflows/tests.yml",
  "docs/statusline/README.md",
  "README.md",
  "tools/toolu-opencode/src/plugin/hooks.ts",
  "bun.lock",
  ...RELEASE_ONLY,
];

function seed(): Sandbox {
  const files = Object.fromEntries(
    SEEDED.map((path) => [path, readFileSync(join(ROOT, path), "utf8")]),
  );
  return createSandbox({ git: true, files });
}

function commit(sb: Sandbox, edits: Record<string, string>): string {
  for (const [path, body] of Object.entries(edits)) sb.write(path, body);
  sb.git("add", "-A");
  sb.git("commit", "-q", "-m", "change");
  return sb.git("rev-parse", "HEAD").trim();
}

type Outputs = Record<string, string>;

async function changes(sb: Sandbox, eventName: string, event: object): Promise<Outputs> {
  const eventPath = join(sb.root, "event.json");
  const outputPath = join(sb.root, "github-output");
  await Bun.write(eventPath, JSON.stringify(event));
  await Bun.write(outputPath, "");
  const res = await run([process.execPath, SCRIPT], {
    cwd: sb.project,
    env: {
      GITHUB_EVENT_NAME: eventName,
      GITHUB_EVENT_PATH: eventPath,
      GITHUB_OUTPUT: outputPath,
      CI_CHANGES_ROOT: sb.project,
    },
  });
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
  const lines = readFileSync(outputPath, "utf8").trim().split("\n");
  return Object.fromEntries(
    lines.map((line) => [line.slice(0, line.indexOf("=")), line.slice(line.indexOf("=") + 1)]),
  );
}

async function prOutputs(edits: Record<string, string>): Promise<Outputs> {
  using sb = seed();
  const base = sb.git("rev-parse", "HEAD").trim();
  const head = commit(sb, edits);
  return await changes(sb, "pull_request", {
    pull_request: { base: { sha: base }, head: { sha: head } },
  });
}

const ALL_ON = { ts: "true", opencode: "true", docs: "true", changed: "true" };
const ALL_OFF = { ts: "false", opencode: "false", docs: "false", changed: "false" };

function bump(path: string): string {
  const text = readFileSync(join(ROOT, path), "utf8");
  if (path === "CHANGELOG.md") return `## [99.0.0](https://example.test) (2026-10-05)\n\n${text}`;
  return text.replace(/("(?:version|\.|@toolu\/core)": "\^?)\d+\.\d+\.\d+/g, "$199.0.0");
}

test.concurrent("a docs-only PR turns on docs and changed only (AC-1)", async () => {
  const outputs = await prOutputs({ "docs/statusline/README.md": "# statusline\n\nEdited.\n" });
  expect(outputs).toEqual({ ts: "false", opencode: "false", docs: "true", changed: "true" });
});

test.concurrent("a root Markdown edit is docs-only too (AC-1)", async () => {
  const outputs = await prOutputs({ "README.md": "# toolu\n" });
  expect(outputs).toEqual({ ts: "false", opencode: "false", docs: "true", changed: "true" });
});

test.concurrent("a release-please version bump turns every output off (AC-2)", async () => {
  expect(RELEASE_ONLY).toContain("tools/toolu-cli/npm/package.json");
  const edits = Object.fromEntries(RELEASE_ONLY.map((path) => [path, bump(path)]));
  expect(Object.keys(edits).length).toBeGreaterThan(30);
  expect(await prOutputs(edits)).toEqual(ALL_OFF);
});

test.concurrent("a scripts edit in root package.json is not release-only (AC-2)", async () => {
  const text = readFileSync(join(ROOT, "package.json"), "utf8");
  const edited = text.replace('"test": "bun run test:ts"', '"test": "bun run test:ts --bail"');
  expect(edited).not.toBe(text);
  const outputs = await prOutputs({ "package.json": edited });
  expect(outputs).toEqual({ ts: "true", opencode: "true", docs: "false", changed: "true" });
});

test.concurrent("an OpenCode adapter edit turns on ts and opencode (AC-3)", async () => {
  const path = "tools/toolu-opencode/src/plugin/hooks.ts";
  const text = readFileSync(join(ROOT, path), "utf8");
  const outputs = await prOutputs({ [path]: `${text}\n// edited\n` });
  expect(outputs).toEqual({ ts: "true", opencode: "true", docs: "false", changed: "true" });
});

for (const path of ["newdir/file.txt", ".github/workflows/tests.yml", CI_PATHS_FILE, "bun.lock"]) {
  test.concurrent(`${path} turns every group on (AC-4)`, async () => {
    const existing = SEEDED.includes(path) ? readFileSync(join(ROOT, path), "utf8") : "";
    expect(await prOutputs({ [path]: `${existing}\n` })).toEqual(ALL_ON);
  });
}

test.concurrent("a push compares before..after with the same groups (AC-6)", async () => {
  using sb = seed();
  const before = sb.git("rev-parse", "HEAD").trim();
  const after = commit(sb, { "docs/statusline/README.md": "# pushed\n" });
  const outputs = await changes(sb, "push", { before, after });
  expect(outputs).toEqual({ ts: "false", opencode: "false", docs: "true", changed: "true" });
});

test.concurrent("a push without a previous commit runs everything (AC-6)", async () => {
  using sb = seed();
  const after = sb.git("rev-parse", "HEAD").trim();
  expect(await changes(sb, "push", { before: "0".repeat(40), after })).toEqual(ALL_ON);
});

test.concurrent("a SHA git cannot resolve runs everything (AC-6)", async () => {
  using sb = seed();
  const after = sb.git("rev-parse", "HEAD").trim();
  expect(await changes(sb, "push", { before: "a".repeat(40), after })).toEqual(ALL_ON);
});

test.concurrent("workflow_dispatch and malformed events run everything (AC-6)", async () => {
  using sb = seed();
  expect(await changes(sb, "workflow_dispatch", {})).toEqual(ALL_ON);
  expect(await changes(sb, "pull_request", { pull_request: {} })).toEqual(ALL_ON);
});

test.concurrent("an empty diff runs everything", async () => {
  using sb = seed();
  const head = sb.git("rev-parse", "HEAD").trim();
  const event = { pull_request: { base: { sha: head }, head: { sha: head } } };
  expect(await changes(sb, "pull_request", event)).toEqual(ALL_ON);
});

test.concurrent("an invalid data file fails the job instead of guessing", async () => {
  using sb = seed();
  commit(sb, { [CI_PATHS_FILE]: '{"groups": {}}\n' });
  const outputPath = join(sb.root, "github-output");
  await Bun.write(outputPath, "");
  const res = await run([process.execPath, SCRIPT], {
    env: {
      GITHUB_EVENT_NAME: "workflow_dispatch",
      GITHUB_OUTPUT: outputPath,
      CI_CHANGES_ROOT: sb.project,
    },
  });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain(`${CI_PATHS_FILE} is invalid`);
  expect(readFileSync(outputPath, "utf8")).toBe("");
});

test.concurrent("a CHANGELOG-only diff is release-only", async () => {
  expect(await prOutputs({ "CHANGELOG.md": bump("CHANGELOG.md") })).toEqual(ALL_OFF);
});

test.concurrent("a version bump plus a docs edit runs docs only (AC-1, AC-2)", async () => {
  const edits = Object.fromEntries(RELEASE_ONLY.map((path) => [path, bump(path)]));
  edits["docs/statusline/README.md"] = "# statusline\n\nEdited.\n";
  expect(await prOutputs(edits)).toEqual({
    ts: "false",
    opencode: "false",
    docs: "true",
    changed: "true",
  });
});

test.concurrent("a plugin manifest with a version and another change is not release-only", async () => {
  const path = "plugins/jev/.claude-plugin/plugin.json";
  const edited = bump(path).replace(/"description": "/, '"description": "Edited. ');
  expect(edited).not.toBe(bump(path));
  expect(await prOutputs({ [path]: edited })).toEqual({
    ts: "true",
    opencode: "true",
    docs: "false",
    changed: "true",
  });
});

test.concurrent("deleting a release-only file is not release-only", async () => {
  using sb = seed();
  const base = sb.git("rev-parse", "HEAD").trim();
  sb.git("rm", "-q", "plugins/jev/.codex-plugin/plugin.json");
  sb.git("commit", "-q", "-m", "delete");
  const head = sb.git("rev-parse", "HEAD").trim();
  const event = { pull_request: { base: { sha: base }, head: { sha: head } } };
  expect(await changes(sb, "pull_request", event)).toEqual({
    ts: "true",
    opencode: "true",
    docs: "false",
    changed: "true",
  });
});
