/**
 * `ownedByLinter` backing. The guardrails runner skips a check a package hands
 * to the linter, so the linter has to run it: every owned id must map to rules
 * enabled at error level in that package's .oxlintrc.json, resolved through
 * `extends`. A package with no lint config, or an id no rule backs, fails.
 */
import { existsSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import {
  OxlintSchema,
  PackageGuardrailsSchema,
  WorkspaceSchema,
  fatal,
  readJson,
} from "./reach-schema.ts";

const WORKSPACE_FILE = "guardrails.workspace.json";

const BACKING: ReadonlyMap<string, readonly string[]> = new Map([
  ["folder-tree", ["house/folder-tree"]],
  ["colocated-tests", ["house/colocated-tests"]],
  ["no-barrels", ["house/no-barrels"]],
  ["filename-case", ["unicorn/filename-case"]],
  // toolu adopted none of the upstream pattern rules, so no rule backs this id.
  ["patterns", []],
]);

/** Rules of the config at `rel`: each `extends` parent in order, then its own. */
function effectiveRules(root: string, rel: string, chain: readonly string[]): Map<string, unknown> {
  if (chain.includes(rel)) fatal(`${rel}: extends itself through ${chain.join(" -> ")}`);
  const config = readJson(root, rel, OxlintSchema);
  const rules = new Map<string, unknown>();
  for (const parent of config.extends) {
    const parentRel = relative(root, resolve(root, dirname(rel), parent));
    for (const [rule, value] of effectiveRules(root, parentRel, [...chain, rel])) {
      rules.set(rule, value);
    }
  }
  for (const [rule, value] of Object.entries(config.rules)) rules.set(rule, value);
  return rules;
}

function isError(value: unknown): boolean {
  const level: unknown = Array.isArray(value) ? value[0] : value;
  return level === "error" || level === "deny" || level === 2;
}

function packageFindings(root: string, pkg: string): string[] {
  const guardrails = readJson(root, `${pkg}/guardrails.config.json`, PackageGuardrailsSchema);
  if (guardrails.ownedByLinter.length === 0) return [];
  const lintConfig = `${pkg}/.oxlintrc.json`;
  const rules = existsSync(join(root, lintConfig))
    ? effectiveRules(root, lintConfig, [])
    : new Map<string, unknown>();
  return guardrails.ownedByLinter.flatMap((id) => {
    const backing = BACKING.get(id);
    if (backing === undefined) {
      return [`gate-reach: ${pkg}: ownedByLinter "${id}" is not a check the linter can own`];
    }
    if (backing.every((rule) => isError(rules.get(rule)))) return [];
    return [
      `gate-reach: ${pkg}: ownedByLinter "${id}" has no rule at error level in ${lintConfig}`,
    ];
  });
}

/** One finding per owned check the package's lint config does not run. */
export function checkOwnedRules(root: string): string[] {
  if (!existsSync(join(root, WORKSPACE_FILE))) return [];
  const { packages } = readJson(root, WORKSPACE_FILE, WorkspaceSchema);
  return packages.flatMap((pkg) => packageFindings(root, pkg));
}
