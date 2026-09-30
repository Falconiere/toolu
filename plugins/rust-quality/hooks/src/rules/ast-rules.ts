/**
 * The structural rules (#267): panic-on-error in production code (60) and
 * mock frameworks (70), each one `ast-grep scan` through `@toolu/core/quality`.
 * Without ast-grep they are skipped, as in bash; a broken scan is itself a
 * violation. Hits are filtered per rule, which keeps ast-grep's order stable.
 */
import { astGrepScan, type AstGrepHit, type AstGrepScan } from "@toolu/core/quality";
import { ERROR_RULES, MOCK_SRC_RULES, MOCK_TEST_RULES } from "./ast-rule-yaml.ts";
import { pathHas, testFileName, type RsFile } from "./rs-file.ts";

/** One rule's excerpts, capped; `read`'s field split drops trailing tabs. */
function hits(all: readonly AstGrepHit[], rule: string, limit: number): string[] {
  return all
    .filter((hit) => hit.ruleId === rule)
    .slice(0, limit)
    .map((hit) => hit.excerpt.replace(/\t+$/, ""));
}

function group(
  f: RsFile,
  all: readonly AstGrepHit[],
  header: string,
  rules: readonly [string, number][],
): string[] {
  const lines = rules.flatMap(([rule, limit]) => hits(all, rule, limit));
  return lines.length === 0
    ? []
    : [`${header.replace("$F", () => f.file.path)}\n${lines.join("\n")}`];
}

function failureDetail(scan: AstGrepScan): string | undefined {
  if (scan.kind !== "failed") return undefined;
  if (scan.stage === "parse")
    return "ast-grep exited 0 but its output did not parse as the documented JSON array";
  const first = scan.stderrFirst === "" ? "" : `: ${scan.stderrFirst}`;
  return `ast-grep exit ${String(scan.exitCode)}${first}`;
}

function failure(f: RsFile, detail: string | undefined, rules: string): string[] {
  if (detail === undefined) return [];
  return [
    `ast-grep failed while scanning ${f.file.path} — ${detail}; ${rules} rules could not be verified. Fix the tool/file and re-edit`,
  ];
}

/** 60-error-handling, for non-test files under src/. */
export function errorHandling(f: RsFile, isTest: boolean): string[] {
  if (!pathHas(f, "/src/") || isTest) return [];
  const scan = astGrepScan(f.file, ERROR_RULES, f.ctx);
  if (scan.kind === "missing") return [];
  const all = scan.kind === "ok" ? scan.hits : [];
  return [
    ...group(f, all, ".unwrap() in $F — use ? or match on Result/Option", [["unwrap", 5]]),
    ...group(f, all, ".expect() in $F — use ? or match on Result/Option", [["expect", 5]]),
    ...group(f, all, "panic!/todo!/unimplemented!/unreachable! in $F — return a Result instead", [
      ["panic", 3],
      ["todo", 3],
      ["unimplemented", 3],
      ["unreachable", 3],
    ]),
    ...failure(f, failureDetail(scan), "error-handling"),
  ];
}

function mockRules(f: RsFile): string {
  const path = f.file.path;
  const inTests = path.includes("/tests/") || testFileName(path);
  const rules = [pathHas(f, "/src/") ? MOCK_SRC_RULES : "", inTests ? MOCK_TEST_RULES : ""];
  return rules.filter((yaml) => yaml !== "").join("\n---\n");
}

/** 70-no-mocks: definitions in src/, imports in test files, unless `lang.rust.noMocks` is false. */
export function noMocks(f: RsFile): string[] {
  if (!f.limits.noMocks) return [];
  const rules = mockRules(f);
  if (rules === "") return [];
  const scan = astGrepScan(f.file, rules, f.ctx);
  if (scan.kind === "missing") return [];
  const all = scan.kind === "ok" ? scan.hits : [];
  const detail =
    scan.kind === "ok" && scan.empty
      ? 'ast-grep exited 0 with empty output (expected at least the JSON array "[]")'
      : failureDetail(scan);
  const tail = "— write against real data/fixtures instead";
  return [
    ...group(f, all, `no-mocks: #[automock] mock definition in $F ${tail}`, [
      ["automock-attr", 5],
      ["automock-cfg-attr", 5],
    ]),
    ...group(f, all, `no-mocks: mock! { ... } mock definition in $F ${tail}`, [["mock-macro", 5]]),
    ...group(f, all, `no-mocks: mockall/faux import in $F ${tail}`, [
      ["mockall-import", 5],
      ["faux-import", 5],
    ]),
    ...failure(f, detail, "no-mocks"),
  ];
}
