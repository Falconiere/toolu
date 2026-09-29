/**
 * AC-1 (#259): every `post-tools/__tests__/dispatcher.bats` scenario, plus the
 * `mod.sh` environment and registry cases, run through the bash dispatcher
 * (`toolu_dispatch_hook ... PostToolUse`) and `dispatchPostTool` over the same
 * real module files, gives byte-identical stdout and the same exit code.
 */
import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import {
  hookEnv,
  install,
  modulesDir,
  registryDir,
  runBashDispatch,
  runTsPostDispatch,
  writeModule,
} from "./dispatch-harness.ts";

const advise = (text: string): string =>
  `jq -n '{hookSpecificOutput:{hookEventName:"PostToolUse",additionalContext:"${text}"}}'`;
const block = (text: string): string => `jq -n '{decision:"block",reason:"${text}"}'`;
const onPath = (path: string, body: string): string =>
  `p=$(jq -r ".tool_input.file_path" -); if [ "$p" = "${path}" ]; then ${body}; fi`;

const BASH = JSON.stringify({
  tool_name: "Bash",
  tool_input: { command: "ls" },
  tool_response: { exit_code: 0 },
});
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
  /** Run from a directory that is not a git repository. */
  outsideRepo?: boolean;
  expect: RegExp | "empty" | "exit2";
};

const SCENARIOS: Scenario[] = [
  {
    name: "decision:block short-circuits later modules",
    modules: { a: block("stop-here"), b: `touch "$HOME/ran"; ${advise("late")}` },
    expect: /stop-here/,
  },
  {
    name: "advisory does not preempt a later block",
    modules: { a: advise("advisory-A"), z: block("blocked-by-Z") },
    expect: /blocked-by-Z/,
  },
  {
    name: "permissionDecision deny is ignored",
    modules: {
      a: `jq -n '{hookSpecificOutput:{hookEventName:"PostToolUse",permissionDecision:"deny",permissionDecisionReason:"pre-only"}}'`,
      b: advise("still-here"),
    },
    expect: /still-here/,
  },
  {
    name: "an ask-shaped result is not held",
    modules: {
      a: `jq -n '{hookSpecificOutput:{hookEventName:"PostToolUse",permissionDecision:"ask",permissionDecisionReason:"r",additionalContext:"ask-ctx"}}'`,
      b: advise("after"),
    },
    expect: /ask-ctx\\n\\nafter/,
  },
  {
    name: "two advisories merge into one object",
    modules: { a: advise("context-one"), b: advise("context-two") },
    expect: /context-one\\n\\ncontext-two/,
  },
  {
    name: "systemMessage merges beside additionalContext",
    modules: { a: `jq -n '{systemMessage:"sys-one"}'`, b: advise("ctx") },
    expect: /"systemMessage": "sys-one"/,
  },
  {
    name: "other non-zero exits are skipped",
    modules: { a: "echo partial-garbage; exit 3", b: advise("good-context") },
    expect: /good-context/,
  },
  {
    name: "exit 2 blocks and skips later modules",
    modules: { a: "echo blocked-by-exit >&2; exit 2", b: advise("never") },
    expect: "exit2",
  },
  { name: "empty modules dir is a no-op", modules: {}, expect: "empty" },
  {
    name: "duplicate per-file advisories are emitted once",
    modules: { a: advise("per-path") },
    stdin: patch("a.ts", "b.ts"),
    expect: /"additionalContext": "per-path"/,
  },
  {
    name: "a block on any patch path blocks the whole patch",
    modules: { a: advise("seen"), z: onPath("b.ts", block("second-path-block")) },
    stdin: patch("a.ts", "b.ts"),
    expect: /second-path-block/,
  },
  {
    name: "exit 2 on one patch path blocks the patch",
    modules: { a: onPath("b.ts", "echo patch-block >&2; exit 2") },
    stdin: patch("a.ts", "b.ts"),
    expect: "exit2",
  },
  {
    name: "malformed apply_patch blocks result processing",
    modules: { a: advise("never") },
    stdin: JSON.stringify({ tool_name: "apply_patch", tool_input: { command: "garbage" } }),
    expect: /per-file post-edit quality checks could not run/,
  },
  {
    name: "Write is walked as one synthetic Edit",
    modules: {
      a: 'jq -c \'{hookSpecificOutput:{hookEventName:"PostToolUse",additionalContext:(.tool_name + " " + env.TOOLU_EDIT_OPERATION)}}\'',
    },
    stdin: JSON.stringify({ tool_name: "Write", tool_input: { file_path: "x.ts", content: "1" } }),
    expect: /Edit write/,
  },
  {
    name: "PROJECT_ROOT, the node_modules PATH entry and TOOLU_LIB_DIR reach modules",
    modules: {
      a: 'jq -n --arg r "$PROJECT_ROOT" --arg p "${PATH%%:*}" --arg l "$TOOLU_LIB_DIR" --arg t "$tool_name" \'{systemMessage:($r + "|" + $p + "|" + $l + "|" + $t)}\'',
    },
    expect: /\/node_modules\/\.bin\|.*hooks\/lib\|Bash/,
  },
  {
    name: "PROJECT_ROOT is the working directory outside a repository",
    modules: { a: 'jq -n --arg r "$PROJECT_ROOT" \'{systemMessage:("root=" + $r)}\'' },
    outsideRepo: true,
    expect: /root=\//,
  },
  {
    name: "an active plugin's registry module runs after built-ins",
    modules: { a: advise("builtin") },
    registry: { "fixture@toolu__probe.sh": advise("from-registry") },
    installed: ["fixture@toolu"],
    expect: /builtin\\n\\nfrom-registry/,
  },
  {
    name: "a registry block stops the walk",
    registry: {
      "fixture@toolu__a.sh": block("reg-block"),
      "fixture@toolu__b.sh": advise("after"),
    },
    installed: ["fixture@toolu"],
    expect: /reg-block/,
  },
  {
    name: "a registry exit 2 blocks",
    registry: { "fixture@toolu__hard.sh": "echo reg-block >&2; exit 2" },
    installed: ["fixture@toolu"],
    expect: "exit2",
  },
  {
    name: "an absent plugin's registry module is skipped",
    registry: { "ghost@nowhere__probe.sh": advise("should-not-appear") },
    installed: ["fixture@toolu"],
    expect: "empty",
  },
  {
    name: "hooks.post-tools false disables the dispatcher",
    modules: { a: block("never") },
    config: { hooks: { "post-tools": false } },
    expect: "empty",
  },
  {
    name: "hooks.pre-tools false leaves the post dispatcher on",
    modules: { a: advise("post-on") },
    config: { hooks: { "pre-tools": false } },
    expect: /post-on/,
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
      writeModule(registryDir(sb, "post"), file, body);
    if (s.installed !== undefined) install(sb, ...s.installed);
    if (s.config !== undefined) sb.writeConfig("claude", "user", s.config);
    const env = hookEnv(sb);
    const cwd = s.outsideRepo === true ? sb.home : sb.project;
    const stdin = s.stdin ?? BASH;
    const bash = runBashDispatch(sb, stdin, env, "post", cwd);
    const ts = await runTsPostDispatch(sb, stdin, env, cwd);
    expect({ stdout: ts.stdout, exitCode: ts.exitCode }).toEqual({
      stdout: bash.stdout,
      exitCode: bash.exitCode,
    });
    if (s.expect === "empty") expect(ts.stdout).toBe("");
    else if (s.expect === "exit2") expect(ts.exitCode).toBe(2);
    else expect(ts.stdout).toMatch(s.expect);
  });
}

test.concurrent("a block leaves later modules unrun", async () => {
  using sb = createSandbox({ git: true });
  writeModule(modulesDir(sb), "a.sh", block("stop"));
  writeModule(modulesDir(sb), "b.sh", 'touch "$HOME/ran"');
  await runTsPostDispatch(sb, BASH, hookEnv(sb));
  expect(await Bun.file(`${sb.home}/ran`).exists()).toBe(false);
});

test.concurrent("exit 2 forwards the blocking module's stderr", async () => {
  using sb = createSandbox({ git: true });
  writeModule(modulesDir(sb), "a.sh", "echo post-blocked >&2; exit 2");
  const ts = await runTsPostDispatch(sb, BASH, hookEnv(sb));
  expect(ts).toMatchObject({ stdout: "", exitCode: 2 });
  expect(ts.stderr).toContain("post-blocked");
});
