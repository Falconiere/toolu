// npm publish wiring: the CLI is versioned by release-please and published by
// a workflow release-please calls once the Release exists.
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";

const ROOT = resolve(import.meta.dir, "../../..");
const WF = readFileSync(join(ROOT, ".github/workflows/npm-publish.yml"), "utf8");
const RELEASE_PLEASE = readFileSync(join(ROOT, ".github/workflows/release-please.yml"), "utf8");
const PUBLISHED = ["packages/toolu-core", "tools/toolu-cli/npm"];
const TOKEN_LINE = "NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}";

const PackageJson = z.looseObject({
  name: z.string().optional(),
  version: z.string().optional(),
  private: z.boolean().nullish(),
  license: z.string().optional(),
  main: z.string().optional(),
  exports: z.record(z.string(), z.unknown()).optional(),
  bin: z.record(z.string(), z.string()).optional(),
  dependencies: z.record(z.string(), z.string()).optional(),
  publishConfig: z.looseObject({ access: z.string(), provenance: z.boolean() }).optional(),
});
const ReleasePleaseConfig = z.object({
  packages: z.object({
    ".": z.object({
      "extra-files": z.array(z.looseObject({ path: z.string(), jsonpath: z.string().optional() })),
    }),
  }),
});

function readPackage(dir: string): z.infer<typeof PackageJson> {
  return PackageJson.parse(JSON.parse(readFileSync(join(ROOT, dir, "package.json"), "utf8")));
}

function countLines(text: string, needle: string): number {
  return text.split("\n").filter((line) => line.includes(needle)).length;
}

test.concurrent("release-please bumps the CLI package alongside every other package", () => {
  const config = ReleasePleaseConfig.parse(
    JSON.parse(readFileSync(join(ROOT, "release-please-config.json"), "utf8")),
  );
  const paths = config.packages["."]["extra-files"].map((file) => file.path);
  expect(paths).toContain("tools/toolu-cli/package.json");
  expect(paths).toContain("tools/toolu-cli/npm/package.json");
  expect(paths).toContain("packages/toolu-core/package.json");
});

test.concurrent("the CLI version matches every other workspace package", () => {
  const root = readPackage(".").version;
  const dirs = [
    "tools/toolu-cli",
    "tools/toolu-cli/npm",
    "packages/toolu-core",
    "tools/toolu-conformance",
  ];
  for (const dir of dirs) {
    expect({ dir, version: readPackage(dir).version }).toEqual({ dir, version: root });
  }
});

// release-please creates the Release with the default GITHUB_TOKEN, and GitHub
// does not start a workflow run from an event created by that token. A
// `release: published` trigger here is silently ignored -- that is exactly why
// the 6.7.0 release published nothing.
test.concurrent("the publish workflow is chained from release-please, not triggered by the release event", () => {
  expect(RELEASE_PLEASE).toContain("uses: ./.github/workflows/npm-publish.yml");
  expect(RELEASE_PLEASE).toContain("needs.release-please.outputs.releases_created == 'true'");
  expect(WF).toContain("workflow_call:");
  // The trigger that cannot fire must not come back.
  expect(WF).not.toContain("types: [published]");
});

test.concurrent("the publish workflow stays runnable by hand for a given tag", () => {
  expect(WF).toContain("workflow_dispatch:");
  expect(WF).toContain("ref: ${{ inputs.tag }}");
});

// A tag cut before a package became publishable must still be retryable.
test.concurrent("private packages are skipped rather than failing the run", () => {
  expect(countLines(WF, "is private at")).toBe(2);
});

test.concurrent("the publish workflow grants id-token for provenance and publishes with it", () => {
  expect(WF).toContain("id-token: write");
  expect(WF).toContain("npm publish --provenance --access public");
});

// Provenance attestation needs npm >= 11.5.1 and Node >= 22.14. setup-node
// installs the npm bundled with Node, which is older, so the upgrade is
// load-bearing -- Falconiere/toolu-conventions hit this publishing @toolu/create.
test.concurrent("the publish workflow pins Node and upgrades npm high enough for provenance", () => {
  expect(WF).toContain('node-version: "22.14"');
  expect(WF).toContain("npm install --global npm@11.5.1");
});

test.concurrent("the publish workflow reads the token from secrets, never a literal", () => {
  expect(WF).toContain(TOKEN_LINE);
  expect(WF).not.toMatch(/npm_[A-Za-z0-9]{20,}/);
});

test.concurrent("the publish workflow refuses a tag that disagrees with any package version", () => {
  expect(WF).toContain("does not match $dir version");
});

test.concurrent("the published packages are publishable and conformance stays private", () => {
  for (const dir of PUBLISHED) {
    const pkg = readPackage(dir);
    expect({ dir, private: pkg.private ?? null }).toEqual({ dir, private: null });
    expect({ dir, license: pkg.license }).toEqual({ dir, license: "MIT" });
    expect({ dir, publishConfig: pkg.publishConfig?.access }).toEqual({
      dir,
      publishConfig: "public",
    });
    expect({ dir, provenance: pkg.publishConfig?.provenance }).toEqual({ dir, provenance: true });
  }
  // The conformance harness is internal and must never ship.
  expect(readPackage("tools/toolu-conformance").private).toBe(true);
  // The CLI's dev workspace must never ship, and must not carry the published
  // name: a workspace called @toolu/plugins would shadow the registry for npx.
  const cli = readPackage("tools/toolu-cli");
  expect(cli.private).toBe(true);
  expect(cli.name).not.toBe("@toolu/plugins");
});

test.concurrent("a package already on the registry is skipped so a re-run resumes", () => {
  expect(WF).toContain("is already published");
  expect(WF).toContain("skipping");
});

// A failed publish must not be followed by the next package in the same run.
test.concurrent("a failed publish aborts instead of continuing to the next package", () => {
  expect(WF).toContain("failed; stopping");
  // No accumulate-and-continue: the old loop set a status flag and carried on.
  expect(WF).not.toContain("status=1");
});

test.concurrent("an npm view failure that is not a 404 fails the job instead of publishing", () => {
  expect(WF).toContain("grep -q 'E404'");
  expect(WF).toContain("for a reason other than the version being absent");
});

test.concurrent("the published tarball carries the bundle and the manifest and nothing else", async () => {
  const res = await run([process.execPath, "run", join(ROOT, "tooling/src/pack-inventory.ts")], {
    cwd: ROOT,
  });
  expect({ exitCode: res.exitCode, output: res.stdout + res.stderr }).toEqual({
    exitCode: 0,
    output: res.stdout + res.stderr,
  });
});

// npm rejected the unscoped name `toolu` for being too similar to the existing package
// `toml` (E403 at publish time, after a 404 had suggested it was free). The
// similarity filter applies to unscoped names only, so the CLI is scoped. It
// declares exactly one bin: npx runs the only bin, which is what makes
// `npx @toolu/plugins install` hand `install` to the wrapper. The bin is
// `toolu-plugins`, never `toolu`, so a global install cannot shadow the native
// binary (#438).
test.concurrent("the wrapper publishes under @toolu/plugins with toolu-plugins the only command", () => {
  const cli = readPackage("tools/toolu-cli/npm");
  expect(cli.name).toBe("@toolu/plugins");
  expect(Object.keys(cli.bin ?? {})).toEqual(["toolu-plugins"]);
  expect(cli.bin?.["toolu-plugins"]).toBe("dist/cli.js");
});

test.concurrent("every published package carries a README and a LICENSE", () => {
  for (const dir of PUBLISHED) {
    expect({ dir, readme: existsSync(join(ROOT, dir, "README.md")) }).toEqual({
      dir,
      readme: true,
    });
    expect({ dir, license: existsSync(join(ROOT, dir, "LICENSE")) }).toEqual({
      dir,
      license: true,
    });
  }
});

// npm acknowledges a publish minutes before the packument stops returning 404.
// The first ever @toolu/cli (6.8.1) was 404 for about six minutes after
// "+ @toolu/cli@6.8.1", so `npx @toolu/cli` failed right after the release went
// green. Green must mean installable.
test.concurrent("the workflow waits until every published package resolves before going green", () => {
  expect(WF).toContain("resolves on the registry");
  expect(WF).toContain("is still not fetchable from the registry");
  // The wait follows the publish loop; it is not a pre-publish check.
  const lines = WF.split("\n");
  const publishLine = lines.indexOf("      - name: Publish");
  const waitLine = lines.findIndex((line) =>
    line.startsWith("      - name: Wait until every package resolves"),
  );
  expect(publishLine).toBeGreaterThanOrEqual(0);
  expect(waitLine).toBeGreaterThan(publishLine);
  // Each package gets its full propagation window. In the 7.7.0 release,
  // core took almost eight minutes and a shared deadline gave the next package only
  // thirty seconds before falsely failing the deployment.
  const waitLines = lines.slice(waitLine);
  const packageLoop = waitLines.findIndex((line) => line.trim().startsWith("for dir in "));
  const deadline = waitLines.findIndex((line) => line.includes("deadline=$(("));
  const poll = waitLines.findIndex((line) => line.trim().startsWith("until npm view "));
  expect(deadline).toBeGreaterThan(packageLoop);
  expect(deadline).toBeLessThan(poll);
  expect(waitLines[deadline]).toBe("            deadline=$(( $(date +%s) + 8 * 60 ))");
  // Two eight-minute waits plus setup and publishing must fit in the job.
  expect(WF).toContain("timeout-minutes: 30");
  // setup-node's .npmrc reads NODE_AUTH_TOKEN on every npm call, so the wait
  // step needs the token too, not only the publish step.
  expect(countLines(WF, TOKEN_LINE)).toBe(2);
});

/** The workflow's "Check the tag matches every package version" script, for `tag`. */
function tagCheckScript(tag: string): string {
  const lines = WF.split("\n");
  const start = lines.findIndex((line) => line.includes("name: Check the tag matches"));
  const body: string[] = [];
  for (const line of lines.slice(start + 2)) {
    if (!line.startsWith("          ") && line.trim() !== "") break;
    body.push(line.slice(10));
  }
  return body.join("\n").replace("${{ inputs.tag }}", tag);
}

test.concurrent("the tag check passes for this release and refuses a stale package version", async () => {
  const tag = `v${readPackage(".").version ?? ""}`;
  const good = mkdtempSync(join(tmpdir(), "npm-publish-tag-"));
  const stale = mkdtempSync(join(tmpdir(), "npm-publish-tag-"));
  try {
    for (const [dir, version] of [
      [good, readPackage(".").version ?? ""],
      [stale, "0.0.1"],
    ] as const) {
      for (const rel of PUBLISHED) {
        const pkg = z
          .record(z.string(), z.unknown())
          .parse(JSON.parse(readFileSync(join(ROOT, rel, "package.json"), "utf8")));
        mkdirSync(join(dir, rel), { recursive: true });
        writeFileSync(join(dir, rel, "package.json"), JSON.stringify({ ...pkg, version }));
      }
    }
    const ok = await run(["bash", "-c", tagCheckScript(tag)], { cwd: good });
    expect({ exitCode: ok.exitCode, stdout: ok.stdout }).toEqual({
      exitCode: 0,
      stdout: `publishing ${tag}\n`,
    });
    const refused = await run(["bash", "-c", tagCheckScript(tag)], { cwd: stale });
    expect({ exitCode: refused.exitCode, stderr: refused.stderr }).toEqual({
      exitCode: 1,
      stderr: `tag ${tag} does not match packages/toolu-core version 0.0.1\n`,
    });
  } finally {
    for (const dir of [good, stale]) rmSync(dir, { recursive: true, force: true });
  }
});
