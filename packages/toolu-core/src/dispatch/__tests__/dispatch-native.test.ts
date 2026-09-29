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
import type { PreToolModule } from "../dispatch.ts";
import {
  hookEnv,
  install,
  modulesDir,
  registryDir,
  runTsDispatch,
  tableOf,
  writeModule,
} from "./dispatch-harness.ts";

const BASH = JSON.stringify({ tool_name: "Bash", tool_input: { command: "git status" } });

function native(name: string, run: () => Promise<Decision>): PreToolModule {
  return { kind: "native", name, run };
}

test.concurrent("a native deny ends the walk before later bash modules run", async () => {
  using sb = createSandbox({ git: true });
  writeModule(modulesDir(sb), "later.sh", `touch "${sb.path("ran")}"`);
  const builtins = [
    native("first", () => Promise.resolve({ kind: "deny", reason: "native says no" })),
    ...tableOf(modulesDir(sb)),
  ];
  const out = await runTsDispatch(sb, BASH, hookEnv(sb), builtins);
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
  const probe: PreToolModule = {
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
  const out = await runTsDispatch(sb, BASH, hookEnv(sb), [probe]);
  expect(out).toEqual({ stdout: "", stderr: "", exitCode: 0 });
  expect(seen).toEqual(["shell/pre", "git status", "Bash"]);
});

test.concurrent("a throwing or invalid native module is skipped and the walk goes on", async () => {
  using sb = createSandbox({ git: true });
  writeModule(
    modulesDir(sb),
    "after.sh",
    `jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"bash after"}}'`,
  );
  const builtins = [
    native("boom", () => Promise.reject(new Error("exploded"))),
    native("bad", () => Promise.resolve({ kind: "advisory", message: "" })),
    ...tableOf(modulesDir(sb)),
  ];
  const out = await runTsDispatch(sb, BASH, hookEnv(sb), builtins);
  expect(JSON.parse(out.stdout)).toEqual({
    hookSpecificOutput: { hookEventName: "PreToolUse", additionalContext: "bash after" },
  });
  expect(out.stderr).toContain("toolu-dispatch: module boom exited 1; output skipped");
  expect(out.stderr).toContain("toolu-dispatch: module bad exited 1; output skipped");
});

test.concurrent("a native advisory merges with bash advisories in table order", async () => {
  using sb = createSandbox({ git: true });
  writeModule(
    modulesDir(sb),
    "b.sh",
    `jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"from bash"}}'`,
  );
  const builtins = [
    native("a", () => Promise.resolve({ kind: "advisory", message: "from native" })),
    ...tableOf(modulesDir(sb)),
  ];
  const out = await runTsDispatch(sb, BASH, hookEnv(sb), builtins);
  expect(JSON.parse(out.stdout).hookSpecificOutput.additionalContext).toBe(
    "from native\n\nfrom bash",
  );
});

test.concurrent("on Codex a native ask cannot prompt, so it is a deny", async () => {
  using sb = createSandbox({ git: true });
  const builtins = [native("a", () => Promise.resolve({ kind: "ask", reason: "confirm?" }))];
  const env = hookEnv(sb, { PLUGIN_ROOT: sb.root, CODEX_HOME: sb.codexHome });
  const out = await runTsDispatch(sb, BASH, env, builtins);
  expect(JSON.parse(out.stdout).hookSpecificOutput.permissionDecision).toBe("deny");
});

test.concurrent("an ESM registry module's advisory merges after the built-ins", async () => {
  using sb = createSandbox({ git: true });
  writeModule(
    modulesDir(sb),
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
  const out = await runTsDispatch(sb, BASH, hookEnv(sb));
  expect(JSON.parse(out.stdout).hookSpecificOutput.additionalContext).toBe(
    "builtin\n\nesm advises",
  );
});

test.concurrent("an ESM registry deny wins over a built-in ask", async () => {
  using sb = createSandbox({ git: true });
  writeModule(
    modulesDir(sb),
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
  const out = await runTsDispatch(sb, BASH, hookEnv(sb));
  expect(JSON.parse(out.stdout).hookSpecificOutput).toMatchObject({
    permissionDecision: "deny",
    permissionDecisionReason: "esm denies",
  });
});
