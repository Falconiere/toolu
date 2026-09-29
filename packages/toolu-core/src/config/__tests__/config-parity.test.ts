/**
 * Bash parity (#253): every config fixture, placed as user, project, or both,
 * resolves to the same thresholds, flags, model tiers, docs-sync globs, string
 * enums and gate modes in the bash libs and in `@toolu/core/config`.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { loadConfig, type LoadedConfig } from "../config-load.ts";
import {
  codexModel,
  configString,
  enabled,
  enabledExplicit,
  flagFalse,
  flagTrue,
  model,
  MODEL_CLASSES,
} from "../config-read.ts";
import {
  docsSyncCodeSurfaces,
  docsSyncSurfaceExcludes,
  docsSyncSurfaces,
} from "../docs-sync-config.ts";
import { GATE_NAMES, gateMode, gatePreset } from "../gate-mode.ts";
import { qualityFlag, qualityThreshold, tsMaxFileLinesResolved } from "../quality-config.ts";

const REPO = resolve(import.meta.dir, "../../../../..");
const LIB = join(REPO, "plugins/toolu/hooks/lib");
const FIXTURES = join(REPO, "tooling/fixtures/config");
const EXAMPLE = join(REPO, "plugins/toolu/settings/toolu.config.example.json");
const OVERLAY = readFileSync(join(FIXTURES, "merge-project.json"), "utf8");

type Host = "claude" | "codex";
type Values = Record<string, string>;

/** Every `toolu_enabled`-family key a real call site reads. */
const FLAG_KEYS = [
  ["skills", "ast-grep"],
  ["skills", "exa-search"],
  ["skills", "context7"],
  ["hooks", "session-start"],
  ["hooks", "user-prompt-submit"],
  ["hooks", "pre-tools"],
  ["hooks", "post-tools"],
  ["hooks", "pre-compact"],
  ["agents", "research-agent"],
  ["models", "enabled"],
  ["telemetry", "enabled"],
  ["gates", "sweep"],
  ["planLedger", "blockOnUncoveredAcs"],
  ["permissions", "autoAllow"],
] as const;

type At = { cwd: string };
const THRESHOLDS: readonly [string, string, (c: LoadedConfig, at: At) => number][] = [
  [
    "ts.maxFileLines",
    "ts_max_file_lines",
    (c, at) => qualityThreshold(c, "ts", "maxFileLines", at),
  ],
  ["ts.maxFnLines", "ts_max_fn_lines", (c, at) => qualityThreshold(c, "ts", "maxFnLines", at)],
  [
    "rust.maxFileLines",
    "rust_max_file_lines",
    (c, at) => qualityThreshold(c, "rust", "maxFileLines", at),
  ],
  [
    "rust.maxFnLines",
    "rust_max_fn_lines",
    (c, at) => qualityThreshold(c, "rust", "maxFnLines", at),
  ],
  [
    "rust.maxImplLines",
    "rust_max_impl_lines",
    (c, at) => qualityThreshold(c, "rust", "maxImplLines", at),
  ],
  [
    "python.maxFileLines",
    "python_max_file_lines",
    (c, at) => qualityThreshold(c, "python", "maxFileLines", at),
  ],
  [
    "python.maxFnLines",
    "python_max_fn_lines",
    (c, at) => qualityThreshold(c, "python", "maxFnLines", at),
  ],
];

const LANGS = ["ts", "rust", "python"] as const;
const SEP = "\u001f";

/** One bash process printing `key=value` lines for everything resolved. */
function bashScript(host: Host): string {
  const lines = [
    `for f in config quality-config docs-sync-config gate-mode; do . "$1/$f.sh"; done`,
    `p() { printf '%s=%s\\n' "$1" "$2"; }`,
    `list() { "$1" | tr '\\n' '\\037'; }`,
    `p preset "$(toolu_gate_preset)"`,
    ...GATE_NAMES.map((name) => `p gate.${name} "$(toolu_gate_mode ${name})"`),
  ];
  if (host === "codex") {
    return lines.join("\n");
  }
  return [
    ...lines,
    ...THRESHOLDS.map(([key, fn]) => `p threshold.${key} "$(${fn})"`),
    `p ts.resolved "$(ts_max_file_lines_resolved)"`,
    ...LANGS.map((lang) => `p noMocks.${lang} "$(quality_flag ${lang} noMocks true)"`),
    ...MODEL_CLASSES.map((cls) => `p model.${cls} "$(toolu_model ${cls})"`),
    ...MODEL_CLASSES.map((cls) => `p codex.${cls} "$(toolu_codex_model ${cls} | tr '\\t' '/')"`),
    `p docs.surfaces "$(list docs_sync_surfaces)"`,
    `p docs.excludes "$(list docs_sync_surface_excludes)"`,
    `p docs.code "$(list docs_sync_code_surfaces)"`,
    `p string.docsSync "$(toolu_string docsSync.mode advise advise block off)"`,
    `p string.agentTier "$(toolu_string agentTier.mode advise advise block off)"`,
    ...FLAG_KEYS.flatMap(([cat, name]) =>
      ["toolu_enabled", "toolu_flag_true", "toolu_flag_false", "toolu_enabled_explicit"].map(
        (fn) => `p ${fn}.${cat}.${name} "$(${fn} ${cat} ${name} && echo 1 || echo 0)"`,
      ),
    ),
  ].join("\n");
}

function parseValues(stdout: string): Values {
  const values: Values = {};
  for (const line of stdout.split("\n")) {
    const at = line.indexOf("=");
    if (at > 0) values[line.slice(0, at)] = line.slice(at + 1);
  }
  return values;
}

function envFor(sb: Sandbox, host: Host): Record<string, string> {
  return { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project, TOOLU_HOST_OVERRIDE: host };
}

async function bashValues(sb: Sandbox, host: Host): Promise<Values> {
  const res = await run(["bash", "-c", bashScript(host), "_", LIB], {
    cwd: sb.project,
    env: envFor(sb, host),
  });
  expect(res.exitCode).toBe(0);
  return parseValues(res.stdout);
}

const bit = (value: boolean): string => (value ? "1" : "0");
const joined = (items: readonly string[]): string => items.map((item) => item + SEP).join("");

function gateValues(c: LoadedConfig): Values {
  const values: Values = { preset: gatePreset(c) };
  for (const name of GATE_NAMES) values[`gate.${name}`] = gateMode(c, name);
  return values;
}

function tsValues(c: LoadedConfig, sb: Sandbox): Values {
  const values = gateValues(c);
  const at = { cwd: sb.project };
  for (const [key, , threshold] of THRESHOLDS) values[`threshold.${key}`] = String(threshold(c, at));
  const resolved = tsMaxFileLinesResolved(c, at);
  values["ts.resolved"] = `${String(resolved.value)} ${resolved.source}`;
  for (const lang of LANGS)
    values[`noMocks.${lang}`] = String(qualityFlag(c, lang, "noMocks", true));
  for (const cls of MODEL_CLASSES) {
    values[`model.${cls}`] = model(c, cls);
    const codex = codexModel(c, cls);
    values[`codex.${cls}`] = `${codex.model}/${codex.reasoningEffort}`;
  }
  values["docs.surfaces"] = joined(docsSyncSurfaces(c));
  values["docs.excludes"] = joined(docsSyncSurfaceExcludes(c));
  values["docs.code"] = joined(docsSyncCodeSurfaces(c));
  const modes = ["advise", "block", "off"] as const;
  values["string.docsSync"] = configString(c, "docsSync.mode", "advise", modes);
  values["string.agentTier"] = configString(c, "agentTier.mode", "advise", modes);
  for (const [cat, name] of FLAG_KEYS) {
    values[`toolu_enabled.${cat}.${name}`] = bit(enabled(c, cat, name));
    values[`toolu_flag_true.${cat}.${name}`] = bit(flagTrue(c, cat, name));
    values[`toolu_flag_false.${cat}.${name}`] = bit(flagFalse(c, cat, name));
    values[`toolu_enabled_explicit.${cat}.${name}`] = bit(enabledExplicit(c, cat, name));
  }
  return values;
}

type Placement = "user" | "project" | "both";

function writeConfig(sb: Sandbox, host: Host, scope: "user" | "project", text: string): void {
  const dir = sb.configDir(host, scope);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "toolu.config.json"), text);
}

/** user: fixture alone; project: fixture alone; both: fixture as user under the overlay. */
function place(sb: Sandbox, host: Host, placement: Placement, text: string): void {
  if (placement === "project") {
    writeConfig(sb, host, "project", text);
    return;
  }
  writeConfig(sb, host, "user", text);
  if (placement === "both") writeConfig(sb, host, "project", OVERLAY);
}

async function compare(sb: Sandbox, host: Host): Promise<void> {
  const loaded = loadConfig({ env: envFor(sb, host), host, cwd: sb.project, warn: () => {} });
  expect(loaded.invalid).toBeUndefined();
  const ts = host === "claude" ? tsValues(loaded, sb) : gateValues(loaded);
  expect(ts).toEqual(await bashValues(sb, host));
}

const PARITY_FIXTURES = [
  ...readdirSync(FIXTURES)
    .filter((name) => name.endsWith(".json"))
    .filter((name) => !name.startsWith("fail-closed-") && name !== "merge-project.json")
    .map((name) => join(FIXTURES, name)),
  EXAMPLE,
];

test("the parity set covers the docs examples, edge cases and the shipped example", () => {
  const names = PARITY_FIXTURES.map((path) => path.split("/").pop());
  expect(names.filter((name) => name?.startsWith("docs-")).length).toBeGreaterThanOrEqual(6);
  expect(names).toContain("edge-values.json");
  expect(names).toContain("toolu.config.example.json");
});

for (const path of PARITY_FIXTURES) {
  const text = readFileSync(path, "utf8");
  const name = path.split("/").pop() ?? path;
  for (const placement of ["user", "project", "both"] as const) {
    for (const host of ["claude", "codex"] as const) {
      test.concurrent(`${name} as ${placement} on ${host} matches bash`, async () => {
        using sb = createSandbox({ git: true });
        place(sb, host, placement, text);
        await compare(sb, host);
      });
    }
  }
}

test.concurrent("no config at all matches bash", async () => {
  using sb = createSandbox({ git: true });
  await compare(sb, "claude");
});

for (const [label, text] of [
  ["truncated", '{"gates":'],
  ["empty", ""],
] as const) {
  test.concurrent(`malformed project file (${label}) under a valid user file matches bash`, async () => {
    using sb = createSandbox({ git: true });
    writeConfig(sb, "claude", "user", readFileSync(join(FIXTURES, "edge-legacy.json"), "utf8"));
    writeConfig(sb, "claude", "project", text);
    await compare(sb, "claude");
  });
}

const OXLINT_222 = JSON.stringify({ rules: { "max-lines": ["error", { max: 222 }] } });
const NATIVE_CASES: [string, Record<string, string>, string?][] = [
  ["oxlint object form", { ".oxlintrc.json": OXLINT_222 }],
  [
    "eslint array form",
    { ".eslintrc.json": JSON.stringify({ rules: { "max-lines": ["error", 111] } }) },
  ],
  [
    "eslint numeric string",
    { ".eslintrc.json": JSON.stringify({ rules: { "max-lines": "150" } }) },
  ],
  [
    "oxlint numeric severity",
    { ".oxlintrc.json": JSON.stringify({ rules: { "max-lines": [2, 90.7] } }) },
  ],
  ["oxlint off", { ".oxlintrc.json": JSON.stringify({ rules: { "max-lines": ["off", 100] } }) }],
  ["oxlint bare off", { ".oxlintrc.json": JSON.stringify({ rules: { "max-lines": "off" } }) }],
  [
    "both linters: oxc wins",
    {
      ".oxlintrc.json": OXLINT_222,
      ".eslintrc.json": JSON.stringify({ rules: { "max-lines": 111 } }),
    },
  ],
  ["biome shadows oxlint", { "biome.json": "{}", ".oxlintrc.json": OXLINT_222 }],
  ["eslint js config only", { ".eslintrc.js": "module.exports = {};" }],
  ["malformed oxlint", { ".oxlintrc.json": "{ // comment\n}" }],
  [
    "override beats native",
    { ".oxlintrc.json": OXLINT_222 },
    '{"lang":{"ts":{"maxFileLines":80}}}',
  ],
];

for (const [label, files, projectConfig] of NATIVE_CASES) {
  test.concurrent(`native max-lines: ${label} matches bash`, async () => {
    using sb = createSandbox({ git: true, files });
    if (projectConfig !== undefined) writeConfig(sb, "claude", "project", projectConfig);
    await compare(sb, "claude");
  });
}

for (const name of [
  "fail-closed-unknown-key.json",
  "fail-closed-version-2.json",
  "fail-closed-array.json",
]) {
  test.concurrent(`${name}: TS blocks every gate where bash reads relaxed/balanced (deliberate)`, async () => {
    using sb = createSandbox({ git: true });
    writeConfig(sb, "claude", "project", readFileSync(join(FIXTURES, name), "utf8"));
    const loaded = loadConfig({
      env: envFor(sb, "claude"),
      host: "claude",
      cwd: sb.project,
      warn: () => {},
    });
    expect(loaded.invalid).toBeDefined();
    const ts = gateValues(loaded);
    for (const gate of GATE_NAMES) expect(ts[`gate.${gate}`]).toBe("block");
    const bash = await bashValues(sb, "claude");
    const bashGates = GATE_NAMES.map((gate) => bash[`gate.${gate}`]);
    expect(bashGates).toHaveLength(GATE_NAMES.length);
    expect(bashGates.every((mode) => mode === "block")).toBe(false);
  });
}
