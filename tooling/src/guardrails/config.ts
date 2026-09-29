/**
 * Locates, validates and reads guardrails.config.json (per package) and
 * guardrails.workspace.json (monorepo root).
 *
 * Fails CLOSED: invalid JSON, a missing required key, an unknown key or a
 * field of the wrong type exits 3 rather than falling back to a default. A
 * guard rail that silently stops enforcing is worse than none.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fatal, warn } from "./report.ts";
import {
  entriesOf,
  jqText,
  numberField,
  objectOf,
  optionalArray,
  optionalStrings,
  stringField,
  stringList,
  strings,
} from "./shape.ts";
import type { Doc } from "./shape.ts";

/** Bumped when a change requires a project to re-copy something; older configs warn. */
const SCRIPTS_VERSION = 2;

const REQUIRED =
  "version srcRoot src fileSize functionSize testDir testGlob barrelNames bannedDeps shadowConfigs";
const OPTIONAL = "$schema barrelExempt requiredFiles secrets filenameCase ownedByLinter";
export const WORKSPACE_FILE = "guardrails.workspace.json";
const WS_REQUIRED = "version packages";
const WS_OPTIONAL = "$schema bannedDeps secrets shadowConfigs requiredFiles";

export type ShadowConfig = { found: string; use: string; why: string };
export type RequiredFile = { path: string; why: string };
export type FilenameRule = { glob: string; regex: string; describe: string };

/** What the four repo-level checks read; a workspace manifest carries only these. */
export type RepoFacts = {
  readonly bannedDeps: readonly string[];
  readonly secretFiles: readonly string[];
  readonly shadowConfigs: readonly ShadowConfig[];
  readonly requiredFiles: readonly RequiredFile[];
};

export type GuardrailsConfig = RepoFacts & {
  readonly srcRoot: string;
  readonly fileMax: number;
  readonly testDir: string;
  readonly testGlobs: readonly string[];
  /** null: `src.topLevel` omitted, unconstrained. [] allows nothing. */
  readonly topLevel: readonly string[] | null;
  readonly requireReadme: readonly string[];
  /** src.nested in document order; first matching key wins. */
  readonly nested: ReadonlyArray<readonly [string, readonly string[]]>;
  readonly barrelNames: readonly string[];
  readonly barrelExempt: readonly string[];
  readonly skipExtensions: readonly string[];
  /** fileSize.overrides in document order; first matching glob wins. */
  readonly sizeOverrides: ReadonlyArray<readonly [string, number]>;
  readonly secretScanExempt: readonly string[];
  readonly filenameCase: readonly FilenameRule[];
  readonly ownedByLinter: readonly string[];
};

export type WorkspaceManifest = RepoFacts & { readonly packages: readonly string[] };

function readDoc(cwd: string, file: string): Doc {
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(resolve(cwd, file), "utf8"));
  } catch {
    fatal(`${file} is not valid JSON — fix the syntax; the gate cannot run without it`);
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    fatal(`${file} is not a JSON object — the gate cannot read its settings`);
  }
  return new Map(Object.entries(parsed));
}

/** A missing required key or an unknown key (almost always a typo) is fatal. */
function validateKeys(file: string, doc: Doc, required: string, optional: string): void {
  const requiredKeys = required.split(" ");
  const known = new Set([...requiredKeys, ...optional.split(" ")]);
  for (const key of requiredKeys) {
    if (!doc.has(key)) fatal(`${file} is missing required key: ${key}`);
  }
  for (const key of [...doc.keys()].toSorted()) {
    if (!known.has(key)) {
      fatal(`${file} has unknown key: ${key} (typo? known keys: ${required} ${optional})`);
    }
  }
  const version = doc.get("version");
  const text = version === null || version === false || version === undefined ? "0" : jqText(version);
  if (/^-?\d+$/.test(text) && Number(text) < SCRIPTS_VERSION) {
    warn(
      `${file} is version ${text} but these scripts are version ${String(SCRIPTS_VERSION)} — re-copy scripts/guardrails/ and the config from the kit`,
    );
  }
}

function repoFacts(file: string, doc: Doc): RepoFacts {
  const secrets = objectOf(file, doc, "secrets");
  return {
    bannedDeps: optionalStrings(file, doc, "bannedDeps"),
    secretFiles: optionalStrings(file, secrets, "neverTracked"),
    shadowConfigs: optionalArray(file, doc, "shadowConfigs", (entry) => ({
      found: stringField(file, entry, "found"),
      use: stringField(file, entry, "use"),
      why: stringField(file, entry, "why"),
    })),
    requiredFiles: optionalArray(file, doc, "requiredFiles", (entry) => ({
      path: stringField(file, entry, "path"),
      why: stringField(file, entry, "why"),
    })),
  };
}

function treeFacts(file: string, doc: Doc): Omit<GuardrailsConfig, keyof RepoFacts> {
  const src = objectOf(file, doc, "src");
  const fileSize = objectOf(file, doc, "fileSize");
  const secrets = objectOf(file, doc, "secrets");
  const topLevel = src.get("topLevel");
  return {
    srcRoot: stringField(file, doc, "srcRoot"),
    fileMax: numberField(file, fileSize, "max"),
    testDir: stringField(file, doc, "testDir"),
    testGlobs: stringField(file, doc, "testGlob").split(/\s+/).filter(Boolean),
    topLevel: topLevel === undefined || topLevel === null ? null : strings(file, src, "topLevel"),
    requireReadme: optionalStrings(file, src, "requireReadme"),
    nested: entriesOf(file, src, "nested", stringList),
    barrelNames: strings(file, doc, "barrelNames"),
    barrelExempt: optionalStrings(file, doc, "barrelExempt"),
    skipExtensions: optionalStrings(file, fileSize, "skipExtensions"),
    sizeOverrides: entriesOf(file, fileSize, "overrides", (value) =>
      typeof value === "number" ? value : null,
    ),
    secretScanExempt: optionalStrings(file, secrets, "scanExempt"),
    filenameCase: optionalArray(file, doc, "filenameCase", (entry) => ({
      glob: stringField(file, entry, "glob"),
      regex: stringField(file, entry, "regex"),
      describe: stringField(file, entry, "describe"),
    })),
    ownedByLinter: optionalStrings(file, doc, "ownedByLinter"),
  };
}

/** One package's config: `file` is GR_CONFIG or guardrails.config.json. */
export function loadConfig(cwd: string, file: string, exists: boolean): GuardrailsConfig {
  if (!exists) {
    fatal(`no config at ${file} — copy guardrails.config.json from the stack kit, or set GR_CONFIG`);
  }
  const doc = readDoc(cwd, file);
  validateKeys(file, doc, REQUIRED, OPTIONAL);
  strings(file, doc, "bannedDeps");
  return { ...repoFacts(file, doc), ...treeFacts(file, doc) };
}

/** The workspace manifest: packages plus the repo-level facts. */
export function loadManifest(cwd: string): WorkspaceManifest {
  const doc = readDoc(cwd, WORKSPACE_FILE);
  validateKeys(WORKSPACE_FILE, doc, WS_REQUIRED, WS_OPTIONAL);
  return { ...repoFacts(WORKSPACE_FILE, doc), packages: strings(WORKSPACE_FILE, doc, "packages") };
}
