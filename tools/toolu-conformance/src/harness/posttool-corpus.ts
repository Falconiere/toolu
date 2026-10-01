/**
 * The PostToolUse fixture corpus (#259): the dispatcher's decision paths, the
 * gate-status and push-waiver modules, and registry `.sh` modules. The
 * language-quality plugins are TypeScript modules (#265 to #267), which bash
 * `mod.sh` cannot run; their parity is their own golden suites, and a fixture
 * module stands in for them on the per-path walk. Triggers come
 * from the `post-tools` bats suites. `expect` is the decision class on each
 * host; post-tool decisions are the same on Claude Code and Codex.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { diffSha } from "@toolu/core/state";
import { pushWaiverPend } from "@toolu/core/ledger";
import {
  bashFixture,
  patchFixture,
  postToolFixture,
  toStdin,
  writeFixture,
  type Fixture,
} from "./fixtures.ts";
import { featureBranch, stateDir } from "./pretool-case.ts";
import { hostConfigRoot, installPlugins, type PretoolHost } from "./pretool.ts";
import type { Sandbox } from "./sandbox.ts";

export type PostOutcome = "block" | "advisory" | "silent" | "exit2";

export type PosttoolCase = {
  name: string;
  fixture?: (sb: Sandbox) => Fixture;
  /** Raw stdin instead of the rendered fixture (malformed input). */
  stdin?: string;
  /** Project `toolu.config.json`. */
  config?: object;
  setup?: (sb: Sandbox, host: PretoolHost) => void;
  expect: PostOutcome;
};

function put(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}

/** A Bash call that already ran with exit status `code`. */
export function ran(command: string, code: number): (sb: Sandbox) => Fixture {
  return () =>
    postToolFixture(bashFixture(command), {
      metadata: { exit_code: code },
      stdout: "",
      stderr: "",
    });
}

/** A Write that landed: the file is on disk before the hook runs. */
function wrote(rel: string, body: string): (sb: Sandbox) => Fixture {
  return (sb) => {
    put(sb.path(rel), body);
    return postToolFixture(writeFixture(sb.path(rel), body), { success: true });
  };
}

/** A failing file-hook entry beside nothing else, the way ts-quality records one. */
function seedFileFailure(sb: Sandbox, host: PretoolHost): void {
  const entry = {
    source: "ts-quality-hook",
    reason: "bad ts",
    violations: "viol\n",
    updatedAt: "2026-01-01T00:00:00Z",
  };
  const doc = { status: "failing", ...entry, file: "/p/a.ts", entries: { "/p/a.ts": entry } };
  put(
    sb.path(`${stateDir(host)}/tmp/quality-gate-status.json`),
    `${JSON.stringify(doc, null, 2)}\n`,
  );
}

/** `feat/example` ahead of `main` with a pending waiver for exactly its diff. */
function pendingWaiver(sb: Sandbox, host: PretoolHost): void {
  featureBranch(sb);
  const env = { HOME: sb.home, PATH: process.env.PATH ?? "/usr/bin:/bin" };
  const sha = diffSha(sb.project, "main", { env }) ?? "";
  pushWaiverPend(sb.project, "feat_example", sha, "main", "no-state", { env, host });
}
function registryModule(sb: Sandbox, host: PretoolHost, file: string, body: string): void {
  const path = join(hostConfigRoot(sb, host), "toolu", "post-tools.d", file);
  put(path, `#!/usr/bin/env bash\n${body}\n`);
}

/** Fails each edited path in the gate through toolu's hooks lib, as the bash quality modules did. */
const GATE_MODULE = [
  '. "$TOOLU_LIB_DIR/detect.sh"',
  '. "$TOOLU_LIB_DIR/gate-file.sh"',
  "file=$(jq -r '.tool_input.file_path // empty' <<<\"$input\")",
  '[ -n "$file" ] || exit 0',
  'gate="$(toolu_project_state_root "$PROJECT_ROOT")/quality-gate-status.json"',
  'mkdir -p "${gate%/*}"',
  'gate_record_failure "$gate" "$file" fixture-hook "fixture violation" "fixture violation in $file"',
  "jq -n --arg e PostToolUse --arg ctx \"fixture violation in $file\" '{hookSpecificOutput:{hookEventName:$e,additionalContext:$ctx}}'",
].join("\n");

export const POSTTOOL_CORPUS: readonly PosttoolCase[] = [
  {
    name: "gate-status: failing quality command advises",
    fixture: ran("bun test", 1),
    expect: "advisory",
  },
  {
    name: "gate-status: first passing quality command",
    fixture: ran("cargo clippy", 0),
    expect: "silent",
  },
  {
    name: "gate-status: a file-hook failure survives a passing command",
    fixture: ran("cargo test", 0),
    setup: seedFileFailure,
    expect: "silent",
  },
  {
    name: "gate-status: NO-TRIGGER cat tsconfig.json",
    fixture: ran("cat tsconfig.json", 0),
    expect: "silent",
  },
  { name: "plain command is silent", fixture: ran("ls", 0), expect: "silent" },
  {
    name: "push-waiver: a successful push promotes the waiver",
    fixture: ran("git push", 0),
    setup: pendingWaiver,
    expect: "silent",
  },
  {
    name: "push-waiver: a failed push keeps the pending marker",
    fixture: ran("git push", 1),
    setup: pendingWaiver,
    expect: "silent",
  },
  {
    name: "hooks.post-tools false disables the dispatcher",
    fixture: ran("bun test", 1),
    config: { version: 1, hooks: { "post-tools": false } },
    expect: "silent",
  },
  {
    name: "malformed apply_patch blocks",
    stdin: JSON.stringify({
      tool_name: "apply_patch",
      tool_input: { command: "garbage" },
      tool_response: "Done",
    }),
    expect: "block",
  },
  { name: "empty stdin is silent", stdin: "", expect: "silent" },
  { name: "non-JSON stdin is silent", stdin: "not json\n", expect: "silent" },
  {
    name: "registry: a written file reaches a module with its path",
    setup: (sb, host) => registryModule(sb, host, "fixture@toolu__gate.sh", GATE_MODULE),
    fixture: wrote("src/bad.txt", "bad\n"),
    expect: "advisory",
  },
  {
    name: "registry: a multi-path patch records a gate entry per path",
    setup: (sb, host) => {
      registryModule(sb, host, "fixture@toolu__gate.sh", GATE_MODULE);
      put(sb.path("src/bad.txt"), "bad\n");
      put(sb.path("src/worse.txt"), "worse\n");
    },
    fixture: () =>
      postToolFixture(
        patchFixture([
          { op: "update", path: "src/bad.txt", lines: ["-a", "+b"] },
          { op: "update", path: "src/worse.txt", lines: ["-a", "+b"] },
        ]),
        "Done",
      ),
    expect: "advisory",
  },
  {
    name: "registry: a block stops the walk",
    fixture: ran("ls", 0),
    setup: (sb, host) => {
      registryModule(
        sb,
        host,
        "fixture@toolu__a.sh",
        `jq -n '{decision:"block",reason:"fixture-block"}'`,
      );
      registryModule(sb, host, "fixture@toolu__b.sh", `touch "${sb.path("ran-after-block")}"`);
    },
    expect: "block",
  },
  {
    name: "registry: exit 2 blocks with stderr",
    fixture: ran("ls", 0),
    setup: (sb, host) =>
      registryModule(sb, host, "fixture@toolu__hard.sh", "echo fixture-exit-2 >&2; exit 2"),
    expect: "exit2",
  },
  {
    name: "registry: advisories merge after the built-ins",
    fixture: ran("bun test", 1),
    setup: (sb, host) =>
      registryModule(
        sb,
        host,
        "fixture@toolu__note.sh",
        `jq -n '{hookSpecificOutput:{hookEventName:"PostToolUse",additionalContext:"fixture-note"}}'`,
      ),
    expect: "advisory",
  },
];

const SPECS = ["toolu@toolu", "fixture@toolu", "ts-quality@toolu"];

/** Put the sandbox in `c`'s state for `host`: plugins installed, config, setup. */
export function preparePost(sb: Sandbox, host: PretoolHost, c: PosttoolCase): void {
  // No detached gc/maintenance: it would write into `.git` between the two runs.
  sb.git("config", "maintenance.auto", "false");
  sb.git("config", "gc.auto", "0");
  installPlugins(sb, ...SPECS);
  if (c.config !== undefined) sb.writeConfig(host, "project", c.config);
  c.setup?.(sb, host);
}

/** The hook stdin for `c` as `host` delivers it. */
export function postStdin(sb: Sandbox, host: PretoolHost, c: PosttoolCase): string {
  if (c.stdin !== undefined) return c.stdin;
  if (c.fixture === undefined) throw new Error(`${c.name}: neither a fixture nor stdin`);
  return JSON.stringify(toStdin(host, c.fixture(sb), { cwd: sb.project }));
}
