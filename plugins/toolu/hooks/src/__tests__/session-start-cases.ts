/**
 * SessionStart golden cases (#263): every branch of the bash session-start.sh
 * context, the bats scenarios it was tested with, and the boundaries a port
 * gets wrong (metacharacter names, unknown source, jq `//` on false).
 */
import type { LifecycleCase } from "./lifecycle-cases.ts";

const START = '{"hook_event_name":"SessionStart","source":"startup"}';
const CODEX_LIST =
  '{"installed":[{"pluginId":"toolu@toolu","name":"toolu","marketplaceName":"toolu","installed":true}],"available":[]}';
const TS_REPO = { "tsconfig.json": "{}\n", "bun.lock": "{}\n" };
const DEP = { name: "toolu", dependencies: [{ name: "ts-quality", marketplace: "toolu" }] };

function ss(
  name: string,
  extra: Omit<LifecycleCase, "name" | "hook" | "stdin"> = {},
  stdin = START,
) {
  return { name: `session-start: ${name}`, hook: "session-start", stdin, ...extra } as const;
}

const EVENTS: readonly LifecycleCase[] = [
  ss("startup in a fresh repo"),
  ss("resume", {}, '{"source":"resume"}'),
  ss("clear", {}, '{"source":"clear"}'),
  ss("compact adds the post-compaction doc", {}, '{"source":"compact"}'),
  ss("an unknown source gets no title and no main doc", {}, '{"source":"reboot"}'),
  ss("a numeric source is an unknown event", {}, '{"source":7}'),
  ss("legacy session_event field", {}, '{"session_event":"resume"}'),
  ss("a false source falls through like jq //", {}, '{"source":false,"event":"resume"}'),
  ss("empty stdin is startup", {}, ""),
  ss("invalid JSON stdin is startup", {}, "{not json"),
  ss("outside a git repo", { git: false }),
  ss("a feature branch names itself", { branch: "feat/live-thing" }),
  ss("project name with sed metacharacters", { subdir: "we|ird&name" }),
  ss("dirty tree and failing gate stay out of the prefix", {
    untracked: {
      "file.txt": "dirty\n",
      ".claude/tmp/quality-gate-status.json": '{"status":"failing","reason":"boom"}',
    },
  }),
];

const TOOLCHAINS: readonly LifecycleCase[] = [
  ss("TypeScript repo, verbose unset", { files: TS_REPO }),
  ss("TypeScript repo, TOOLU_VERBOSE=0", { files: TS_REPO, env: { TOOLU_VERBOSE: "0" } }),
  ss("TypeScript repo, TOOLU_VERBOSE=1", { files: TS_REPO, env: { TOOLU_VERBOSE: "1" } }),
  ss("untracked tsconfig is not TypeScript", {
    untracked: { "tsconfig.json": "{}\n" },
    env: { TOOLU_VERBOSE: "1" },
  }),
  ss("rust and python repo, verbose", {
    files: { "Cargo.toml": "[package]\n", "pyproject.toml": "[project]\n", "pnpm-lock.yaml": "" },
    env: { TOOLU_VERBOSE: "true" },
  }),
  ss("nested tsconfig with npm lock, verbose", {
    files: { "web/tsconfig.app.json": "{}\n", "package-lock.json": "{}\n" },
    env: { TOOLU_VERBOSE: "1" },
  }),
];

const MODELS: readonly LifecycleCase[] = [
  ss("config remaps two tiers", {
    userConfig: { version: 1, models: { review: "opus", mechanical: "sonnet" } },
  }),
  ss("an unroutable alias falls back with a warning", {
    userConfig: { version: 1, models: { review: "gpt-9" } },
  }),
  ss("a false model is unset", { userConfig: { version: 1, models: { review: false } } }),
  ss("models.enabled false drops the routing block", {
    userConfig: { version: 1, models: { enabled: false } },
  }),
  ss(
    "routing block survives compact with a remap",
    { userConfig: { models: { review: "opus" } } },
    '{"source":"compact"}',
  ),
  ss("Codex default slugs and efforts", { host: "codex", codexList: CODEX_LIST }),
  ss("Codex project override of one tier", {
    host: "codex",
    codexList: CODEX_LIST,
    untracked: {
      ".codex/toolu.config.json":
        '{"models":{"codex":{"review":{"model":"review-local","reasoningEffort":"xhigh"}}}}',
    },
  }),
  ss("Codex bad effort falls back alone", {
    host: "codex",
    codexList: CODEX_LIST,
    userConfig: { models: { codex: { review: { model: "r", reasoningEffort: "extreme" } } } },
  }),
];

const NOTICES: readonly LifecycleCase[] = [
  ss("first run announces gates, workflow move and permissions", { firstRun: true }),
  ss("Codex first run names the npx install", {
    firstRun: true,
    host: "codex",
    codexList: CODEX_LIST,
  }),
  ss("a pinned preset silences the gate notice", {
    firstRun: true,
    untracked: { ".claude/toolu.config.json": '{"version":1,"gates":{"preset":"strict"}}' },
  }),
  ss("a per-gate mode silences the gate notice", {
    firstRun: true,
    untracked: {
      ".claude/toolu.config.json": '{"version":1,"gates":{"pushReview":{"mode":"ask"}}}',
    },
  }),
  ss("sweep and ttl alone still announce", {
    firstRun: true,
    untracked: {
      ".claude/toolu.config.json": '{"version":1,"gates":{"sweep":true,"stateTtlHours":24}}',
    },
  }),
  ss("a non-object gates still announces", {
    firstRun: true,
    untracked: { ".claude/toolu.config.json": '{"version":1,"gates":[]}' },
  }),
  ss("autoAllow false skips the permission write", {
    firstRun: true,
    userConfig: { permissions: { autoAllow: false } },
  }),
];

const MANDATES: readonly LifecycleCase[] = [
  ss("ast-grep installed and on PATH", {
    astGrep: true,
    registry: { plugins: { "ast-grep@toolu": {} } },
  }),
  ss("ast-grep plugin absent", { astGrep: true, registry: { plugins: {} } }),
  ss("ast-grep skill disabled hides the missing-tool warning", {
    userConfig: { skills: { "ast-grep": false } },
    registry: { plugins: {} },
  }),
  ss("exa-search mandate with key and wrapper", {
    registry: { plugins: { "exa-search@toolu": {} } },
    wrappers: ["exa-search"],
    env: { EXA_API_KEY: "test-key" },
  }),
  ss("exa-search without a key", {
    registry: { plugins: { "exa-search@toolu": {} } },
    wrappers: ["exa-search"],
  }),
  ss("exa-search without a wrapper", {
    registry: { plugins: { "exa-search@toolu": {} } },
    env: { EXA_API_KEY: "k" },
  }),
  ss("exa-search skill disabled", {
    registry: { plugins: { "exa-search@toolu": {} } },
    wrappers: ["exa-search"],
    userConfig: { skills: { "exa-search": false } },
    env: { EXA_API_KEY: "k" },
  }),
  ss("context7 mandate with wrapper", {
    registry: { plugins: { "context7@toolu": {} } },
    wrappers: ["context7"],
  }),
  ss("context7 without a wrapper", { registry: { plugins: { "context7@toolu": {} } } }),
  ss("context7 and exa plugins absent", {
    registry: { plugins: {} },
    wrappers: ["context7", "exa-search"],
    env: { EXA_API_KEY: "k" },
  }),
  ss("context7 skill disabled", {
    registry: { plugins: { "context7@toolu": {} } },
    wrappers: ["context7"],
    userConfig: { skills: { context7: false } },
  }),
  ss("all three mandates, no registry file (fail open)", {
    astGrep: true,
    wrappers: ["context7", "exa-search"],
    env: { EXA_API_KEY: "k" },
  }),
];

const DEPENDENCIES: readonly LifecycleCase[] = [
  ss("missing dependency warns with the install command", {
    manifest: DEP,
    registry: { plugins: {} },
  }),
  ss("installed dependency is silent", {
    manifest: DEP,
    registry: { plugins: { "ts-quality@toolu": {} } },
  }),
  ss("missing registry suppresses dependency warnings", { manifest: DEP }),
  ss("malformed registry suppresses dependency warnings", {
    manifest: DEP,
    registry: { plugins: [] },
  }),
  ss("no dependencies key", { manifest: { name: "toolu" }, registry: { plugins: {} } }),
  ss("nameless entry and scalar entry are skipped", {
    manifest: {
      dependencies: [
        { marketplace: "toolu" },
        42,
        "",
        { name: "ts-quality", marketplace: "toolu" },
      ],
    },
    registry: { plugins: {} },
  }),
  ss("string, marketplace-less and object-valued dependencies", {
    manifest: {
      dependencies: { a: "jev@toolu", b: { name: "jira" }, c: { name: "x", marketplace: false } },
    },
    registry: { plugins: {} },
  }),
  ss("scalar dependencies yield nothing", {
    manifest: { dependencies: "ts-quality" },
    registry: { plugins: {} },
  }),
  ss("Codex reads shared dependencies and names codex plugin add", {
    host: "codex",
    codexList: '{"installed":[]}',
    manifest: { name: "dependent", dependencies: [{ name: "toolu", marketplace: "toolu" }] },
    codexManifest: { name: "dependent", version: "1.0.0", description: "test" },
  }),
  ss("Codex without a snapshot is indeterminate", {
    host: "codex",
    manifest: { name: "dependent", dependencies: [{ name: "toolu", marketplace: "toolu" }] },
    codexManifest: { name: "dependent" },
  }),
];

export const SESSION_START_CASES: readonly LifecycleCase[] = [
  ...EVENTS,
  ...TOOLCHAINS,
  ...MODELS,
  ...NOTICES,
  ...MANDATES,
  ...DEPENDENCIES,
];
