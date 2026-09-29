/**
 * #260: the native code-edit-rules over real rule files: first match wins,
 * extra docs on a path match, repo-relative paths, and the lenient reading the
 * bash `jq` calls had (extra keys, missing or odd fields, malformed files).
 * Absolute paths inside a repo are covered by the bundle replay, whose cwd is
 * the sandbox repo (`pre-tool-modules-a.test.ts`).
 */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { codeEditRulesModule } from "../code-edit-rules.ts";
import { gateCtx, gateEnv, SHIPPED_SETTINGS, shellEvent, toolEvent } from "./gate-harness.ts";

const gate = codeEditRulesModule();

let made = 0;

function rules(sb: Sandbox, body: string | undefined): string {
  made += 1;
  const dir = join(sb.root, `settings-${String(made)}`);
  mkdirSync(dir, { recursive: true });
  if (body !== undefined) writeFileSync(join(dir, "code-edit-rules.json"), body);
  return dir;
}

async function context(sb: Sandbox, dir: string, path: string, tool = "Edit"): Promise<string> {
  const decision = await gate.run(
    toolEvent(sb, tool, { file_path: path }),
    gateCtx(sb, "claude", gateEnv(sb, dir)),
  );
  return decision.kind === "advisory" ? decision.message : "";
}

test.concurrent("the first matching rule's docs, joined, with the repo-relative path", async () => {
  using sb = createSandbox({ git: true });
  const dir = rules(sb, '{"rules":[{"match":"*.rs","docs":["a","b"]},{"match":"*","docs":["c"]}]}');
  expect(await context(sb, dir, "/abs/src/foo.rs")).toBe(
    "File: /abs/src/foo.rs\nApply these rules: a + b",
  );
  expect(await context(sb, dir, "x.md", "MultiEdit")).toBe("File: x.md\nApply these rules: c");
  expect(await context(sb, dir, "x.md", "Read")).toBe("");
});

test.concurrent("shipped rules add feature docs only on a feature path", async () => {
  using sb = createSandbox({ git: true });
  expect(await context(sb, SHIPPED_SETTINGS, "src/features/x.ts")).toContain(
    "frontend: colocate by feature",
  );
  expect(await context(sb, SHIPPED_SETTINGS, "lib/x.ts")).not.toContain("frontend");
});

test.concurrent("lenient reading, as jq read it", async () => {
  using sb = createSandbox({ git: true });
  const cases: [string | undefined, string][] = [
    [undefined, ""],
    ['{"rules":[]}', ""],
    ["{not json", ""],
    ['{"rules":{"match":"*"}}', ""],
    ['{"rules":[{"match":"*.rs","docs":[]},{"match":"*.rs","docs":["second"]}]}', ""],
    ['{"rules":[{"match":"*.py","note":1},{"match":"*.rs","docs":["rs"],"x":true}]}', "rs"],
    ['{"rules":[{"match":"*.rs","docs":["a",5,true,null]}]}', "a + 5 + true + "],
    ['{"rules":[{"match":"*.rs","docs":["a",{}]}]}', ""],
    ['{"rules":[{"match":"*.rs","docs":"x"}]}', ""],
    ['{"rules":["x",null,{"match":false},{"match":"*.rs","docs":["ok"]}]}', "ok"],
    ['{"rules":[{"match":"*.rs","docs":["null"]}]}', ""],
    [
      '{"rules":[{"match":"*.rs","docs":[],"when_path_matches":["a*"],"extra_docs":["extra"]}]}',
      "extra",
    ],
  ];
  for (const [body, docs] of cases) {
    const got = await context(sb, rules(sb, body), "a.rs");
    expect(got).toBe(docs === "" ? "" : `File: a.rs\nApply these rules: ${docs}`);
  }
});

test.concurrent("a shell command is not an edit", async () => {
  using sb = createSandbox({ git: true });
  const dir = rules(sb, '{"rules":[{"match":"*","docs":["x"]}]}');
  const decision = await gate.run(
    shellEvent(sb, "touch a.rs"),
    gateCtx(sb, "claude", gateEnv(sb, dir)),
  );
  expect(decision).toEqual({ kind: "allow" });
});
