/**
 * No-mocks (#266, 70-no-mocks): mock-framework imports and `mocker` /
 * `monkeypatch` fixture parameters in test files, one `ast-grep scan`. Each
 * match shows as `<line>: <its first line>`, cut at a tab as `awk -F'\t'`
 * cut it; without ast-grep the rule is skipped, and a broken scan is itself a
 * violation. Severity is `warning` on purpose: `scan` exits non-zero on an
 * error-level match, which would look like a crashed tool.
 */
import { astGrepScan, type AstGrepHit, type AstGrepScan } from "@toolu/core/quality";
import { head, type PyFile } from "./py-file.ts";

const RULES = `id: mock-import
language: python
severity: warning
message: 'mock import'
rule:
  any:
    - kind: import_statement
      has: {kind: dotted_name, regex: '^(unittest\\.mock|mock|pytest_mock)$'}
    - kind: import_from_statement
      has: {field: module_name, kind: dotted_name, regex: '^(unittest\\.mock|mock)$'}
    - kind: import_from_statement
      all:
        - has: {field: module_name, kind: dotted_name, regex: '^unittest$'}
        - has: {field: name, kind: dotted_name, regex: '^mock$'}
---
id: mocker-param
language: python
severity: warning
message: 'mocker/monkeypatch fixture parameter'
rule:
  kind: function_definition
  has:
    field: parameters
    has: {kind: identifier, regex: '^(mocker|monkeypatch)$'}`;

/** The first five matches of `rule`, as `<line>: <first line>`. */
function matches(hits: readonly AstGrepHit[], rule: string): string {
  const shown = hits
    .filter((hit) => hit.ruleId === rule && hit.first)
    .map((hit) => `${String(hit.line)}: ${hit.text.split("\t")[0] ?? ""}`);
  return head(shown, 5);
}

function failure(scan: AstGrepScan): string | undefined {
  if (scan.kind === "ok") {
    return scan.empty
      ? 'ast-grep exited 0 with empty output (expected at least the JSON array "[]")'
      : undefined;
  }
  if (scan.kind !== "failed") return undefined;
  if (scan.stage === "parse") {
    return "ast-grep exited 0 but its output did not parse as the documented JSON array";
  }
  const first = scan.stderrFirst === "" ? "" : `: ${scan.stderrFirst}`;
  return `ast-grep exit ${String(scan.exitCode)}${first}`;
}

/** Only for `test_*.py` / `*_test.py`, unless `lang.python.noMocks` is false. */
export function noMocks(f: PyFile, isTest: boolean): string[] {
  if (!isTest || !f.limits.noMocks) return [];
  const scan = astGrepScan(f.file, RULES, f.ctx);
  if (scan.kind === "missing") return [];
  const path = f.file.path;
  const hits = scan.kind === "ok" ? scan.hits : [];
  const errors: string[] = [];
  const imports = matches(hits, "mock-import");
  if (imports !== "") {
    errors.push(
      `no-mocks: mock import in ${path} — write against real data/fixtures instead\n${imports}`,
    );
  }
  const params = matches(hits, "mocker-param");
  if (params !== "") {
    errors.push(
      `no-mocks: mocker/monkeypatch fixture parameter in ${path} — write against real data/fixtures instead\n${params}`,
    );
  }
  const broken = failure(scan);
  if (broken !== undefined) {
    errors.push(
      `ast-grep failed while scanning ${path} — ${broken}; no-mocks rules could not be verified. Fix the tool/file and re-edit`,
    );
  }
  return errors;
}
