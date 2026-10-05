#!/usr/bin/env bun
/**
 * Hermetic OpenCode documentation check (#363): `bun run check:opencode-docs`.
 *
 * 1. docs/opencode.md's Plugin support section must equal its rendering from
 *    the capability matrix and the acceptance registry; `--write` regenerates
 *    it. A catalog plugin without a dedicated actual-host check fails.
 * 2. The documented install path must not carry V2-only claims: the V2 plugin
 *    commands and packages, the V2 pin and docs, `permission.evaluate`, or the
 *    manual surface wiring the config hook replaced. The migration guide and
 *    the historical conformance report are exempt; they must name V2.
 */
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { envOr } from "./env.ts";
import { readMarked, writeMarked } from "./opencode-host/doc-blocks.ts";
import { contractPaths, ROOT } from "./opencode-host/results.ts";
import {
  ContractError,
  MatrixSchema,
  ProbeResultsSchema,
  readJson,
} from "./opencode-host/schema.ts";
import {
  renderSupport,
  requiredChecks,
  SUPPORT_END,
  SUPPORT_START,
} from "./opencode-acceptance/support-doc.ts";

/** The documented target path, relative to the docs root. */
export const TARGET_DOCS = [
  "README.md",
  "docs/opencode.md",
  "docs/cli.md",
  "docs/plugins/index.md",
  "tools/toolu-opencode/README.md",
];

/** V2-only claims that must not appear on the target path. */
export const STALE_CLAIMS: ReadonlyArray<readonly [RegExp, string]> = [
  [/opencode plugin (add|check|update|remove)/, "V2 plugin command"],
  [/@opencode\/(plugin|cli)\b/, "V2 package"],
  [/(?<![\d.])2\.0\.12(?![\d.])/, "V2 pin"],
  [/opencode\.ai\/v2\//, "V2 docs"],
  [/permission\.evaluate/, "V2 permission hook"],
  [/(?<![\d.])1\.18\.31(?![\d.])/, "superseded pin"],
  [/generated\/skills/, "manual surface wiring"],
];

/** `file:line: label (match)` for every stale claim in the target docs under `root`. */
export function staleClaims(root: string): string[] {
  return TARGET_DOCS.flatMap((rel) => {
    const path = join(root, rel);
    if (!existsSync(path)) throw new ContractError(`missing ${path}`);
    return readFileSync(path, "utf8")
      .split("\n")
      .flatMap((line, index) =>
        STALE_CLAIMS.flatMap(([pattern, label]) => {
          const hit = line.match(pattern);
          return hit === null ? [] : [`${rel}:${index + 1}: ${label} (${hit[0]})`];
        }),
      );
  });
}

function main(argv: readonly string[]): void {
  const env = process.env;
  const docPath = envOr("TOOLU_OPENCODE_DOC", join(ROOT, "docs/opencode.md"), env);
  const paths = contractPaths(env);
  const matrix = readJson(paths.matrix, MatrixSchema);
  const checks = requiredChecks(readJson(paths.results, ProbeResultsSchema));
  const uncovered = Object.keys(matrix.plugins).filter((name) => (checks[name] ?? []).length === 0);
  if (uncovered.length > 0)
    throw new ContractError(`no dedicated actual-host check for: ${uncovered.join(", ")}`);
  const rendered = renderSupport(matrix, checks);
  let doc = readFileSync(docPath, "utf8");
  if (argv.includes("--write")) {
    doc = writeMarked(doc, SUPPORT_START, SUPPORT_END, "support", rendered);
    writeFileSync(docPath, doc);
  }
  if (readMarked(doc, SUPPORT_START, SUPPORT_END, "support") !== rendered)
    throw new ContractError("support block is stale; run bun run check:opencode-docs --write");
  const stale = staleClaims(envOr("TOOLU_DOCS_ROOT", ROOT, env));
  if (stale.length > 0)
    throw new ContractError(`stale V2 claims on the install path:\n${stale.join("\n")}`);
}

if (import.meta.main) {
  try {
    main(process.argv.slice(2));
    process.stdout.write("check-opencode-docs: ok\n");
  } catch (err: unknown) {
    if (!(err instanceof ContractError)) throw err;
    process.stderr.write(`check-opencode-docs: ${err.message}\n`);
    process.exitCode = 1;
  }
}
