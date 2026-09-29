// The install-everything prompts in the README and plugin index stay identical
// and name every plugin in the Claude marketplace catalog.
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { z } from "zod";

const ROOT = resolve(import.meta.dir, "../../..");
const README = readFileSync(join(ROOT, "README.md"), "utf8");
const PLUGIN_INDEX = readFileSync(join(ROOT, "docs/plugins/index.md"), "utf8");
const PackageVersion = z.object({ version: z.string() });

/** The lines between the `install-everything:<host>` markers, joined without a trailing newline. */
function extractRegion(text: string, host: string): string {
  const start = `<!-- install-everything:${host} -->`;
  const end = `<!-- /install-everything:${host} -->`;
  const captured: string[] = [];
  let capturing = false;
  for (const line of text.split("\n")) {
    if (line === start) {
      capturing = true;
    } else if (line === end) {
      capturing = false;
    } else if (capturing) {
      captured.push(line);
    }
  }
  return captured.join("\n").trimEnd();
}

test.concurrent("install-everything fences match in the README and the plugin index", () => {
  for (const host of ["claude", "codex", "opencode"]) {
    const readme = extractRegion(README, host);
    const docs = extractRegion(PLUGIN_INDEX, host);
    expect(readme).not.toBe("");
    expect(readme).toBe(docs);
  }
});

// The fences used to enumerate every plugin as `<name>@toolu`, core first, and
// this test compared that list against the catalog. The CLI now derives both the
// set and the order from .claude-plugin/marketplace.json itself -- covered by
// tools/toolu-cli/src/catalog/__tests__/order.test.ts -- so the README must NOT
// hand-enumerate them: a second copy of the catalog is exactly the drift the
// version column already taught us about.
test.concurrent("install-everything fences drive the CLI instead of enumerating plugins", () => {
  for (const host of ["claude", "codex"]) {
    const fence = extractRegion(README, host);
    expect(fence).not.toBe("");
    expect(fence).toContain("npx @toolu/plugins install");
    expect(fence).not.toContain("comemory");
    expect(fence).not.toMatch(/[a-z0-9-]+@toolu/);
  }
});

test.concurrent("the codex fence targets codex and the claude fence does not", () => {
  expect(extractRegion(README, "codex")).toContain("--host codex");
  expect(extractRegion(README, "claude")).not.toContain("--host");
});

// @toolu/opencode is on npm (from 6.8.0) and carries the bash plugins/ tree, so
// the user prompt installs it with OpenCode's own plugin CLI. The git clone is
// the contributor path in docs/opencode.md, not something a user is told to do.
test.concurrent("install-everything opencode fence installs the npm bridge, not a clone or marketplace plugins", () => {
  const fence = extractRegion(README, "opencode");
  expect(fence).not.toBe("");
  // Mentions the ban explicitly (same intent for Claude/Codex omitting comemory installs).
  expect(fence).toContain("Do not install comemory via toolu");
  expect(fence).not.toMatch(/[a-z0-9-]+@toolu/);
  expect(fence).toContain("opencode plugin add @toolu/opencode");
  expect(fence).toContain(".opencode/toolu/plugins.json");
  expect(fence).not.toContain("TOOLU_REPO_ROOT");
  expect(fence).not.toContain("git clone");
  expect(fence).not.toContain("bun install");
});

// The README used to carry a per-plugin version column that release-please never
// updated, so it silently went stale at every release. The column is gone; this
// keeps a hardcoded repository version from creeping back into the file.
test.concurrent("README does not hardcode the repository version", () => {
  const { version } = PackageVersion.parse(
    JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8")),
  );
  const hits = README.split("\n").filter((line) => line.includes(version));
  expect(hits).toEqual([]);
});
