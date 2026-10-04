import { expect, test } from "bun:test";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { STALE_CLAIMS, TARGET_DOCS, staleClaims } from "../check-opencode-docs.ts";
import { MatrixSchema, readJson, type Matrix } from "../opencode-host/schema.ts";

// Real-data checks for tooling/src/check-opencode-docs.ts (#363): the committed
// docs pass, and each mutated copy fails with its named message.

const ROOT = resolve(import.meta.dir, "../../..");
const CHECK = join(ROOT, "tooling/src/check-opencode-docs.ts");
const CONTRACT = join(ROOT, "tools/toolu-opencode/contract");
const INSTALL_DOC = join(ROOT, "docs/opencode.md");

type Mutations = {
  matrix?: (m: Matrix) => void;
  doc?: (doc: string) => string;
  /** Edits sandbox copies of the target docs, given the copy root. */
  targets?: (root: string) => void;
};

function mirrorTargets(root: string): void {
  for (const rel of [...TARGET_DOCS, "docs/opencode-migration.md"]) {
    mkdirSync(dirname(join(root, rel)), { recursive: true });
    copyFileSync(join(ROOT, rel), join(root, rel));
  }
}

async function check(sb: Sandbox, m: Mutations = {}, args: string[] = []): Promise<RunResult> {
  const dir = sb.path("contract");
  mkdirSync(dir, { recursive: true });
  for (const file of ["pin.json", "probe-results.json"])
    copyFileSync(join(CONTRACT, file), join(dir, file));
  const matrix = readJson(join(CONTRACT, "capability-matrix.json"), MatrixSchema);
  m.matrix?.(matrix);
  writeFileSync(join(dir, "capability-matrix.json"), JSON.stringify(matrix));
  const doc = sb.write("opencode.md", (m.doc ?? ((d) => d))(readFileSync(INSTALL_DOC, "utf8")));
  const docsRoot = sb.path("docs-root");
  mirrorTargets(docsRoot);
  m.targets?.(docsRoot);
  return run([process.execPath, CHECK, ...args], {
    cwd: ROOT,
    env: { TOOLU_OPENCODE_CONTRACT_DIR: dir, TOOLU_OPENCODE_DOC: doc, TOOLU_DOCS_ROOT: docsRoot },
  });
}

test.concurrent("the committed docs, matrix and acceptance registry agree", async () => {
  using sb = createSandbox({});
  const res = await check(sb);
  expect(res.stderr).toBe("");
  expect(res.stdout).toBe("check-opencode-docs: ok\n");
  expect(res.exitCode).toBe(0);
});

test.concurrent("the support section lists all 16 plugins with a status and their CI checks", () => {
  const doc = readFileSync(INSTALL_DOC, "utf8");
  const section = doc.slice(doc.indexOf("<!-- opencode-support:start -->"));
  const rows = section.split("\n").filter((line) => /^\| [a-z0-9-]+ \| Supported/.test(line));
  expect(rows).toHaveLength(16);
  expect(section).toContain("| context7 | Supported | ");
  expect(section).toContain("`docs.quickstart`");
  expect(section).toContain("| statusline | Supported with limitations |");
});

/** The committed doc with one support row's status changed by hand. */
function edited(doc: string): string {
  return doc.replace("| jira | Supported |", "| jira | Supported with limitations |");
}

test.concurrent("an edited support row is stale, and --write restores it", async () => {
  using stale = createSandbox({});
  const res = await check(stale, { doc: edited });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toBe(
    "check-opencode-docs: support block is stale; run bun run check:opencode-docs --write\n",
  );
  using rewritten = createSandbox({});
  const written = await check(rewritten, { doc: edited }, ["--write"]);
  expect(written.exitCode).toBe(0);
  expect(readFileSync(rewritten.path("opencode.md"), "utf8")).toBe(
    readFileSync(INSTALL_DOC, "utf8"),
  );
});

test.concurrent("a matrix change makes the committed section stale", async () => {
  using sb = createSandbox({});
  const res = await check(sb, {
    matrix: (m) => {
      const row = m.plugins.statusline;
      if (row === undefined) throw new Error("no statusline row");
      delete row.notes;
    },
  });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain("support block is stale");
});

test.concurrent("a catalog plugin with no dedicated actual-host check fails", async () => {
  using sb = createSandbox({});
  const res = await check(sb, {
    matrix: (m) => {
      const jira = m.plugins.jira;
      if (jira === undefined) throw new Error("no jira row");
      m.plugins.ghost = jira;
    },
  });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toBe("check-opencode-docs: no dedicated actual-host check for: ghost\n");
});

test.concurrent("every V2-only claim on the install path fails with file:line; the migration guide is exempt", async () => {
  const samples: Array<[string, string]> = [
    ["opencode plugin add @toolu/opencode", "V2 plugin command (opencode plugin add)"],
    ["pin @opencode/plugin here", "V2 package (@opencode/plugin)"],
    ["OpenCode v2.0.12", "V2 pin (2.0.12)"],
    ["see https://opencode.ai/v2/docs/build/plugins", "V2 docs (opencode.ai/v2/)"],
    ["wires permission.evaluate", "V2 permission hook (permission.evaluate)"],
    ["the opencode-ai@1.18.31 line", "superseded pin (1.18.31)"],
    ['point skills.paths at "generated/skills"', "manual surface wiring (generated/skills)"],
  ];
  expect(samples).toHaveLength(STALE_CLAIMS.length);
  const text = samples.map(([line]) => line).join("\n");
  using sb = createSandbox({});
  const root = sb.path("docs-root");
  mirrorTargets(root);
  expect(staleClaims(root)).toEqual([]);
  const migration = join(root, "docs/opencode-migration.md");
  writeFileSync(migration, `${readFileSync(migration, "utf8")}\n${text}\n`);
  expect(staleClaims(root)).toEqual([]);
  const rel = "tools/toolu-opencode/README.md";
  const before = readFileSync(join(root, rel), "utf8");
  const first = before.split("\n").length;
  writeFileSync(join(root, rel), `${before}${text}\n`);
  expect(staleClaims(root)).toEqual(
    samples.map(([, label], index) => `${rel}:${first + index}: ${label}`),
  );

  using spawned = createSandbox({});
  const res = await check(spawned, {
    targets: (copy) => {
      const readme = join(copy, "README.md");
      writeFileSync(readme, `${readFileSync(readme, "utf8")}wires permission.evaluate\n`);
    },
  });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toMatch(/\nREADME\.md:\d+: V2 permission hook \(permission\.evaluate\)\n$/);
});
