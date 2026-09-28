/**
 * `npx @toolu/plugins <verb>` must reach the registry from any directory, this
 * repository included, with no @latest tag.
 *
 * npm exec checks the local project tree before the registry. When the root
 * package or a declared workspace carries the requested name, npx treats it as
 * installed and runs its bin from node_modules/.bin, which here does not exist:
 * `sh: toolu: command not found`. The old @toolu/cli did exactly that from every
 * toolu checkout, because tools/toolu-cli was a workspace under that name.
 *
 * So the CLI publishes from tools/toolu-cli/npm, a folder no workspace declares,
 * and the workspace itself is private under another name.
 */
import { expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync, realpathSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";

// Physical path: Arborist reports a workspace twice when the project path and
// the workspace's realpath differ only by a symlink (macOS /var -> /private/var).
const ROOT = realpathSync(resolve(import.meta.dir, "../../.."));
const PUBLISH_DIR = join(ROOT, "tools/toolu-cli/npm");
const SLOW_MS = 240_000;
const HELP_FIRST_LINE = "npx @toolu/plugins <command> [options]";

/** Tracked markdown only: gitignored scratch specs are not published docs. CHANGELOG.md is release-please history and stays untouched. */
async function trackedMarkdown(): Promise<string[]> {
  const res = await run(["git", "-C", ROOT, "ls-files", "-z", "--", "*.md", ":!CHANGELOG.md"]);
  expect(res.exitCode).toBe(0);
  return res.stdout.split("\0").filter((file) => file !== "" && existsSync(join(ROOT, file)));
}

type Hit = { location: string; match: string };

/** Every match of `pattern` in the tracked markdown, as `file:line` plus the matched text (grep -noE). */
async function markdownHits(pattern: RegExp): Promise<Hit[]> {
  const files = await trackedMarkdown();
  return files.flatMap((file) =>
    readFileSync(join(ROOT, file), "utf8")
      .split("\n")
      .flatMap((line, index) =>
        [...line.matchAll(pattern)].map((m) => ({ location: `${file}:${index + 1}`, match: m[0] })),
      ),
  );
}

/**
 * The same lookup libnpmexec runs before it decides to install: Arborist's
 * loadActual over the project, then an inventory query by package name. Uses
 * the Arborist that ships inside npm, so it cannot drift from what npx does.
 * Resolves to the locations of every local package with that name, in JSON text.
 */
async function localMatches(name: string): Promise<{ exitCode: number; output: string }> {
  const root = await run(["npm", "root", "-g"]);
  expect(root.exitCode).toBe(0);
  const arborist = join(root.stdout.trim(), "npm/node_modules/@npmcli/arborist");
  const script = `
    const Arborist = require(process.argv[1]);
    new Arborist({ path: process.argv[2] }).loadActual().then((tree) => {
      const hits = [...tree.inventory.query("packageName", process.argv[3])];
      console.log(JSON.stringify(hits.map((node) => node.location)));
    });
  `;
  const res = await run(["node", "-e", script, arborist, ROOT, name], { timeoutMs: SLOW_MS });
  return { exitCode: res.exitCode, output: (res.stdout + res.stderr).trim() };
}

const PackageJson = z.object({ version: z.string() });

/** Pack the publish folder into the sandbox and return the `file:` spec npx installs. */
async function packTarball(sb: Sandbox, env: Record<string, string>): Promise<string> {
  const packed = await run(["npm", "pack", "--silent", "--pack-destination", sb.project], {
    cwd: PUBLISH_DIR,
    env,
    timeoutMs: SLOW_MS,
  });
  expect(packed.exitCode).toBe(0);
  const tgz = readdirSync(sb.project).find((f) => /^toolu-plugins-.*\.tgz$/.test(f));
  expect(tgz).toBeDefined();
  // A bare path is executed like a command; the file: spec makes npx install the
  // tarball and pick its bin the way it would for the registry package.
  return `file:${join(sb.project, tgz ?? "")}`;
}

test.concurrent("npm sees no local package named @toolu/plugins, so npx goes to the registry", async () => {
  // The probe must still find the dev workspace, or an empty answer proves nothing.
  const dev = await localMatches("toolu-cli");
  expect(dev.exitCode).toBe(0);
  expect(dev.output).toBe('["tools/toolu-cli"]');

  const published = await localMatches("@toolu/plugins");
  expect(published.exitCode).toBe(0);
  expect(published.output).toBe("[]");
}, 300_000);

test.concurrent("the packed tarball runs through npx with the verb first", async () => {
  using sb = createSandbox();
  // Isolated cache, and no update notice mixed into the captured output.
  const env = {
    npm_config_cache: sb.path("npm-cache"),
    npm_config_update_notifier: "false",
  };
  const spec = await packTarball(sb, env);
  const { version } = PackageJson.parse(
    JSON.parse(readFileSync(join(PUBLISH_DIR, "package.json"), "utf8")),
  );
  const npx = async (...args: string[]): Promise<{ exitCode: number; output: string }> => {
    const res = await run(["npx", "--yes", spec, ...args], {
      cwd: sb.project,
      env,
      timeoutMs: SLOW_MS,
    });
    return { exitCode: res.exitCode, output: (res.stdout + res.stderr).replace(/\n+$/, "") };
  };

  const versionRun = await npx("--version");
  expect(versionRun.exitCode).toBe(0);
  expect(versionRun.output).toBe(version);

  const help = await npx("--help");
  expect(help.exitCode).toBe(0);
  expect(help.output.split("\n")[0]).toBe(HELP_FIRST_LINE);

  // No command at all prints the same help, but like a usage error.
  const bare = await npx();
  expect(bare.exitCode).toBe(2);
  expect(bare.output.split("\n")[0]).toBe(HELP_FIRST_LINE);

  // The noun grammar of the old @toolu/cli is gone: `plugins` is not a verb,
  // and the error says what to type instead.
  const noun = await npx("plugins", "install");
  expect(noun.exitCode).toBe(2);
  expect(noun.output).toBe(
    "toolu: unknown command: plugins. The package name already says plugins: run `npx @toolu/plugins install`",
  );
}, 300_000);

test.concurrent("documented npx invocations use the bare @toolu/plugins name", async () => {
  const found = await markdownHits(/npx( +-[A-Za-z0-9-]+)* +@toolu\/(cli|plugins)[^\s`]*/g);
  // The pattern must still see the documented commands, or this test is vacuous.
  expect(found.length).toBeGreaterThan(0);
  const offenders = found
    .filter((hit) => hit.match !== "npx @toolu/plugins")
    .map((hit) => `${hit.location}:${hit.match}`);
  // document npx @toolu/plugins <verb>, with no tag, version, or old name
  expect(offenders).toEqual([]);
});

test.concurrent("no documented npx invocation uses the unscoped toolu package", async () => {
  const offenders = await markdownHits(/npx( +-[A-Za-z0-9-]+)* +toolu(?=[\s`]|$)/g);
  // the unscoped toolu package does not exist on npm; use npx @toolu/plugins
  expect(offenders.map((hit) => `${hit.location}:${hit.match}`)).toEqual([]);
});
