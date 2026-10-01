/**
 * AC-3 (#259): the named #283 fixtures through the committed bundle behind its
 * launcher against real project state.
 * Commands come from `tooling/fixtures/shell/issue-283.json`.
 */
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { isJsonObject } from "@toolu/core/config";
import { toStdin } from "@toolu/conformance/harness/fixtures";
import { runPostBundle, type PosttoolResult } from "@toolu/conformance/harness/posttool";
import { POSTTOOL_CORPUS, preparePost, ran } from "@toolu/conformance/harness/posttool-corpus";
import { pretoolEnv, type PretoolRun } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";

const FIXTURES = resolve(import.meta.dir, "../../../../../tooling/fixtures/shell/issue-283.json");

function fixtureCommand(id: string): string {
  const doc: unknown = JSON.parse(readFileSync(FIXTURES, "utf8"));
  const cases = isJsonObject(doc) && Array.isArray(doc.cases) ? doc.cases : [];
  const found: unknown = cases.find((c: unknown) => isJsonObject(c) && c.id === id);
  if (!isJsonObject(found) || typeof found.command !== "string")
    throw new Error(`no fixture ${id}`);
  return found.command;
}

const GATE = ".claude/tmp/quality-gate-status.json";
const WAIVER = ".claude/tmp/push-review/feat_example.waiver.json";

function gateStatus(r: PosttoolResult): unknown {
  const doc: unknown = JSON.parse(r.state[GATE] ?? "null");
  return isJsonObject(doc) ? doc.status : undefined;
}

/** Run `commands` (command, exit status) in order through the bundle. */
async function bundleAfter(
  sb: Sandbox,
  commands: readonly (readonly [string, number])[],
): Promise<PosttoolResult> {
  const calls: PretoolRun[] = commands.map(([command, code]) => ({
    cwd: sb.project,
    env: pretoolEnv(sb, "claude"),
    stdin: JSON.stringify(toStdin("claude", ran(command, code)(sb), { cwd: sb.project })),
  }));
  let last: PosttoolResult | undefined;
  for (const call of calls) last = await runPostBundle(sb, call);
  if (last === undefined) throw new Error("no calls");
  return last;
}

const STAYS_FAILING: Record<string, string> = {
  "283-6a": fixtureCommand("283-6a"),
  "283-7a": fixtureCommand("283-7a"),
};

for (const [id, command] of Object.entries(STAYS_FAILING)) {
  test.concurrent(`${id}: ${command} exiting 0 leaves a failing gate failing`, async () => {
    using sb = createSandbox({ git: true });
    const bundle = await bundleAfter(sb, [
      ["bun test", 1],
      [command, 0],
    ]);
    expect(gateStatus(bundle)).toBe("failing");
  });
}

const PUSHES: Record<string, string> = {
  "283-8a": fixtureCommand("283-8a"),
  "283-8h": fixtureCommand("283-8h"),
};

const PROMOTE = POSTTOOL_CORPUS.find((c) => c.name.startsWith("push-waiver: a successful push"));

for (const [id, command] of Object.entries(PUSHES)) {
  test.concurrent(`${id}: ${command} promotes the pending waiver`, async () => {
    if (PROMOTE === undefined) throw new Error("no push-waiver corpus case");
    using sb = createSandbox({ git: true });
    preparePost(sb, "claude", PROMOTE);
    const bundle = await bundleAfter(sb, [[command, 0]]);
    expect(bundle.state[WAIVER]).toBeDefined();
  });
}
