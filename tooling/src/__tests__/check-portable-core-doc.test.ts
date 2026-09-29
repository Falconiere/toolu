import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";

// Real-data checks for tooling/src/check-portable-core-doc.ts (#205).

const ROOT = resolve(import.meta.dir, "../../..");
const CHECK = join(ROOT, "tooling/src/check-portable-core-doc.ts");
const DOC = join(ROOT, "docs/portable-core.md");

function checkDoc(doc: string, cwd: string): ReturnType<typeof run> {
  return run([process.execPath, CHECK], { cwd, env: { PORTABLE_CORE_DOC: doc } });
}

test.concurrent("portable-core doc checker passes on the committed doc", async () => {
  const res = await run([process.execPath, CHECK], {
    cwd: ROOT,
    env: { PORTABLE_CORE_DOC: undefined },
  });
  expect(res.exitCode).toBe(0);
  expect(res.stdout + res.stderr).toContain("ok");
});

test.concurrent("portable-core doc checker fails when a required heading is removed", async () => {
  using sb = createSandbox();
  const original = await Bun.file(DOC).text();
  const kept = original.split("\n").filter((line) => line !== "## Pins");
  const doc = sb.write("portable-core.md", kept.join("\n"));

  const res = await checkDoc(doc, ROOT);
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("missing heading");
});

test.concurrent("portable-core doc checker fails on invalid classification token maybe-later", async () => {
  using sb = createSandbox();
  const original = await Bun.file(DOC).text();
  const lines = original
    .split("\n")
    .flatMap((line) => (line.startsWith("## Policy split") ? [line, "- `maybe-later`"] : [line]));
  const doc = sb.write("portable-core.md", lines.join("\n"));

  const res = await checkDoc(doc, ROOT);
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("maybe-later");
});
