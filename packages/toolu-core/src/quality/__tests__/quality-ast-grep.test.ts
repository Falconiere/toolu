/** Real ast-grep scans and bounded executable failure cases from the shared fixture. */
import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { qualityCaseEnv } from "@toolu/conformance/harness/quality-cases";
import { astGrepScan } from "../quality-ast-grep.ts";
import { postContext } from "./quality-harness.ts";
import { SCAN_CASES } from "./runner-cases.ts";

const HAS_AST_GREP = Bun.which("ast-grep") !== null;

for (const c of SCAN_CASES) {
  test.skipIf(c.requiresAstGrep && !HAS_AST_GREP)(c.name, () => {
    using sb = createSandbox({ git: true });
    for (const check of c.checks) {
      sb.write(check.path, check.body);
      const patch =
        check.env?.kind === "stubAstGrep"
          ? qualityCaseEnv(sb, check.env)
          : check.env?.kind === "pathWithoutAstGrep"
            ? { PATH: sb.path("empty-bin") }
            : {};
      const env = patch.PATH === undefined ? {} : { PATH: patch.PATH };
      const file = { path: check.path, absolute: sb.path(check.path), removed: false };
      const result = astGrepScan(file, check.rules, postContext(sb, { env }));
      const expected = check.expect;
      if (expected.kind === "exact") {
        expect<unknown>(result).toEqual(expected.result);
      } else if (expected.kind === "rules") {
        expect(result).toMatchObject({ kind: "ok", empty: expected.empty });
        expect(result.kind === "ok" ? result.hits.length : 0).toBe(expected.count);
        for (const [rule, hits] of Object.entries(expected.hitsByRule)) {
          expect(
            result.kind === "ok" ? result.hits.filter((hit) => hit.ruleId === rule) : [],
          ).toEqual(hits);
        }
      } else if (expected.kind === "lines") {
        const hits = result.kind === "ok" ? result.hits : [];
        expect(hits.map(({ line, text, first }) => ({ line, text, first }))).toEqual(
          expected.lines,
        );
      } else {
        expect(result).toMatchObject({ kind: "failed", stage: expected.stage });
        if (result.kind !== "failed") continue;
        if (expected.nonzeroExit) expect(result.exitCode).not.toBe(0);
        expect(result.stderrFirst.length).toBeGreaterThanOrEqual(expected.stderrMin);
        expect(result.stderrFirst.length).toBeLessThanOrEqual(expected.stderrMax);
      }
    }
  });
}
