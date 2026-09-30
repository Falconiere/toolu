/**
 * The PostToolUse fixture corpus (#259): the dispatcher's decision paths, the
 * gate-status and push-waiver modules, and the bash language-quality registry
 * modules (python- and rust-quality until #266/#267) registered by their real
 * `register.sh`. ts-quality is a TypeScript module since #265, which bash
 * `mod.sh` cannot run; its parity is its own golden suite. Triggers come
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
import { hostConfigRoot, installPlugins, registerPlugin, type PretoolHost } from "./pretool.ts";
import type { Sandbox } from "./sandbox.ts";

export type PostOutcome = "block" | "advisory" | "silent" | "exit2";

export type PosttoolCase = {
  name: string;
  fixture?: (sb: Sandbox) => Fixture;
  /** Raw stdin instead of the rendered fixture (malformed input). */
  stdin?: string;
  /** Project `toolu.config.json`. */
  config?: object;
  /** Plugins whose real register hook syncs their modules into the registry. */
  register?: readonly string[];
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

/** Project marker files, committed: detection reads the repository, as in the bats setup. */
function project(files: Record<string, string>): (sb: Sandbox) => void {
  return (sb) => {
    for (const [rel, body] of Object.entries(files)) put(sb.path(rel), body);
    sb.git("add", ...Object.keys(files));
    sb.git("commit", "-q", "-m", "project markers");
  };
}

const rustProject = project({ "Cargo.toml": '[package]\nname = "fixture"\nversion = "0.1.0"\n' });
const pyProject = project({ "pyproject.toml": '[project]\nname = "fixture"\n' });

function registryModule(sb: Sandbox, host: PretoolHost, file: string, body: string): void {
  const path = join(hostConfigRoot(sb, host), "toolu", "post-tools.d", file);
  put(path, `#!/usr/bin/env bash\n${body}\n`);
}

const BAD_PY = "from unittest.mock import patch\n\n\ndef test_x():\n    assert patch\n";
const BAD_RS = "#[allow(dead_code)]\nfn bad() {}\n";

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
    name: "python-quality registry: mock import in a test file",
    register: ["python-quality"],
    setup: pyProject,
    fixture: wrote("tests/test_bad.py", BAD_PY),
    expect: "advisory",
  },
  {
    name: "rust-quality registry: lint suppression in a written file",
    register: ["rust-quality"],
    setup: rustProject,
    fixture: wrote("src/bad.rs", BAD_RS),
    expect: "advisory",
  },
  {
    name: "multi-path patch through python-quality and rust-quality",
    register: ["python-quality", "rust-quality"],
    setup: (sb) => {
      pyProject(sb);
      rustProject(sb);
      put(sb.path("tests/test_bad.py"), BAD_PY);
      put(sb.path("src/bad.rs"), BAD_RS);
    },
    fixture: () =>
      postToolFixture(
        patchFixture([
          { op: "update", path: "tests/test_bad.py", lines: ["-a", "+b"] },
          { op: "update", path: "src/bad.rs", lines: ["-a", "+b"] },
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

const SPECS = [
  "toolu@toolu",
  "fixture@toolu",
  "ts-quality@toolu",
  "python-quality@toolu",
  "rust-quality@toolu",
];

/** Put the sandbox in `c`'s state for `host`: plugins installed and registered, config, setup. */
export async function preparePost(sb: Sandbox, host: PretoolHost, c: PosttoolCase): Promise<void> {
  // No detached gc/maintenance: it would write into `.git` between the two runs.
  sb.git("config", "maintenance.auto", "false");
  sb.git("config", "gc.auto", "0");
  installPlugins(sb, ...SPECS);
  // Each plugin's register.sh writes only its own `<spec>__*` files.
  await Promise.all((c.register ?? []).map((plugin) => registerPlugin(sb, host, plugin)));
  if (c.config !== undefined) sb.writeConfig(host, "project", c.config);
  c.setup?.(sb, host);
}

/** The hook stdin for `c` as `host` delivers it. */
export function postStdin(sb: Sandbox, host: PretoolHost, c: PosttoolCase): string {
  if (c.stdin !== undefined) return c.stdin;
  if (c.fixture === undefined) throw new Error(`${c.name}: neither a fixture nor stdin`);
  return JSON.stringify(toStdin(host, c.fixture(sb), { cwd: sb.project }));
}
