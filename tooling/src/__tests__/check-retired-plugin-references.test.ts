/** The retired-plugin reference gate runs against a real temporary Git index. */
import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { run } from "@toolu/conformance/harness/spawn";

const CHECKER = resolve(import.meta.dir, "../check-retired-plugin-references.ts");
// Split these names so the checked test source does not need its own exception.
const RETIRED_NAME = ["context", "7"].join("");
const TRACKER_NAME = ["ji", "ra"].join("");

function write(root: string, path: string, content: string): void {
  const target = join(root, path);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function stage(root: string): void {
  const result = spawnSync("git", ["add", "-A"], { cwd: root, encoding: "utf8" });
  expect(result.status).toBe(0);
}

test("the reference gate accepts reviewed history and rejects a current reference", async () => {
  const root = mkdtempSync(join(tmpdir(), "toolu-retired-refs-"));
  try {
    const init = spawnSync("git", ["init", "-q"], { cwd: root, encoding: "utf8" });
    expect(init.status).toBe(0);
    const relativeChecker = "tooling/src/check-retired-plugin-references.ts";
    write(root, relativeChecker, readFileSync(CHECKER, "utf8"));
    write(root, `plugins/epic-orchestrator/scripts/trackers/${TRACKER_NAME}.ts`, "// tracker\n");
    write(
      root,
      "plugins/pr-babysit/scripts/__tests__/fixtures/pr120-verdict-changes.txt",
      `historical ${RETIRED_NAME} output\n`,
    );
    stage(root);
    const script = join(root, relativeChecker);
    const clean = await run([process.execPath, script], { cwd: root });
    expect(clean.exitCode).toBe(0);
    expect(clean.stdout).toContain("only reviewed history");

    write(root, "README.md", `current ${RETIRED_NAME} guidance\n`);
    stage(root);
    const stale = await run([process.execPath, script], { cwd: root });
    expect(stale.exitCode).not.toBe(0);
    expect(stale.stderr).toContain("README.md");

    write(root, "README.md", `See ${["plugins", TRACKER_NAME, "README.md"].join("/")}\n`);
    stage(root);
    const standalone = await run([process.execPath, script], { cwd: root });
    expect(standalone.exitCode).not.toBe(0);
    expect(standalone.stderr).toContain("standalone");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
