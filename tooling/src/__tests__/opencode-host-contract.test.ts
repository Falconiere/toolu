import { expect, test } from "bun:test";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import {
  isUsed,
  MatrixSchema,
  ProbeResultsSchema,
  PinSchema,
  readJson,
  type Cell,
  type Matrix,
  type ProbeResults,
  type UsedCell,
} from "../opencode-host/schema.ts";

// Real-data checks for tooling/src/opencode-host-contract.ts (#335): the committed
// contract passes, and each mutated copy fails with its named message.

const ROOT = resolve(import.meta.dir, "../../..");
const CHECK = join(ROOT, "tooling/src/opencode-host-contract.ts");
const CONTRACT = join(ROOT, "tools/toolu-opencode/contract");
const DOC = join(ROOT, "docs/opencode-host-contract.md");
const pin = readJson(join(CONTRACT, "pin.json"), PinSchema);
const AdapterManifest = z.looseObject({
  devDependencies: z.record(z.string(), z.string()),
  peerDependencies: z.record(z.string(), z.string()),
});
type AdapterManifest = z.infer<typeof AdapterManifest>;

type Mutations = {
  matrix?: (m: Matrix) => void;
  results?: (r: ProbeResults) => void;
  doc?: (doc: string) => string;
  /** Edits a copy of the adapter's real package.json. */
  adapter?: (pkg: AdapterManifest) => void;
  plugins?: (dir: string) => void;
};

function row(m: Matrix, name: string): Matrix["plugins"][string] {
  const found = m.plugins[name];
  if (found === undefined) throw new Error(`no matrix row ${name}`);
  return found;
}

function used(cell: Cell): UsedCell {
  if (!isUsed(cell)) throw new Error("expected a used cell");
  return cell;
}

/** Copy only what the checker reads from each plugin: manifests, hooks and surface files. */
function mirrorPlugins(target: string): void {
  const source = join(ROOT, "plugins");
  for (const name of readdirSync(source)) {
    const files = [".claude-plugin/plugin.json", "hooks/hooks.json", "hooks/src/register.ts"];
    for (const kind of ["skills", "commands", "agents"]) {
      const dir = join(source, name, kind);
      if (!existsSync(dir)) continue;
      for (const entry of readdirSync(dir)) {
        files.push(kind === "skills" ? `${kind}/${entry}/SKILL.md` : `${kind}/${entry}`);
      }
    }
    for (const rel of files) {
      const from = join(source, name, rel);
      if (!existsSync(from)) continue;
      mkdirSync(dirname(join(target, name, rel)), { recursive: true });
      copyFileSync(from, join(target, name, rel));
    }
  }
}

async function check(sb: Sandbox, m: Mutations = {}, args: string[] = []): Promise<RunResult> {
  const dir = sb.path("contract");
  mkdirSync(dir, { recursive: true });
  copyFileSync(join(CONTRACT, "pin.json"), join(dir, "pin.json"));
  const matrix = readJson(join(CONTRACT, "capability-matrix.json"), MatrixSchema);
  m.matrix?.(matrix);
  writeFileSync(join(dir, "capability-matrix.json"), JSON.stringify(matrix));
  const results = readJson(join(CONTRACT, "probe-results.json"), ProbeResultsSchema);
  m.results?.(results);
  writeFileSync(join(dir, "probe-results.json"), JSON.stringify(results));
  const doc = sb.write("contract.md", (m.doc ?? ((d) => d))(readFileSync(DOC, "utf8")));
  const env: Record<string, string> = {
    TOOLU_OPENCODE_CONTRACT_DIR: dir,
    TOOLU_OPENCODE_CONTRACT_DOC: doc,
  };
  if (m.adapter !== undefined) {
    const pkg = AdapterManifest.parse(
      JSON.parse(readFileSync(join(ROOT, "tools/toolu-opencode/package.json"), "utf8")),
    );
    m.adapter(pkg);
    env.TOOLU_OPENCODE_ADAPTER_PKG = sb.write("adapter/package.json", pkg);
  }
  if (m.plugins !== undefined) {
    mirrorPlugins(sb.path("plugins"));
    m.plugins(sb.path("plugins"));
    env.TOOLU_PLUGINS_DIR = sb.path("plugins");
  }
  return run([process.execPath, CHECK, ...args], { cwd: ROOT, env });
}

async function expectFailure(m: Mutations, message: string): Promise<void> {
  using sb = createSandbox();
  const res = await check(sb, m);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({
    exitCode: 1,
    stderr: `opencode-host-contract: ${message}\n`,
  });
}

test.concurrent("the committed contract, evidence, matrix and doc agree", async () => {
  const res = await run([process.execPath, CHECK], { cwd: ROOT, env: {} });
  expect({ exitCode: res.exitCode, stdout: res.stdout, stderr: res.stderr }).toEqual({
    exitCode: 0,
    stdout: "opencode-host-contract: ok\n",
    stderr: "",
  });
});

test.concurrent("a catalog plugin without a matrix row fails", () =>
  expectFailure({ matrix: (m) => void delete m.plugins.jira }, "missing matrix row: jira"));

test.concurrent("a new plugins/<name> with a manifest fails until the matrix covers it", () =>
  expectFailure(
    {
      plugins: (dir) => {
        mkdirSync(join(dir, "newplug/.claude-plugin"), { recursive: true });
        writeFileSync(join(dir, "newplug/.claude-plugin/plugin.json"), "{}");
      },
    },
    "missing matrix row: newplug",
  ));

test.concurrent("a matrix row for a plugin that does not exist fails", async () => {
  await expectFailure(
    { matrix: (m) => void (m.plugins.ghost = structuredClone(row(m, "brainstorm"))) },
    "unknown plugin: ghost",
  );
});

test.concurrent("evidence naming a probe that does not exist fails schema validation", async () => {
  using sb = createSandbox();
  const res = await check(sb, {
    matrix: (m) => void Reflect.set(used(row(m, "toolu").axes.tools), "evidence", ["deny.nope"]),
  });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toStartWith(
    `opencode-host-contract: ${sb.path("contract/capability-matrix.json")}: `,
  );
  expect(res.stderr).toContain("at plugins.toolu.axes.tools");
});

test.concurrent("a status the evidence contradicts fails", () =>
  expectFailure(
    { matrix: (m) => void (used(row(m, "toolu").axes.tools).status = "unsupported") },
    "status unsupported contradicts evidence for toolu.tools",
  ));

test.concurrent("a required unsupported cell without alternative or blocker fails", () =>
  expectFailure(
    {
      matrix: (m) => {
        const cell = used(row(m, "ast-grep").axes.tools);
        delete cell.alternative;
        delete cell.alternativeEvidence;
      },
    },
    "required ast-grep.tools is unsupported without alternative or releaseBlocker",
  ));

test.concurrent("an enforcement cell with only a prose alternative fails", () =>
  expectFailure(
    { matrix: (m) => void delete used(row(m, "toolu").axes.permission).alternativeEvidence },
    "enforcement toolu.permission needs supported alternativeEvidence or releaseBlocker",
  ));

test.concurrent("an axis the plugin's manifests need cannot be marked none", () =>
  expectFailure(
    { matrix: (m) => void (row(m, "jev").axes.prompt = { use: "none" }) },
    "jev.prompt: manifests need it but the matrix says none",
  ));

test.concurrent("surface counts must match the plugin's files", () =>
  expectFailure(
    { matrix: (m) => void (row(m, "toolu").surfaces.agents = 4) },
    "toolu.surfaces.agents is 4, plugins/toolu has 5",
  ));

test.concurrent("an unsupported probe must be owned by some matrix entry", () =>
  expectFailure(
    { matrix: (m) => void (m.host = m.host.filter((c) => !c.evidence.includes("surface.names"))) },
    "unsupported probe surface.names has no owner in the matrix",
  ));

test.concurrent("results must hold every probe exactly once", () =>
  expectFailure(
    { results: (r) => void (r.probes = r.probes.filter((p) => p.id !== "deny.mcp")) },
    "probe results must contain deny.mcp exactly once (found 0)",
  ));

test.concurrent("the adapter devDependency must equal the SDK pin", () =>
  expectFailure(
    { adapter: (pkg) => void (pkg.devDependencies["@opencode-ai/plugin"] = "1.18.33") },
    `pin mismatch: @toolu/opencode devDependency @opencode-ai/plugin is 1.18.33, pin is ${pin.sdk.version}`,
  ));

test.concurrent("the adapter's optional SDK peers must equal the SDK pin", () =>
  expectFailure(
    { adapter: (pkg) => void (pkg.peerDependencies["@opencode-ai/sdk"] = "1.18.33") },
    `pin mismatch: @toolu/opencode peerDependency @opencode-ai/sdk is 1.18.33, pin is ${pin.sdk.version}`,
  ));

test.concurrent("an adapter without the SDK peers fails the pin check", () =>
  expectFailure(
    { adapter: (pkg) => void delete pkg.peerDependencies["@opencode-ai/plugin"] },
    `pin mismatch: @toolu/opencode peerDependency @opencode-ai/plugin is absent, pin is ${pin.sdk.version}`,
  ));

test.concurrent("a hand-edited generated block is stale", () =>
  expectFailure(
    { doc: (d) => d.replace("| toolu |", "| toolu (edited) |") },
    "doc block matrix is stale; run bun run check:opencode-host --write-doc",
  ));

test.concurrent("every declared Hooks member must appear in the host surface section", () =>
  expectFailure(
    { doc: (d) => d.replace(/^\| `shell\.env` .*\n/m, "") },
    "host surface misses hook shell.env",
  ));

test.concurrent("the contract doc must not cite the V2 plugin docs", () =>
  expectFailure(
    { doc: (d) => `${d}\nSee https://opencode.ai/v2/docs/build/plugins\n` },
    "docs/opencode-host-contract.md cites the V2 contract (opencode.ai/v2/)",
  ));

test.concurrent("--write-doc regenerates stale blocks and lists release blockers", async () => {
  using sb = createSandbox();
  const res = await check(
    sb,
    {
      doc: (d) => d.replace("| toolu |", "| toolu (edited) |"),
      matrix: (m) => {
        const cell = used(row(m, "ast-grep").axes.tools);
        delete cell.alternative;
        delete cell.alternativeEvidence;
        cell.releaseBlocker = true;
      },
    },
    ["--write-doc"],
  );
  expect({ exitCode: res.exitCode, stdout: res.stdout }).toEqual({
    exitCode: 0,
    stdout: "opencode-host-contract: ok\n",
  });
  const doc = sb.read("contract.md");
  expect(doc).toContain("| toolu |");
  expect(doc).toContain("- `ast-grep.tools` (unsupported) — search-nudge");
});

test.concurrent("results recorded on another CLI version fail the pin check", () =>
  expectFailure(
    { results: (r) => void (r.host.cliVersion = "1.18.33") },
    `pin mismatch: probe results recorded opencode-ai 1.18.33, pin is ${pin.cli.version}`,
  ));

test.concurrent("results whose host provisioned another SDK fail the pin check", () =>
  expectFailure(
    { results: (r) => void (r.host.provisionedSdkVersion = "1.18.33") },
    `pin mismatch: probe results recorded @opencode-ai/plugin 1.18.33, pin is ${pin.sdk.version}`,
  ));

/** Replace every experimental mechanism, leaving the matrix with no experimental hooks. */
function withoutExperimental(m: Matrix): void {
  for (const plugin of Object.values(m.plugins)) {
    for (const cell of Object.values(plugin.axes)) {
      if (isUsed(cell) && cell.mechanism.includes("experimental.")) cell.mechanism = "plugin init";
    }
  }
}

test.concurrent("a matrix without experimental hooks renders a stable limitations block", async () => {
  using sb = createSandbox();
  const written = await check(sb, { matrix: withoutExperimental }, ["--write-doc"]);
  expect(written.exitCode).toBe(0);
  expect(sb.read("contract.md")).toContain("### Experimental hooks under the exact pin\n\nNone.\n");
  const again = await run([process.execPath, CHECK], {
    cwd: ROOT,
    env: {
      TOOLU_OPENCODE_CONTRACT_DIR: sb.path("contract"),
      TOOLU_OPENCODE_CONTRACT_DOC: sb.path("contract.md"),
    },
  });
  expect({ exitCode: again.exitCode, stdout: again.stdout, stderr: again.stderr }).toEqual({
    exitCode: 0,
    stdout: "opencode-host-contract: ok\n",
    stderr: "",
  });
});
