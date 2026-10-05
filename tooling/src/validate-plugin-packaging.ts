/**
 * Validates that every plugin is packaged consistently for Claude Code and
 * Codex: manifests agree, marketplace catalogs list each plugin once with the
 * right source and policy, release-please bumps every versioned manifest,
 * the Cargo workspace version moves with package.json (#407),
 * skills carry frontmatter, Codex agent TOML parses, and hook commands point at
 * executable plugin-relative scripts (launcher one-liners are gated by
 * check-hooks-json.ts). `PACKAGING_ROOT` points it at another checkout.
 */
import { existsSync, readdirSync, readlinkSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { get, list } from "./json-path.ts";
import {
  EXPECTED,
  PackagingError,
  Repo,
  checkAgents,
  checkHooks,
  checkSkills,
  fail,
} from "./packaging-assets.ts";
import { envOr } from "./env.ts";

const WORKSPACE_PACKAGES = [
  "packages/toolu-core/package.json",
  "tools/toolu-opencode/package.json",
  "tools/toolu-conformance/package.json",
];

function releaseTracks(release: unknown, path: string): boolean {
  return list(get(release, "packages", ".", "extra-files")).some(
    (entry) =>
      get(entry, "type") === "json" &&
      get(entry, "path") === path &&
      get(entry, "jsonpath") === "$.version",
  );
}

/** The release-please TOML entries that keep the Cargo workspace in lockstep (#407). */
const CARGO_RELEASE = [
  { path: "Cargo.toml", jsonpath: "$.workspace.package.version" },
  { path: "Cargo.lock", jsonpath: "$.package[?(!@.source)].version" },
];

/**
 * `Cargo.toml`'s workspace version and every workspace crate in `Cargo.lock`
 * (the entries without a `source`) equal `version`, and release-please bumps both.
 */
function checkCargo(repo: Repo, version: string, release: unknown): void {
  for (const want of CARGO_RELEASE) {
    const tracked = list(get(release, "packages", ".", "extra-files")).some(
      (entry) =>
        get(entry, "type") === "toml" &&
        get(entry, "path") === want.path &&
        get(entry, "jsonpath") === want.jsonpath,
    );
    if (!tracked) fail(`release-please is missing ${want.path} (${want.jsonpath})`);
  }
  if (get(repo.toml("Cargo.toml"), "workspace", "package", "version") !== version) {
    fail("Cargo.toml [workspace.package] version differs from package.json");
  }
  const local = list(get(repo.toml("Cargo.lock"), "package")).filter(
    (pkg) => get(pkg, "source") === undefined,
  );
  if (local.length === 0) fail("Cargo.lock lists no workspace crate");
  for (const pkg of local) {
    if (get(pkg, "version") !== version) {
      fail(`Cargo.lock ${String(get(pkg, "name"))} version differs from package.json`);
    }
  }
}

/** Every symlink under the plugin must resolve inside it. */
function checkSymlinks(repo: Repo, pluginRoot: string, name: string): void {
  const rootReal = realpathSync(repo.path(pluginRoot));
  const walk = (dir: string): void => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isSymbolicLink()) {
        const target = readlinkSync(path);
        const resolved = target.startsWith("/")
          ? target
          : resolve(realpathSync(dirname(path)), target);
        if (!resolved.startsWith(`${rootReal}/`)) {
          fail(
            `${name} symlink escapes plugin root: ${path.slice(repo.root.length + 1)} -> ${target}`,
          );
        }
      } else if (entry.isDirectory()) {
        walk(path);
      }
    }
  };
  walk(repo.path(pluginRoot));
}

/** The identity fields Claude and Codex manifests must agree on. */
function identity(doc: unknown): string {
  return JSON.stringify(["name", "version", "description"].map((key) => get(doc, key) ?? null));
}

function checkManifests(
  repo: Repo,
  pluginRoot: string,
  name: string,
  version: string,
  release: unknown,
): void {
  const claudePath = `${pluginRoot}/.claude-plugin/plugin.json`;
  const codexPath = `${pluginRoot}/.codex-plugin/plugin.json`;
  if (!existsSync(repo.path(codexPath))) fail(`${name} is missing its Codex manifest`);
  checkSymlinks(repo, pluginRoot, name);
  const claude = repo.json(claudePath);
  const codex = repo.json(codexPath);
  if (identity(claude) !== identity(codex))
    fail(`${name} has mismatched Claude/Codex identity metadata`);
  if (get(claude, "version") !== version)
    fail(`${name} Claude manifest version differs from package.json`);
  if (get(codex, "version") !== version)
    fail(`${name} Codex manifest version differs from package.json`);
  for (const manifest of [claudePath, codexPath]) {
    if (!releaseTracks(release, manifest)) fail(`release-please is missing ${manifest}`);
  }
  const declares = (key: string): boolean =>
    typeof codex === "object" && codex !== null && key in codex;
  if (existsSync(repo.path(`${pluginRoot}/skills`))) {
    if (get(codex, "skills") !== "./skills/") fail(`${name} must declare ./skills/`);
  } else if (declares("skills")) fail(`${name} declares skills without a skills directory`);
  if (existsSync(repo.path(`${pluginRoot}/hooks/hooks.json`))) {
    if (get(codex, "hooks") !== "./hooks/hooks.json")
      fail(`${name} must declare ./hooks/hooks.json`);
  } else if (declares("hooks")) fail(`${name} declares hooks without hooks/hooks.json`);
}

function checkCatalogs(
  name: string,
  claudeManifest: unknown,
  claudeCatalog: unknown,
  codexCatalog: unknown,
): void {
  const claudeEntries = list(get(claudeCatalog, "plugins")).filter((p) => get(p, "name") === name);
  if (claudeEntries.length === 0 || get(claudeEntries[0], "source") !== `./plugins/${name}`) {
    fail(`${name} is missing or has the wrong Claude marketplace source`);
  }
  if (claudeEntries.length !== 1)
    fail(`${name} must appear exactly once in the Claude marketplace`);
  if (get(claudeEntries[0], "description") !== get(claudeManifest, "description")) {
    fail(`${name} marketplace description differs from its manifest`);
  }
  const codexEntries = list(get(codexCatalog, "plugins")).filter((p) => get(p, "name") === name);
  const entry = codexEntries[0];
  if (entry === undefined) fail(`${name} is missing from the Codex marketplace`);
  if (codexEntries.length !== 1) fail(`${name} must appear exactly once in the Codex marketplace`);
  const rules: ReadonlyArray<readonly [string[], string, string]> = [
    [["source", "source"], "local", "Codex marketplace source must be local"],
    [["source", "path"], `./plugins/${name}`, "Codex marketplace source path is wrong"],
    [["policy", "installation"], "AVAILABLE", "Codex marketplace installation policy is wrong"],
    [
      ["policy", "authentication"],
      "ON_INSTALL",
      "Codex marketplace authentication policy is wrong",
    ],
    [["category"], "Productivity", "Codex marketplace category is wrong"],
  ];
  for (const [keys, want, message] of rules) {
    if (get(entry, ...keys) !== want) fail(`${name} ${message}`);
  }
}

function checkPlugins(repo: Repo, version: string): number {
  const claudeCatalog = repo.json(".claude-plugin/marketplace.json");
  const codexCatalog = repo.json(".agents/plugins/marketplace.json");
  const release = repo.json("release-please-config.json");
  let count = 0;
  for (const pluginRoot of repo.dirs("plugins")) {
    if (!existsSync(repo.path(`${pluginRoot}/.claude-plugin/plugin.json`))) continue;
    const name = pluginRoot.slice("plugins/".length);
    checkManifests(repo, pluginRoot, name, version, release);
    checkCatalogs(
      name,
      repo.json(`${pluginRoot}/.claude-plugin/plugin.json`),
      claudeCatalog,
      codexCatalog,
    );
    count += 1;
  }
  for (const pkg of WORKSPACE_PACKAGES) {
    if (get(repo.json(pkg), "version") !== version)
      fail(`${pkg} version differs from package.json`);
    if (!releaseTracks(release, pkg)) fail(`release-please is missing ${pkg}`);
  }
  checkCargo(repo, version, release);
  if (count !== EXPECTED.plugins)
    fail(`expected ${String(EXPECTED.plugins)} plugins, found ${String(count)}`);
  if (list(get(claudeCatalog, "plugins")).length !== count) {
    fail("Claude marketplace count does not match plugin manifests");
  }
  if (list(get(codexCatalog, "plugins")).length !== count) {
    fail("Codex marketplace count does not match plugin manifests");
  }
  return count;
}

function main(): number {
  const repo = new Repo(envOr("PACKAGING_ROOT", resolve(import.meta.dir, "../..")));
  try {
    const version = get(repo.json("package.json"), "version");
    if (typeof version !== "string") fail("package.json must contain a version");
    const plugins = checkPlugins(repo, version);
    const skills = checkSkills(repo);
    const agents = checkAgents(repo);
    const hooks = checkHooks(repo);
    process.stdout.write(
      `validate-plugin-packaging: validated ${String(plugins)} plugins, ${String(skills)} skills, ${String(agents)} agents, and ${String(hooks)} hook manifests\n`,
    );
    return 0;
  } catch (err: unknown) {
    if (!(err instanceof PackagingError)) throw err;
    // stdout too, so a test harness that only shows stdout still surfaces the reason.
    process.stdout.write(`validate-plugin-packaging: ${err.message}\n`);
    console.error(`validate-plugin-packaging: ${err.message}`);
    return 1;
  }
}

if (import.meta.main) process.exitCode = main();
