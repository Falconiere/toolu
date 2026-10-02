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
  expect({ exitCode: res.exitCode, stdout: res.stdout }).toEqual({
    exitCode: 0,
    stdout: "check-portable-core-doc: ok\n",
  });
});

test.concurrent("portable-core doc checker fails when a required heading is removed", async () => {
  using sb = createSandbox();
  const original = await Bun.file(DOC).text();
  const kept = original.split("\n").filter((line) => line !== "## Pins");
  const doc = sb.write("portable-core.md", kept.join("\n"));

  const res = await checkDoc(doc, ROOT);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({
    exitCode: 1,
    stderr: "check-portable-core-doc: missing heading: ## Pins\n",
  });
});

test.concurrent("portable-core doc checker fails on invalid classification token maybe-later", async () => {
  using sb = createSandbox();
  const original = await Bun.file(DOC).text();
  const lines = original
    .split("\n")
    .flatMap((line) => (line.startsWith("## Policy split") ? [line, "- `maybe-later`"] : [line]));
  const doc = sb.write("portable-core.md", lines.join("\n"));

  const res = await checkDoc(doc, ROOT);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({
    exitCode: 1,
    stderr: "check-portable-core-doc: invalid classification token maybe-later\n",
  });
});

test.concurrent("portable-core doc checker rejects a citation of the superseded V2 plugin docs", async () => {
  using sb = createSandbox();
  const original = await Bun.file(DOC).text();
  const doc = sb.write(
    "portable-core.md",
    `${original}\nSource: https://opencode.ai/v2/docs/build/plugins\n`,
  );

  const res = await checkDoc(doc, ROOT);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({
    exitCode: 1,
    stderr: "check-portable-core-doc: cites the V2 contract (opencode.ai/v2/)\n",
  });
});

test.concurrent("portable-core doc checker requires the documented SDK pin", async () => {
  using sb = createSandbox();
  const original = await Bun.file(DOC).text();
  const doc = sb.write(
    "portable-core.md",
    original.replaceAll("@opencode-ai/plugin@1.18.34", "@opencode-ai/plugin"),
  );

  const res = await checkDoc(doc, ROOT);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({
    exitCode: 1,
    stderr: "check-portable-core-doc: missing SDK pin\n",
  });
});
