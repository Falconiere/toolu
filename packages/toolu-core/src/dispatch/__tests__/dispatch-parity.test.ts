/**
 * AC-2 (#258): every `dispatcher.bats` scenario, run through the bash
 * dispatcher and `dispatchPreTool` over the same real module files, gives
 * byte-identical stdout and the same exit code.
 */
import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import {
  hookEnv,
  install,
  modulesDir,
  registryDir,
  runBashDispatch,
  runTsDispatch,
  writeModule,
} from "./dispatch-harness.ts";

const advise = (text: string): string =>
  `jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"${text}"}}'`;
const deny = (text: string): string =>
  `jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"deny",permissionDecisionReason:"${text}"}}'`;
const ask = (text: string): string =>
  `jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"ask",permissionDecisionReason:"${text}"}}'`;
const onPath = (path: string, body: string): string =>
  `p=$(jq -r ".tool_input.file_path" -); if [ "$p" = "${path}" ]; then ${body}; fi`;

const BASH = JSON.stringify({ tool_name: "Bash", tool_input: { command: "ls" } });
const patch = (...paths: string[]): string =>
  JSON.stringify({
    tool_name: "apply_patch",
    tool_input: {
      command: [
        "*** Begin Patch",
        ...paths.flatMap((p) => [`*** Update File: ${p}`, "@@", "-a", "+b"]),
        "*** End Patch",
      ].join("\n"),
    },
  });

type Scenario = {
  name: string;
  modules?: Record<string, string>;
  registry?: Record<string, string>;
  installed?: string[];
  stdin?: string;
  config?: object;
  expect: RegExp | "empty" | "exit2";
};

const SCENARIOS: Scenario[] = [
  {
    name: "advisory does not preempt a later deny",
    modules: { a_adv: advise("advisory-A"), z_deny: deny("blocked-by-Z") },
    expect: /blocked-by-Z/,
  },
  {
    name: "two advisories merge into one object",
    modules: { a: advise("context-one"), b: advise("context-two") },
    expect: /context-one\\n\\ncontext-two/,
  },
  {
    name: "duplicate advisories are emitted once",
    modules: { a: advise("same"), b: advise("same") },
    expect: /"additionalContext": "same"/,
  },
  {
    name: "deny short-circuits later modules",
    modules: { a: deny("first"), b: `touch "$HOME/ran"; ${advise("late")}` },
    expect: /first/,
  },
  {
    name: "silent modules produce no output",
    modules: { a: "exit 0", b: "true" },
    expect: "empty",
  },
  {
    name: "modules receive the hook input on stdin",
    modules: {
      a: 'c=$(jq -r .tool_input.command -); jq -n --arg c "$c" \'{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:$c}}\'',
    },
    expect: /"additionalContext": "ls"/,
  },
  {
    name: "exit 2 blocks and skips later modules",
    modules: { a: "echo blocked-by-exit >&2; exit 2", b: advise("never") },
    expect: "exit2",
  },
  {
    name: "other non-zero exits are skipped",
    modules: { a: "echo partial-garbage; exit 3", b: advise("good-context") },
    expect: /good-context/,
  },
  {
    name: "non-JSON stdout is ignored",
    modules: { a: "echo not json", b: advise("kept") },
    expect: /kept/,
  },
  { name: "empty modules dir is a no-op", modules: {}, expect: "empty" },
  {
    name: "systemMessage merges beside additionalContext",
    modules: { a: `jq -n '{systemMessage:"sys-one"}'`, b: advise("ctx") },
    expect: /"systemMessage": "sys-one"/,
  },
  {
    name: "ask reaches the caller",
    modules: { a: ask("please-confirm") },
    expect: /"permissionDecision": "ask"/,
  },
  {
    name: "a later deny outranks an earlier ask",
    modules: { a: ask("maybe"), b: deny("no-way") },
    expect: /no-way/,
  },
  {
    name: "the first ask wins",
    modules: { a: ask("ask-one"), b: ask("ask-two") },
    expect: /ask-one/,
  },
  {
    name: "advisories before and after an ask ride on its reason",
    modules: { a: advise("before"), b: ask("the-ask"), c: advise("after") },
    expect: /the-ask\\n\\nbefore\\n\\nafter/,
  },
  {
    name: "a systemMessage survives an ask",
    modules: { a: ask("the-ask"), b: `jq -n '{systemMessage:"note"}'` },
    expect: /"systemMessage": "note"/,
  },
  {
    name: "exit 2 after an ask still blocks",
    modules: { a: ask("the-ask"), b: "echo hard >&2; exit 2" },
    expect: "exit2",
  },
  {
    name: "a deny on any patch path blocks the whole patch",
    modules: {
      a: advise("seen-path"),
      z: onPath("hooks/lib/dispatch.sh", deny("protected-second-path")),
    },
    stdin: patch("src/safe.ts", "hooks/lib/dispatch.sh"),
    expect: /protected-second-path/,
  },
  {
    name: "duplicate advisories across patch paths are emitted once",
    modules: { a: advise("per-path") },
    stdin: patch("a.ts", "b.ts"),
    expect: /"additionalContext": "per-path"/,
  },
  {
    name: "malformed apply_patch fails closed",
    modules: { a: advise("never") },
    stdin: JSON.stringify({ tool_name: "apply_patch", tool_input: { command: "garbage" } }),
    expect: /Unable to parse apply_patch/,
  },
  {
    name: "an ask on one patch path prompts for the whole patch",
    modules: { a: onPath("b.ts", ask("path-ask")) },
    stdin: patch("a.ts", "b.ts"),
    expect: /path-ask/,
  },
  {
    name: "a deny on a later path outranks an ask on an earlier one",
    modules: { a: onPath("a.ts", ask("early-ask")), b: onPath("b.ts", deny("late-deny")) },
    stdin: patch("a.ts", "b.ts"),
    expect: /late-deny/,
  },
  {
    name: "an advisory from another path rides on the ask",
    modules: { a: onPath("a.ts", advise("path-a-note")), b: onPath("b.ts", ask("path-b-ask")) },
    stdin: patch("a.ts", "b.ts"),
    expect: /path-b-ask\\n\\npath-a-note/,
  },
  {
    name: "exit 2 on one patch path blocks the patch",
    modules: { a: onPath("b.ts", "echo patch-block >&2; exit 2") },
    stdin: patch("a.ts", "b.ts"),
    expect: "exit2",
  },
  {
    name: "Edit and Write are walked as one synthetic Edit",
    modules: {
      a: 'jq -c \'{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:(.tool_name + " " + .tool_input.toolu_edit_operation + " " + env.TOOLU_EDIT_OPERATION)}}\'',
    },
    stdin: JSON.stringify({ tool_name: "Write", tool_input: { file_path: "x.ts", content: "1" } }),
    expect: /Edit write write/,
  },
  {
    name: "TOOLU_LIB_DIR reaches modules",
    modules: { a: "jq -n --arg d \"$TOOLU_LIB_DIR\" '{systemMessage:$d}'" },
    expect: /hooks\/lib/,
  },
  {
    name: "an active plugin's registry module runs after built-ins",
    modules: { a: advise("builtin") },
    registry: { "fixture@toolu__probe.sh": advise("from-registry") },
    installed: ["fixture@toolu"],
    expect: /builtin\\n\\nfrom-registry/,
  },
  {
    name: "an absent plugin's registry module is skipped",
    registry: { "ghost@nowhere__probe.sh": advise("should-not-appear") },
    installed: ["fixture@toolu"],
    expect: "empty",
  },
  {
    name: "an un-namespaced registry file never runs",
    registry: { "foo.sh": advise("ungated") },
    installed: [],
    expect: "empty",
  },
  {
    name: "a registry exit 2 blocks",
    registry: { "fixture@toolu__hard.sh": "echo reg-block >&2; exit 2" },
    installed: ["fixture@toolu"],
    expect: "exit2",
  },
  {
    name: "a registry deny stops the walk",
    registry: { "fixture@toolu__a.sh": deny("reg-deny"), "fixture@toolu__b.sh": advise("after") },
    installed: ["fixture@toolu"],
    expect: /reg-deny/,
  },
  {
    name: "hooks.pre-tools false disables the dispatcher",
    modules: { a: deny("never") },
    config: { hooks: { "pre-tools": false } },
    expect: "empty",
  },
  { name: "empty stdin", modules: { a: advise("still-runs") }, stdin: "", expect: /still-runs/ },
  {
    name: "non-JSON stdin",
    modules: { a: 'jq -n --arg t "$tool_name" \'{systemMessage:("tool=" + $t)}\'' },
    stdin: "not json\n\n",
    expect: /tool=/,
  },
];

for (const s of SCENARIOS) {
  test.concurrent(s.name, async () => {
    using sb = createSandbox({ git: true });
    for (const [name, body] of Object.entries(s.modules ?? {}))
      writeModule(modulesDir(sb), `${name}.sh`, body);
    for (const [file, body] of Object.entries(s.registry ?? {}))
      writeModule(registryDir(sb), file, body);
    if (s.installed !== undefined) install(sb, ...s.installed);
    if (s.config !== undefined) sb.writeConfig("claude", "user", s.config);
    const env = hookEnv(sb);
    const stdin = s.stdin ?? BASH;
    const bash = runBashDispatch(sb, stdin, env);
    const ts = await runTsDispatch(sb, stdin, env);
    expect({ stdout: ts.stdout, exitCode: ts.exitCode }).toEqual({
      stdout: bash.stdout,
      exitCode: bash.exitCode,
    });
    if (s.expect === "empty") expect(ts.stdout).toBe("");
    else if (s.expect === "exit2") expect(ts.exitCode).toBe(2);
    else expect(ts.stdout).toMatch(s.expect);
  });
}

test.concurrent("exit 2 forwards the blocking module's stderr", async () => {
  using sb = createSandbox({ git: true });
  writeModule(modulesDir(sb), "a.sh", "echo blocked-by-exit >&2; exit 2");
  const ts = await runTsDispatch(sb, BASH, hookEnv(sb));
  expect(ts.stderr).toContain("blocked-by-exit");
});
