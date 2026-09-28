import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";

// Tests for tooling/release.sh against a REAL throwaway git repo (no mocks):
// a fixture marketplace with a v* tag, one plugin changed after the tag and one
// not, driven via the RELEASE_ROOT override. Release-note drafting lives in
// release-notes.test.ts.

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

test.concurrent("release: anchors package.json + toolu to the new version", async () => {
  using sb = releaseRepo();
  expect((await release(sb, "1.1.0")).exitCode).toBe(0);
  expect(ver(sb, "package.json")).toBe("1.1.0");
  expect(ver(sb, `plugins/toolu/${CLAUDE}`)).toBe("1.1.0");
});

test.concurrent("release: keeps each changed plugin's Codex manifest synchronized", async () => {
  using sb = releaseRepo();
  expect((await release(sb, "1.1.0")).exitCode).toBe(0);
  expect(ver(sb, `plugins/toolu/${CODEX}`)).toBe("1.1.0");
  expect(ver(sb, `plugins/alpha/${CODEX}`)).toBe("0.1.1");
  expect(ver(sb, `plugins/beta/${CODEX}`)).toBe("0.2.0");
});

test.concurrent("release: patch-bumps a plugin changed since the last tag", async () => {
  using sb = releaseRepo();
  expect((await release(sb, "1.1.0")).exitCode).toBe(0);
  expect(ver(sb, `plugins/alpha/${CLAUDE}`)).toBe("0.1.1");
});

test.concurrent("release: leaves an unchanged plugin untouched", async () => {
  using sb = releaseRepo();
  expect((await release(sb, "1.1.0")).exitCode).toBe(0);
  expect(ver(sb, `plugins/beta/${CLAUDE}`)).toBe("0.2.0");
});

test.concurrent("release: rejects a non-semver argument", async () => {
  using sb = releaseRepo();
  expect((await release(sb, "not-a-version")).exitCode).not.toBe(0);
});

test.concurrent("release: rejects a loosely-formed version (extra segment)", async () => {
  using sb = releaseRepo();
  expect((await release(sb, "1.2.3.4")).exitCode).not.toBe(0);
});

test.concurrent("release: a malformed manifest version aborts without corrupting it", async () => {
  using sb = releaseRepo();
  // Make alpha (changed since the tag) carry a non-semver version.
  sb.write(`plugins/alpha/${CLAUDE}`, manifest("alpha", "0.1.x"));
  sb.git("commit", "-qam", "break alpha version");
  expect((await release(sb, "1.1.0")).exitCode).not.toBe(0);
  // alpha untouched (no empty version written); no orphan tmp left behind.
  expect(ver(sb, `plugins/alpha/${CLAUDE}`)).toBe("0.1.x");
  expect(existsSync(sb.path(`plugins/alpha/${CLAUDE}.tmp`))).toBe(false);
  // Atomic: the anchor (package.json + toolu) must NOT have been bumped either.
  expect(ver(sb, "package.json")).toBe("1.0.0");
  expect(ver(sb, `plugins/toolu/${CLAUDE}`)).toBe("1.0.0");
});

test.concurrent("release: manifests stay valid JSON after the bump", async () => {
  using sb = releaseRepo();
  expect((await release(sb, "2.0.0")).exitCode).toBe(0);
  for (const rel of ["package.json", `plugins/toolu/${CLAUDE}`, `plugins/alpha/${CLAUDE}`]) {
    expect(() => {
      JSON.parse(sb.read(rel));
    }).not.toThrow();
  }
});

test.concurrent("release: --dry-run writes nothing", async () => {
  using sb = releaseRepo();
  expect((await release(sb, "--dry-run", "1.1.0")).exitCode).toBe(0);
  expect(ver(sb, "package.json")).toBe("1.0.0");
  expect(ver(sb, `plugins/toolu/${CLAUDE}`)).toBe("1.0.0");
  expect(ver(sb, `plugins/alpha/${CLAUDE}`)).toBe("0.1.0");
  expect(ver(sb, `plugins/beta/${CLAUDE}`)).toBe("0.2.0");
  expect(existsSync(sb.path("docs/releases/v1.1.0.md"))).toBe(false);
});

test.concurrent("release: --dry-run prints a plan with diff stats", async () => {
  using sb = releaseRepo();
  const res = await release(sb, "--dry-run", "1.1.0");
  const output = res.stdout + res.stderr;
  expect(res.exitCode).toBe(0);
  // Anchor transitions show up in the plan.
  expect(output).toContain("1.0.0 -> 1.1.0");
  expect(output).toContain("0.1.0 -> 0.1.1");
  // Per-plugin diff stat is included (alpha is the only changed plugin).
  expect(output).toContain("file changed");
  // Unchanged plugins are listed in the "skipped" footer.
  expect(output).toContain("skipped");
  expect(output).toContain("beta");
  // Banner identifies dry-run.
  expect(output).toContain("dry-run");
});

test.concurrent("release: --no-notes skips the draft but still bumps", async () => {
  using sb = releaseRepo();
  expect((await release(sb, "--no-notes", "1.1.0")).exitCode).toBe(0);
  expect(existsSync(sb.path("docs/releases/v1.1.0.md"))).toBe(false);
  // Manifests are still bumped.
  expect(ver(sb, "package.json")).toBe("1.1.0");
  expect(ver(sb, `plugins/toolu/${CLAUDE}`)).toBe("1.1.0");
  expect(ver(sb, `plugins/alpha/${CLAUDE}`)).toBe("0.1.1");
});

test.concurrent("release: --help exits 0 and prints usage", async () => {
  using sb = releaseRepo();
  const res = await release(sb, "--help");
  const output = res.stdout + res.stderr;
  expect(res.exitCode).toBe(0);
  expect(output).toContain("usage:");
  expect(output).toContain("--dry-run");
  expect(output).toContain("--no-notes");
});
