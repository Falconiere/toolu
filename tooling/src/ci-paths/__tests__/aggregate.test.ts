import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";

// The `typescript` aggregate (#458) run the way CI runs it: the real script, the real
// data file, and NEEDS in GitHub's toJSON(needs) shape.

const ROOT = resolve(import.meta.dir, "../../../..");
const SCRIPT = join(ROOT, "tooling/src/ci-aggregate.ts");

type Job = { result: string; outputs?: Record<string, string> };

function changes(on: {
  ts: boolean;
  opencode: boolean;
  docs: boolean;
  rust?: boolean;
  ports?: boolean;
}): Job {
  const outputs = Object.fromEntries(
    Object.entries({ rust: false, ports: false, ...on }).map(([key, value]) => [
      key,
      String(value),
    ]),
  );
  return { result: "success", outputs: { ...outputs, changed: "true" } };
}

const SKIPPED: Job = { result: "skipped", outputs: {} };
const SUCCESS: Job = { result: "success", outputs: {} };

/** Runs the aggregate; the Rust jobs default to skipped (their groups are off unless a test turns them on). */
async function aggregate(needs: Record<string, Job>, workflow = "tests.yml") {
  const all = { rust: SKIPPED, "rust-musl": SKIPPED, "rust-conformance": SKIPPED, ...needs };
  const res = await run([process.execPath, SCRIPT, workflow], {
    cwd: ROOT,
    env: { NEEDS: JSON.stringify(all), CI_CHANGES_ROOT: ROOT },
  });
  return { exitCode: res.exitCode, out: res.stdout + res.stderr };
}

const DOCS_ONLY = changes({ ts: false, opencode: false, docs: true });
const EVERYTHING = changes({ ts: true, opencode: true, docs: true });

test.concurrent("a docs-only run passes with gate and opencode skipped (AC-1)", async () => {
  const res = await aggregate({
    changes: DOCS_ONLY,
    gate: { result: "skipped", outputs: {} },
    opencode: { result: "skipped", outputs: {} },
    docs: { result: "success", outputs: {} },
  });
  expect(res.exitCode).toBe(0);
  expect(res.out).toContain("gate (ts off): skipped");
  expect(res.out).toContain("docs (docs on): success");
});

test.concurrent("a release-only run passes with every gated job skipped (AC-2)", async () => {
  const off = {
    result: "success",
    outputs: {
      ts: "false",
      opencode: "false",
      docs: "false",
      rust: "false",
      ports: "false",
      changed: "false",
    },
  };
  const skipped = { result: "skipped", outputs: {} };
  const res = await aggregate({ changes: off, gate: skipped, opencode: skipped, docs: skipped });
  expect(res.exitCode).toBe(0);
});

test.concurrent("a failed changes job fails the aggregate and names it (AC-5)", async () => {
  const skipped = { result: "skipped", outputs: {} };
  const res = await aggregate({
    changes: { result: "failure", outputs: {} },
    gate: skipped,
    opencode: skipped,
    docs: skipped,
  });
  expect(res).toEqual({ exitCode: 1, out: "changes: failure; no group decision to trust\n" });
});

for (const result of ["failure", "cancelled"]) {
  test.concurrent(`a needed job that ends ${result} fails the aggregate (AC-5)`, async () => {
    const res = await aggregate({
      changes: EVERYTHING,
      gate: { result: "success", outputs: {} },
      opencode: { result, outputs: {} },
      docs: { result: "success", outputs: {} },
    });
    expect(res).toEqual({ exitCode: 1, out: `opencode (opencode on): ${result}\n` });
  });
}

test.concurrent("a job skipped while its group is on fails the aggregate (AC-5)", async () => {
  const res = await aggregate({
    changes: EVERYTHING,
    gate: { result: "skipped", outputs: {} },
    opencode: { result: "success", outputs: {} },
    docs: { result: "success", outputs: {} },
  });
  expect(res).toEqual({ exitCode: 1, out: "gate (ts on): skipped, but its group is on\n" });
});

test.concurrent("needs that differ from the gated jobs fail the aggregate", async () => {
  const res = await aggregate({
    changes: EVERYTHING,
    gate: { result: "success", outputs: {} },
    opencode: { result: "success", outputs: {} },
    lint: { result: "success", outputs: {} },
  });
  expect(res.exitCode).toBe(1);
  expect(res.out).toBe("lint: needed but not mapped to a group\ndocs: missing from needs\n");
});

test.concurrent("missing or malformed NEEDS never passes", async () => {
  const res = await run([process.execPath, SCRIPT, "tests.yml"], {
    cwd: ROOT,
    env: { NEEDS: undefined },
  });
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({
    exitCode: 1,
    stderr: "ci-aggregate: NEEDS is not JSON\n",
  });
  expect(await aggregate({ changes: { result: "success" } }, "nope.yml")).toEqual({
    exitCode: 1,
    out: "ci-aggregate: nope.yml has no entry in the data file\n",
  });
});

test.concurrent("a changes job that succeeded without group outputs never passes (AC-5)", async () => {
  const skipped = { result: "skipped", outputs: {} };
  const res = await aggregate({
    changes: { result: "success", outputs: { ts: "", opencode: "false" } },
    gate: skipped,
    opencode: skipped,
    docs: skipped,
  });
  expect(res).toEqual({
    exitCode: 1,
    out: 'changes: output ts is "", not true or false\nchanges: output docs is "", not true or false\nchanges: output rust is "", not true or false\nchanges: output ports is "", not true or false\n',
  });
});

const RUST_ONLY = changes({ ts: false, opencode: false, docs: false, rust: true });

test.concurrent("a Rust-only run passes with the TypeScript jobs skipped (#407 AC-5)", async () => {
  const res = await aggregate({
    changes: RUST_ONLY,
    gate: SKIPPED,
    opencode: SKIPPED,
    docs: SKIPPED,
    rust: SUCCESS,
    "rust-musl": SUCCESS,
  });
  expect(res.exitCode).toBe(0);
  expect(res.out).toContain("rust (rust on): success");
  expect(res.out).toContain("rust-musl (rust on): success");
});

test.concurrent("a failed Rust job fails the required aggregate (#407 AC-5)", async () => {
  const res = await aggregate({
    changes: RUST_ONLY,
    gate: SKIPPED,
    opencode: SKIPPED,
    docs: SKIPPED,
    rust: { result: "failure", outputs: {} },
    "rust-musl": SUCCESS,
  });
  expect(res).toEqual({ exitCode: 1, out: "rust (rust on): failure\n" });
});

test.concurrent("a musl job skipped while rust is on fails the aggregate (#407 AC-5)", async () => {
  const res = await aggregate({
    changes: RUST_ONLY,
    gate: SKIPPED,
    opencode: SKIPPED,
    docs: SKIPPED,
    rust: SUCCESS,
  });
  expect(res).toEqual({ exitCode: 1, out: "rust-musl (rust on): skipped, but its group is on\n" });
});

test.concurrent("the Rust conformance leg skipped while ports is on fails the aggregate (#409 AC-5)", async () => {
  const ports = changes({ ts: true, opencode: true, docs: false, ports: true });
  const passed = await aggregate({
    changes: ports,
    gate: SUCCESS,
    opencode: SUCCESS,
    docs: SKIPPED,
    "rust-conformance": SUCCESS,
  });
  expect(passed.exitCode).toBe(0);
  expect(passed.out).toContain("rust-conformance (ports on): success");
  const skipped = await aggregate({
    changes: ports,
    gate: SUCCESS,
    opencode: SUCCESS,
    docs: SKIPPED,
  });
  expect(skipped).toEqual({
    exitCode: 1,
    out: "rust-conformance (ports on): skipped, but its group is on\n",
  });
});
