import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dir, "../../../../..");
const fixture = join(
  root,
  "plugins/pr-babysit/scripts/__tests__/fixtures/states/toolu-165-initial.json",
);
const dirs: string[] = [];

function setup(): { dir: string; next: string; env: NodeJS.ProcessEnv; log: string } {
  const dir = mkdtempSync(join(tmpdir(), "babysit-writes-"));
  dirs.push(dir);
  const next = join(dir, "next.json");
  copyFileSync(fixture, next);
  const stub = join(dir, "gh");
  writeFileSync(
    stub,
    `#!/usr/bin/env bash
printf '%s\\n' "$*" >> "$GH_LOG"
case "$*" in
  *'--method POST'*)
    while [ "$1" != '--input' ]; do shift; done
    jq -c . "$2" >> "$GH_PAYLOAD"
    printf '%s\\n' '{"id":7234,"html_url":"https://github.com/Falconiere/toolu/pull/165#issuecomment-7234"}' ;;
  *'resolveReviewThread'*)
    if [ "\${GH_RESOLVE_FALSE:-}" = 1 ]; then
      printf '%s\\n' '{"data":{"resolveReviewThread":{"thread":{"id":"PRRT_a","isResolved":false}}}}'
    else
      printf '%s\\n' '{"data":{"resolveReviewThread":{"thread":{"id":"PRRT_a","isResolved":true}}}}'
    fi ;;
  *) echo 'unexpected gh command' >&2; exit 1 ;;
esac
`,
    { mode: 0o755 },
  );
  const log = join(dir, "gh.log");
  const env = {
    ...process.env,
    PATH: `${dir}:${process.env.PATH}`,
    GH_LOG: log,
    GH_PAYLOAD: join(dir, "payload.json"),
    PB_GH_BACKOFF: "0 0 0",
  };
  return { dir, next, env, log };
}

afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function run(
  name: string,
  state: string,
  env: NodeJS.ProcessEnv,
  args: string[],
): { status: number | null; output: Record<string, unknown>; state: Record<string, unknown> } {
  const path = join(root, `plugins/pr-babysit/hooks/src/babysit-${name}.ts`);
  const result = spawnSync(process.execPath, [path, "--state-file", state, ...args], {
    encoding: "utf8",
    env,
  });
  if (!result.stdout.trim()) throw new Error(`${name} had no stdout: ${result.stderr}`);
  return {
    status: result.status,
    output: JSON.parse(result.stdout),
    state: JSON.parse(readFileSync(state, "utf8")),
  };
}

test("thread reply uses input file, records one id, and refuses a duplicate without another request", () => {
  const f = setup();
  const bodyPath = join(f.dir, "body.txt");
  writeFileSync(bodyPath, "Fixed the empty case.\n");
  const args = [
    "--kind",
    "thread",
    "--thread",
    "PRRT_a",
    "--root-comment",
    "41",
    "--in-reply-to",
    "42",
    "--body-file",
    bodyPath,
  ];
  const next = run("reply-thread", f.next, f.env, args);
  expect(next.status).toBe(0);
  expect(next.output).toMatchObject({ ok: true, key: "thread:PRRT_a@42", commentId: 7234 });
  expect((next.state.actions as Record<string, any>).replied["thread:PRRT_a@42"].commentId).toBe(
    7234,
  );
  expect(
    JSON.parse(readFileSync(join(f.dir, "payload.json"), "utf8").split("\n")[0] ?? "").body,
  ).toBe("Fixed the empty case.\n");
  const before = readFileSync(f.log, "utf8");
  const duplicate = run("reply-thread", f.next, f.env, args);
  expect(duplicate.status).toBe(4);
  expect(duplicate.output.errors).toBeDefined();
  expect(readFileSync(f.log, "utf8")).toBe(before);
});

test("resolve records only confirmed mutation and skips a repeated request", () => {
  const f = setup();
  const args = ["--thread", "PRRT_a"];
  const next = run("resolve-thread", f.next, f.env, args);
  expect(next.status).toBe(0);
  expect(next.output).toMatchObject({ ok: true, thread: "PRRT_a", confirmed: true, attempts: 1 });
  expect((next.state.actions as Record<string, any>).resolved.PRRT_a.confirmed).toBe(true);
  const before = readFileSync(f.log, "utf8");
  const again = run("resolve-thread", f.next, f.env, args);
  expect(again.output.alreadyResolved).toBe(true);
  expect(readFileSync(f.log, "utf8")).toBe(before);
});

test("unconfirmed resolve returns code 5 and leaves the state unchanged", () => {
  const f = setup();
  const original = readFileSync(f.next, "utf8");
  const result = run("resolve-thread", f.next, { ...f.env, GH_RESOLVE_FALSE: "1" }, [
    "--thread",
    "PRRT_a",
  ]);
  expect(result.status).toBe(5);
  expect((result.output.errors as Record<string, unknown>[])[0]?.code).toBe("resolve_unconfirmed");
  expect(readFileSync(f.next, "utf8")).toBe(original);
  expect(readFileSync(f.log, "utf8").trim().split("\n")).toHaveLength(3);
});
