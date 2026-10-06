import { expect, test } from "bun:test";
import { appendFileSync, cpSync, mkdirSync, symlinkSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";

// Validates real plugin manifests and host marketplace catalogs together.

const ROOT = resolve(import.meta.dir, "../../..");
const SCRIPT = join(ROOT, "tooling/src/validate-plugin-packaging.ts");

const ReleaseConfig = z.looseObject({
  packages: z.looseObject({
    ".": z.looseObject({ "extra-files": z.array(z.looseObject({ path: z.string() })) }),
  }),
});
const Marketplace = z.looseObject({
  plugins: z.array(z.looseObject({ name: z.string(), description: z.string() })),
});
const Versioned = z.looseObject({ version: z.string() });

/** A copy of the real packaging inputs in a throwaway repo the test may corrupt. */
function packagingFixture(): Sandbox {
  const sb = createSandbox();
  try {
    const repo = sb.project;
    for (const file of ["package.json", "release-please-config.json", "Cargo.toml", "Cargo.lock"]) {
      cpSync(join(ROOT, file), join(repo, file));
    }
    for (const dir of ["plugins", ".claude-plugin", ".agents"]) {
      cpSync(join(ROOT, dir), join(repo, dir), { recursive: true, verbatimSymlinks: true });
    }
    for (const pkg of ["packages/toolu-core", "tools/toolu-opencode", "tools/toolu-conformance"]) {
      mkdirSync(join(repo, pkg), { recursive: true });
      cpSync(join(ROOT, pkg, "package.json"), join(repo, pkg, "package.json"));
    }
  } catch (err: unknown) {
    sb[Symbol.dispose]();
    throw err;
  }
  return sb;
}

function validate(repo: string): ReturnType<typeof run> {
  return run([process.execPath, SCRIPT], { cwd: repo, env: { PACKAGING_ROOT: repo } });
}

test.concurrent("plugin packaging validator accepts the checked-in dual-host catalog", async () => {
  const res = await run([process.execPath, SCRIPT], {
    cwd: ROOT,
    env: { PACKAGING_ROOT: undefined },
  });
  const output = res.stdout + res.stderr;
  if (res.exitCode !== 0) {
    console.error(`packaging validator failed:\n${output}`);
  }
  expect(res.exitCode).toBe(0);
  expect(output).toContain("validated 12 plugins");
});

test.concurrent("plugin packaging validator rejects a release config that omits a Codex manifest", async () => {
  using sb = packagingFixture();
  const config = ReleaseConfig.parse(JSON.parse(sb.read("release-please-config.json")));
  const root = config.packages["."];
  root["extra-files"] = root["extra-files"].filter(
    (entry) => entry.path !== "plugins/toolu/.codex-plugin/plugin.json",
  );
  sb.write("release-please-config.json", config);

  const res = await validate(sb.project);
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("release-please is missing");
});

test.concurrent("plugin packaging validator rejects marketplace descriptions that drift from manifests", async () => {
  using sb = packagingFixture();
  const catalog = Marketplace.parse(JSON.parse(sb.read(".claude-plugin/marketplace.json")));
  for (const plugin of catalog.plugins) {
    if (plugin.name === "toolu") {
      plugin.description = "stale description";
    }
  }
  sb.write(".claude-plugin/marketplace.json", catalog);

  const res = await validate(sb.project);
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("marketplace description differs");
});

test.concurrent("plugin packaging validator rejects malformed Codex agent TOML", async () => {
  using sb = packagingFixture();
  appendFileSync(sb.path("plugins/toolu/assets/agents/architect.toml"), "model = [\n");

  const res = await validate(sb.project);
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("invalid agent TOML");
});

test.concurrent("plugin packaging validator rejects symlinks that escape a plugin root", async () => {
  using sb = packagingFixture();
  symlinkSync("/etc/hosts", sb.path("plugins/toolu/escaping-link"));

  const res = await validate(sb.project);
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("symlink escapes plugin root");
});

test.concurrent("plugin packaging validator rejects workspace package.json version drift", async () => {
  using sb = packagingFixture();
  const pkg = Versioned.parse(JSON.parse(sb.read("packages/toolu-core/package.json")));
  pkg.version = "0.0.0";
  sb.write("packages/toolu-core/package.json", pkg);

  const res = await validate(sb.project);
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("packages/toolu-core/package.json version differs");
});

test.concurrent("plugin packaging validator rejects a Cargo workspace version that drifts (#407)", async () => {
  using sb = packagingFixture();
  const manifest = sb.read("Cargo.toml");
  const drifted = manifest.replace(/^version = "[^"]+"$/m, 'version = "9.9.9"');
  expect(drifted).not.toBe(manifest);
  sb.write("Cargo.toml", drifted);

  const res = await validate(sb.project);
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain(
    "Cargo.toml [workspace.package] version differs from package.json",
  );
});

test.concurrent("plugin packaging validator rejects a stale Cargo.lock workspace crate (#407)", async () => {
  using sb = packagingFixture();
  const lock = sb.read("Cargo.lock");
  const stale = lock.replace(/(name = "toolu-runtime"\nversion = )"[^"]+"/, '$1"9.9.9"');
  expect(stale).not.toBe(lock);
  sb.write("Cargo.lock", stale);

  const res = await validate(sb.project);
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("Cargo.lock toolu-runtime version differs");
});

test.concurrent("plugin packaging validator rejects a release config without the Cargo.lock bump (#407)", async () => {
  using sb = packagingFixture();
  const config = ReleaseConfig.parse(JSON.parse(sb.read("release-please-config.json")));
  const root = config.packages["."];
  root["extra-files"] = root["extra-files"].filter((entry) => entry.path !== "Cargo.lock");
  sb.write("release-please-config.json", config);

  const res = await validate(sb.project);
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain(
    "release-please is missing Cargo.lock ($.package[?(!@.source && @.name.value != 'tree-sitter-bash')].version)",
  );
});
