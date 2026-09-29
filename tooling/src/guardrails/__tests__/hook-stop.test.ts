// Ported from the upstream kit's run-fixtures.sh: the PostToolUse (--hook) and
// Stop (--stop) modes, single-repo and workspace. Exit 2, not 1: Claude Code
// ignores a 1 from a hook, and only 2 blocks on Stop.
import { expect, test } from "bun:test";
import { buildFixture } from "./fixture-tree.ts";
import { gr } from "./gr-harness.ts";

function postToolUse(path: string): string {
  return JSON.stringify({
    session_id: "test",
    hook_event_name: "PostToolUse",
    tool_name: "Write",
    tool_input: { file_path: path, content: "x" },
    tool_response: { filePath: path, success: true },
  });
}

test.concurrent("AC-5 --hook on a real PostToolUse payload: exit 2 with stderr", async () => {
  using tree = buildFixture("violating");
  const res = await gr(tree.root, ["--hook"], {
    stdin: postToolUse(`${tree.root}/src/domains/shifts/utils/helper.ts`),
  });
  expect(res.exit).toBe(2);
  expect(res.out).toContain("guardrails[folder-tree] src/domains/shifts/utils");
});

test.concurrent("--hook stays out of the way: no path, unparseable payload, missing file", async () => {
  using tree = buildFixture("violating");
  const payloads = ["{}", "not json", postToolUse(`${tree.root}/src/nowhere.ts`)];
  const results = await Promise.all(payloads.map((stdin) => gr(tree.root, ["--hook"], { stdin })));
  expect(results).toEqual(payloads.map(() => ({ exit: 0, out: "" })));
});

test.concurrent("AC-6 --stop: 0 when stop_hook_active, 0 on an unchanged tree, 2 on a changed violating tree", async () => {
  using dirty = buildFixture("violating");
  expect((await gr(dirty.root, ["--stop"], { stdin: '{"stop_hook_active":true}' })).exit).toBe(0);
  using clean = buildFixture("clean");
  expect((await gr(clean.root, ["--stop"], { stdin: "{}" })).exit).toBe(0);
  dirty.write("src/utilities/late.ts", "export const late = 1;\n");
  expect((await gr(dirty.root, ["--stop"], { stdin: "{}" })).exit).toBe(2);
});

test.concurrent("AC-4 --hook from a workspace root exits 2 and names the prefixed file", async () => {
  using ws = buildFixture("workspace-violating");
  const file = `${ws.root}/packages/database/src/schema/wide-table.ts`;
  const res = await gr(ws.root, ["--hook"], {
    stdin: JSON.stringify({ tool_input: { file_path: file } }),
  });
  expect(res.exit).toBe(2);
  expect(res.out).toContain("packages/database/src/schema/wide-table.ts");
});

test.concurrent("AC-4 --hook on a path outside the workspace exits 3", async () => {
  using ws = buildFixture("workspace");
  const res = await gr(ws.root, ["--hook"], {
    stdin: JSON.stringify({ tool_input: { file_path: "/etc/hosts" } }),
  });
  expect(res.exit).toBe(3);
  expect(res.out).toContain("is outside the workspace root");
});

test.concurrent("AC-5 --stop blocks a dirty violating workspace and passes a dirty clean one", async () => {
  using dirty = buildFixture("workspace-violating");
  dirty.write("dirty.txt", "");
  expect((await gr(dirty.root, ["--stop"], { stdin: "{}" })).exit).toBe(2);
  using clean = buildFixture("workspace");
  clean.write("dirty.txt", "");
  expect((await gr(clean.root, ["--stop"], { stdin: "{}" })).exit).toBe(0);
});
