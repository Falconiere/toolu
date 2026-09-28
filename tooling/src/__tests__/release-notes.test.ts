import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";

// Release-note drafting in tooling/release.sh, against a REAL throwaway git
// repo (no mocks): a fixture marketplace with a v* tag, one plugin changed after
// the tag and one not, driven via the RELEASE_ROOT override.

const ROOT = resolve(import.meta.dir, "../../..");
const SCRIPT = resolve(ROOT, "tooling/release.sh");
const Manifest = z.object({ version: z.string() });
const CLAUDE = ".claude-plugin/plugin.json";
const CODEX = ".codex-plugin/plugin.json";

function manifest(name: string, version: string): string {
  return `{\n  "name": "${name}",\n  "version": "${version}"\n}\n`;
}

/** Fixture repo: tagged v1.0.0, then ONLY alpha changed after the tag. */
function releaseRepo(): Sandbox {
  const files: Record<string, string> = { "package.json": manifest("toolu", "1.0.0") };
  for (const [name, version] of [
    ["toolu", "1.0.0"],
    ["alpha", "0.1.0"],
    ["beta", "0.2.0"],
  ] as const) {
    files[`plugins/${name}/${CLAUDE}`] = manifest(name, version);
    files[`plugins/${name}/${CODEX}`] = manifest(name, version);
  }
  const sb = createSandbox({ git: true, files });
  try {
    sb.git("tag", "-m", "rel", "v1.0.0");
    sb.write("plugins/alpha/run.sh", "echo hi\n"); // change ONLY alpha after the tag
    sb.git("add", "-A");
    sb.git("commit", "-qm", "touch alpha");
  } catch (err: unknown) {
    sb[Symbol.dispose]();
    throw err;
  }
  return sb;
}

function release(sb: Sandbox, ...args: string[]): ReturnType<typeof run> {
  return run(["bash", SCRIPT, ...args], { cwd: sb.project, env: { RELEASE_ROOT: sb.project } });
}

function ver(sb: Sandbox, rel: string): string {
  return Manifest.parse(JSON.parse(sb.read(rel))).version;
}

test.concurrent("release: apply auto-drafts docs/releases/v1.1.0.md", async () => {
  using sb = releaseRepo();
  expect((await release(sb, "1.1.0")).exitCode).toBe(0);
  expect(existsSync(sb.path("docs/releases/v1.1.0.md"))).toBe(true);
  const notes = sb.read("docs/releases/v1.1.0.md");
  // Header + release date.
  expect(notes).toContain("# toolu v1.1.0");
  expect(notes).toMatch(/^Released: [0-9]{4}-[0-9]{2}-[0-9]{2}$/m);
  // Per-plugin bullets: toolu (anchored) + alpha (changed) are present; beta (unchanged) is NOT.
  expect(notes).toContain("`toolu`");
  expect(notes).toContain("`alpha`");
  expect(notes).not.toContain("`beta`");
  // Section header uses the exact "## Included changes since v<prev>" form.
  expect(notes).toContain("## Included changes since v1.0.0");
  // Upgrade notes section is always drafted (matches tooling/templates/release-notes.md).
  expect(notes).toContain("## Upgrade notes");
  // Highlights placeholder.
  expect(notes).toContain("## Highlights");
  expect(notes).toContain("TODO");
  // toolu bullet shows the captured pre-bump version (1.0.0) -- exercises the
  // toolu_old variable (not a hardcoded index into old_versions).
  expect(notes).toContain("(`toolu` 1.0.0 -> 1.1.0)");
});

test.concurrent("release: apply refuses to overwrite an existing notes file (and aborts before printing the plan)", async () => {
  using sb = releaseRepo();
  sb.write("docs/releases/v1.1.0.md", "marker\n");
  const res = await release(sb, "1.1.0");
  // Error message is on stderr, so check both streams.
  const output = res.stdout + res.stderr;
  expect(res.exitCode).not.toBe(0);
  expect(output).toContain("already exists");
  // Pre-flight runs before the plan banner, so the plan must NOT appear.
  expect(output).not.toContain("plan for v1.1.0");
  // Marker preserved verbatim.
  expect(sb.read("docs/releases/v1.1.0.md")).toBe("marker\n");
  // Atomicity: no manifest was bumped.
  expect(ver(sb, "package.json")).toBe("1.0.0");
  expect(ver(sb, `plugins/toolu/${CLAUDE}`)).toBe("1.0.0");
  expect(ver(sb, `plugins/alpha/${CLAUDE}`)).toBe("0.1.0");
});

test.concurrent("release: no-prev-tag drafts without a 'since' clause and lists every plugin", async () => {
  using sb = releaseRepo();
  sb.git("tag", "-d", "v1.0.0");
  const res = await release(sb, "1.0.0");
  expect(res.exitCode).toBe(0);
  expect(existsSync(sb.path("docs/releases/v1.0.0.md"))).toBe(true);
  const notes = sb.read("docs/releases/v1.0.0.md");
  // No "since v<num>" anywhere.
  expect(notes).not.toMatch(/since v[0-9]/);
  // Initial-release marker is in the placeholder.
  expect(notes).toContain("## Included changes");
  expect(notes).toContain("initial release");
  // Every plugin (toolu, alpha, beta) is in the bullet list.
  expect(notes).toContain("`toolu`");
  expect(notes).toContain("`alpha`");
  expect(notes).toContain("`beta`");
  // toolu was anchored to the requested version.
  expect(ver(sb, `plugins/toolu/${CLAUDE}`)).toBe("1.0.0");
});
