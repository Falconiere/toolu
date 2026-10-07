import { expect, test } from "bun:test";
import { cpSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";

// `check:ci-paths` (#458) on this checkout and on copies of its real .github
// tree, each mutated one way.

const ROOT = resolve(import.meta.dir, "../../../..");
const SCRIPT = join(ROOT, "tooling/src/check-ci-paths.ts");

type Mutation = (dir: string) => void;

function edit(dir: string, rel: string, change: (text: string) => string): void {
  const path = join(dir, rel);
  const before = readFileSync(path, "utf8");
  const after = change(before);
  if (after === before) throw new Error(`mutation left ${rel} unchanged`);
  writeFileSync(path, after);
}

async function check(mutate?: Mutation): Promise<{ exitCode: number; out: string }> {
  using sb = createSandbox();
  const dir = join(sb.root, "github");
  cpSync(join(ROOT, ".github"), dir, { recursive: true });
  mutate?.(dir);
  const res = await run([process.execPath, SCRIPT, "--github", dir], { cwd: ROOT });
  return { exitCode: res.exitCode, out: res.stdout + res.stderr };
}

test.concurrent("the repository's workflows and path groups agree (AC-7)", async () => {
  const res = await run([process.execPath, SCRIPT], { cwd: ROOT });
  expect({ exitCode: res.exitCode, out: res.stdout + res.stderr }).toEqual({
    exitCode: 0,
    out: "check:ci-paths: workflows and path groups agree\n",
  });
});

test.concurrent("an unmutated copy of .github passes", async () => {
  expect((await check()).exitCode).toBe(0);
});

test.concurrent("paths-ignore on a workflow that reports a required check fails (AC-7)", async () => {
  const res = await check((dir) =>
    edit(dir, "workflows/tests.yml", (text) =>
      text.replace(
        "  pull_request:\n    branches: [main]\n",
        '  pull_request:\n    branches: [main]\n    paths-ignore: ["docs/**"]\n',
      ),
    ),
  );
  expect(res).toEqual({
    exitCode: 1,
    out: "check:ci-paths: tests.yml: reports a required check but filters pull_request by paths\n",
  });
});

test.concurrent("deleting a job's group from the data file fails (AC-5)", async () => {
  const res = await check((dir) =>
    edit(dir, "ci-paths.json", (text) => text.replace('        "docs": "docs",\n', "")),
  );
  expect(res.exitCode).toBe(1);
  expect(res.out).toContain(
    "tests.yml: job docs reads needs.changes.outputs but has no group in the data file",
  );
  expect(res.out).toContain(
    "tests.yml: aggregate gate needs [changes, ts, opencode, docs, rust, rust-musl, fuzz, rust-conformance, hook-bench], expected [changes, ts, opencode, rust, rust-musl, fuzz, rust-conformance, hook-bench]",
  );
});

test.concurrent("an aggregate whose needs miss a gated job fails (AC-7)", async () => {
  const res = await check((dir) =>
    edit(dir, "workflows/tests.yml", (text) =>
      text.replace(
        "needs: [changes, ts, opencode, docs, rust, rust-musl, fuzz, rust-conformance, hook-bench]",
        "needs: [changes, ts, opencode, rust, rust-musl, fuzz, rust-conformance, hook-bench]",
      ),
    ),
  );
  expect(res).toEqual({
    exitCode: 1,
    out: "check:ci-paths: tests.yml: aggregate gate needs [changes, ts, opencode, rust, rust-musl, fuzz, rust-conformance, hook-bench], expected [changes, ts, opencode, docs, rust, rust-musl, fuzz, rust-conformance, hook-bench]\n",
  });
});

test.concurrent("a glob that matches no tracked file fails (AC-7)", async () => {
  const res = await check((dir) =>
    edit(dir, "ci-paths.json", (text) =>
      text.replace('"docs": ["docs/**",', '"docs": ["no-such-dir/**", "docs/**",'),
    ),
  );
  expect(res).toEqual({
    exitCode: 1,
    out: "check:ci-paths: ci-paths.json: no-such-dir/** matches no tracked file\n",
  });
});

test.concurrent("a gated job reading another group fails (AC-7)", async () => {
  const res = await check((dir) =>
    edit(dir, "workflows/tests.yml", (text) =>
      text.replace(
        "if: needs.changes.outputs.ts == 'true'",
        "if: needs.changes.outputs.docs == 'true'",
      ),
    ),
  );
  expect(res).toEqual({
    exitCode: 1,
    out: "check:ci-paths: tests.yml: job ts is not gated on needs.changes.outputs.ts\n",
  });
});

test.concurrent("a required review check that loses its job fails", async () => {
  const res = await check((dir) =>
    edit(dir, "ci-paths.json", (text) =>
      text.replace('"required": ["review", "merge-gate"]', '"required": ["reviews", "merge-gate"]'),
    ),
  );
  expect(res).toEqual({
    exitCode: 1,
    out: "check:ci-paths: toolu-review.yml: required check reviews is not a job\n",
  });
});
