/**
 * AC-5 (#258): with its fallback off, a module runs in process. A native deny
 * short-circuits the bash modules after it, a native failure is skipped like a
 * non-zero bash exit, and an ESM registry module's decision merges with the
 * bash modules' output.
 */
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import type { Decision } from "../../decision/decision.ts";
import { buildModule } from "../../registry/__tests__/module-bundles.ts";
import { dispatchPostTool, type ToolModule } from "../dispatch.ts";
import {
  hookEnv,
  install,
  LIB,
  registryDir,
  runTsDispatch,
  writeBuiltin,
} from "./dispatch-harness.ts";

const BASH = JSON.stringify({ tool_name: "Bash", tool_input: { command: "git status" } });

function native(name: string, run: () => Promise<Decision>): ToolModule {
  return { kind: "native", name, run };
}

test.concurrent("a native deny ends the walk before later bash modules run", async () => {
  using sb = createSandbox({ git: true });
  writeBuiltin(sb, "later.sh", `touch "${sb.path("ran")}"`);
  const builtins = [
    native("first", () => Promise.resolve({ kind: "deny", reason: "native says no" })),
  ];
  const out = await runTsDispatch(BASH, hookEnv(sb), builtins);
  expect(JSON.parse(out.stdout)).toEqual({
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: "native says no",
    },
  });
  expect(existsSync(sb.path("ran"))).toBe(false);
});

test.concurrent("a native module sees the shell event and the raw payload", async () => {
  using sb = createSandbox({ git: true });
  const seen: string[] = [];
  const probe: ToolModule = {
    kind: "native",
    name: "probe",
    run: (event, ctx) => {
      seen.push(
        event.type,
        event.type === "shell/pre" ? event.command : "",
        String(ctx.raw.tool_name),
      );
      return Promise.resolve({ kind: "allow" });
    },
  };
  const out = await runTsDispatch(BASH, hookEnv(sb), [probe]);
  expect(out).toEqual({ stdout: "", stderr: "", exitCode: 0 });
  expect(seen).toEqual(["shell/pre", "git status", "Bash"]);
});

test.concurrent("a throwing or invalid native module is skipped and the walk goes on", async () => {
  using sb = createSandbox({ git: true });
  writeBuiltin(
    sb,
    "after.sh",
    `jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"bash after"}}'`,
  );
  const builtins = [
    native("boom", () => Promise.reject(new Error("exploded"))),
    native("bad", () => Promise.resolve({ kind: "advisory", message: "" })),
  ];
  const out = await runTsDispatch(BASH, hookEnv(sb), builtins);
  expect(JSON.parse(out.stdout)).toEqual({
    hookSpecificOutput: { hookEventName: "PreToolUse", additionalContext: "bash after" },
  });
  expect(out.stderr).toContain("toolu-dispatch: module boom exited 1; output skipped");
  expect(out.stderr).toContain("toolu-dispatch: module bad exited 1; output skipped");
});

test.concurrent("a native advisory merges with bash advisories in table order", async () => {
  using sb = createSandbox({ git: true });
  writeBuiltin(
    sb,
    "b.sh",
    `jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"from bash"}}'`,
  );
  const builtins = [
    native("a", () => Promise.resolve({ kind: "advisory", message: "from native" })),
  ];
  const out = await runTsDispatch(BASH, hookEnv(sb), builtins);
  expect(JSON.parse(out.stdout).hookSpecificOutput.additionalContext).toBe(
    "from native\n\nfrom bash",
  );
});

test.concurrent("on Codex a native ask cannot prompt, so it is a deny", async () => {
  using sb = createSandbox({ git: true });
  const builtins = [native("a", () => Promise.resolve({ kind: "ask", reason: "confirm?" }))];
  const env = hookEnv(sb, { PLUGIN_ROOT: sb.root, CODEX_HOME: sb.codexHome });
  const out = await runTsDispatch(BASH, env, builtins);
  expect(JSON.parse(out.stdout).hookSpecificOutput.permissionDecision).toBe("deny");
});

test.concurrent("an ESM registry module's advisory merges after the built-ins", async () => {
  using sb = createSandbox({ git: true });
  writeBuiltin(
    sb,
    "a.sh",
    `jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"builtin"}}'`,
  );
  await buildModule(join(registryDir(sb), "fixture@toolu__esm.js"), {
    spec: "fixture@toolu",
    name: "esm",
    event: "tool/pre",
    behavior: "advisory",
  });
  install(sb, "fixture@toolu");
  const out = await runTsDispatch(BASH, hookEnv(sb));
  expect(JSON.parse(out.stdout).hookSpecificOutput.additionalContext).toBe(
    "builtin\n\nesm advises",
  );
});

test.concurrent("an ESM registry deny wins over a built-in ask", async () => {
  using sb = createSandbox({ git: true });
  writeBuiltin(
    sb,
    "a.sh",
    `jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"ask",permissionDecisionReason:"maybe"}}'`,
  );
  await buildModule(join(registryDir(sb), "fixture@toolu__esm.js"), {
    spec: "fixture@toolu",
    name: "esm",
    event: "tool/pre",
    behavior: "deny",
  });
  install(sb, "fixture@toolu");
  const out = await runTsDispatch(BASH, hookEnv(sb));
  expect(JSON.parse(out.stdout).hookSpecificOutput).toMatchObject({
    permissionDecision: "deny",
    permissionDecisionReason: "esm denies",
  });
});

test.concurrent("opt-in post patch walk records every block while the default stops at the first", async () => {
  using sb = createSandbox({ git: true });
  const files = [sb.path("one.ts"), sb.path("two.ts")] as const;
  const patch = [
    "*** Begin Patch",
    `*** Add File: ${files[0]}`,
    "+first",
    `*** Add File: ${files[1]}`,
    "+second",
    "*** End Patch",
  ].join("\n");
  const input = JSON.stringify({ tool_name: "apply_patch", tool_input: { command: patch } });
  const visited: string[] = [];
  const blocker: ToolModule = {
    kind: "native",
    name: "blocker",
    run(event) {
      const path = String(event.toolInput.file_path);
      visited.push(path);
      return Promise.resolve({ kind: "post_block", reason: `invalid ${path}` });
    },
  };
  const options = { builtins: [blocker], libDir: LIB, env: hookEnv(sb), cwd: sb.project };
  const previous = await dispatchPostTool(input, options);
  expect(visited).toEqual([files[0]]);
  expect(JSON.parse(previous.stdout)).toMatchObject({
    decision: "block",
    reason: `invalid ${files[0]}`,
  });

  visited.length = 0;
  const continued = await dispatchPostTool(input, { ...options, continuePostBlocks: true });
  expect(visited).toEqual([...files]);
  expect(JSON.parse(continued.stdout)).toEqual({
    decision: "block",
    reason: files.map((file) => `invalid ${file}`).join("\n\n"),
  });
});
