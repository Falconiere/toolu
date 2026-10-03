/**
 * The structural rules (#265): error handling (78) and mock calls in tests
 * (85), each one `ast-grep scan` through `@toolu/core/quality`. Without
 * ast-grep they are skipped, as in bash; a broken scan is itself a violation.
 */
import { astGrepScan, type AstGrepHit, type AstGrepScan } from "@toolu/core/quality";
import { ERROR_RULES, MOCK_RULES } from "./ast-rule-yaml.ts";
import { ere, isTestPath, type TsFile } from "./ts-file.ts";

const ERROR_RULES_TSX = ERROR_RULES.replaceAll("language: ts\n", "language: tsx\n");
const MOCK_RULES_TSX = MOCK_RULES.replaceAll("language: ts\n", "language: tsx\n");

/** OpenCode must give ast-grep TSX rules for `.tsx`; retain the earlier host behavior. */
function scan(f: TsFile, ts: string, tsx: string): AstGrepScan {
  const rules = f.ctx.host === "opencode" && f.file.path.endsWith(".tsx") ? tsx : ts;
  return astGrepScan(f.file, rules, f.ctx);
}

/** One rule's excerpts, capped; `read`'s field split drops trailing tabs. */
function hits(all: readonly AstGrepHit[], rule: string, limit: number): string[] {
  return all
    .filter((hit) => hit.ruleId === rule)
    .slice(0, limit)
    .map((hit) => hit.excerpt.replace(/\t+$/, ""));
}

function group(
  f: TsFile,
  all: readonly AstGrepHit[],
  header: string,
  rules: readonly [string, number][],
) {
  const lines = rules.flatMap(([rule, limit]) => hits(all, rule, limit));
  return lines.length === 0 ? [] : [`${header.replace("$F", f.file.path)}\n${lines.join("\n")}`];
}

function scanFailure(scan: AstGrepScan): string | undefined {
  if (scan.kind !== "failed") return undefined;
  if (scan.stage === "parse")
    return "ast-grep exited 0 but its output did not parse as the documented JSON array";
  const first = scan.stderrFirst === "" ? "" : `: ${scan.stderrFirst}`;
  return `ast-grep exit ${String(scan.exitCode)}${first}`;
}

/** 78-error-ast. */
export function errorHandling(f: TsFile): string[] {
  const result = scan(f, ERROR_RULES, ERROR_RULES_TSX);
  if (result.kind === "missing") return [];
  const all = result.kind === "ok" ? result.hits : [];
  const errors = [
    ...group(f, all, "Empty catch block in $F — handle the error or rethrow; do not swallow", [
      ["empty-catch", 3],
      ["empty-catch-noarg", 3],
    ]),
    ...group(f, all, "Silent promise rejection in $F — log or rethrow the error", [
      ["empty-catch-handler", 3],
      ["null-catch-handler", 3],
      ["undef-catch-handler", 3],
    ]),
    ...group(
      f,
      all,
      "Catch swallows the error by returning a nullish value in $F — handle, log, or rethrow it",
      [
        ["swallow-null-arg", 2],
        ["swallow-undef-arg", 2],
        ["swallow-null", 2],
        ["swallow-undef", 2],
        ["swallow-bare", 2],
      ],
    ),
    ...group(f, all, "throw new Error() with no message in $F — include a descriptive message", [
      ["throw-empty-error", 3],
    ]),
    ...group(f, all, "throw of string literal in $F — throw an Error (or subclass) instead", [
      ["throw-string", 3],
      ["throw-template", 3],
    ]),
  ];
  const failure = scanFailure(result);
  if (failure !== undefined) {
    errors.push(
      `ast-grep failed while scanning ${f.file.path} — ${failure}; error-handling rules could not be verified. Fix the tool/file and re-edit`,
    );
  }
  return errors;
}

function mockScanFailure(f: TsFile, scan: AstGrepScan): string | undefined {
  const at = `ast-grep failed while scanning ${f.file.path} for mocks —`;
  const tail = "no-mocks rule could not be verified. Fix the tool/file and re-edit";
  if (scan.kind === "ok" && scan.empty) {
    return `${at} exited 0 with empty output (expected at least the JSON array "[]"); ${tail}`;
  }
  if (scan.kind !== "failed") return undefined;
  if (scan.stage === "parse")
    return `${at} its output did not parse as the documented JSON array; ${tail}`;
  const first = scan.stderrFirst === "" ? "" : `: ${scan.stderrFirst}`;
  return `${at} exit ${String(scan.exitCode)}${first}; ${tail}`;
}

/**
 * 85-no-mocks, for test files outside e2e unless `lang.ts.noMocks` is false.
 * Hits are shown in source order: bash printed ast-grep's multi-rule output
 * unsorted, an order that varied from run to run.
 */
export function mockDoubles(f: TsFile): string[] {
  const path = f.file.path;
  const inTests = isTestPath(path) || path.includes("/__tests__/");
  if (path.includes("/e2e/") || !inTests || !f.limits.noMocks) return [];
  const errors: string[] = [];
  const result = scan(f, MOCK_RULES, MOCK_RULES_TSX);
  const failure = result.kind === "missing" ? undefined : mockScanFailure(f, result);
  if (failure !== undefined) errors.push(failure);
  const found = result.kind === "ok" ? result.hits.toSorted((a, b) => a.line - b.line) : [];
  if (found.length > 0) {
    const excerpt = found
      .slice(0, 5)
      .map((hit) => hit.excerpt)
      .join("\n");
    errors.push(
      `Mocked test double in ${path} — tests must exercise real data/services, not mocks/stubs (jest.mock/vi.mock/jest.fn/vi.fn/sinon)\n${excerpt}`,
    );
  }
  if (f.lines.some((line) => ere(`from[[:space:]]+["']ts-mockito["']`).test(line))) {
    errors.push(
      `Import from ts-mockito in ${path} — tests must exercise real data/services, not mocks/stubs`,
    );
  }
  return errors;
}
