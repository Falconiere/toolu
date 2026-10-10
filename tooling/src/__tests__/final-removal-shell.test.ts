/** Real Git inventories exercise the final-removal shell rule and its narrow shims. */
import { expect, test } from "bun:test";
import { appendFileSync, chmodSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { shellProblems } from "../final-removal-shell.ts";

const JEV = "plugins/jev/scripts/jev.sh";
const REVIEW = "plugins/toolu-review/scripts/write-state.sh";

function trackedShims(sb: Sandbox): string {
  const review = join(sb.project, REVIEW);
  sb.write(JEV, '#!/bin/sh\nexec toolu jev "$@"\n');
  sb.write(REVIEW, '#!/bin/sh\nexec toolu review write-state "$@"\n');
  chmodSync(join(sb.project, JEV), 0o755);
  chmodSync(review, 0o755);
  sb.git("add", "-A");
  return review;
}

test.concurrent("the two exact native shims pass with real tracked files", () => {
  using sb = createSandbox({ git: true });
  trackedShims(sb);
  expect(shellProblems(sb.project)).toEqual([]);
});

test.concurrent("added shell logic in the review shim fails", () => {
  using sb = createSandbox({ git: true });
  const review = trackedShims(sb);
  appendFileSync(review, "printf 'extra'\n");
  expect(shellProblems(sb.project)).toContain(`native shim changed: ${REVIEW}`);
});

test.concurrent("a changed review shim body fails", () => {
  using sb = createSandbox({ git: true });
  const review = trackedShims(sb);
  writeFileSync(review, '#!/bin/sh\nexec toolu review write-state --quiet "$@"\n');
  expect(shellProblems(sb.project)).toContain(`native shim changed: ${REVIEW}`);
});

test.concurrent("a nonexecutable review shim fails", () => {
  using sb = createSandbox({ git: true });
  const review = trackedShims(sb);
  chmodSync(review, 0o644);
  expect(shellProblems(sb.project)).toContain(`native shim not executable: ${REVIEW}`);
});

test.concurrent("an unknown tracked shell file still fails", () => {
  using sb = createSandbox({ git: true });
  trackedShims(sb);
  sb.write("plugins/other/scripts/extra.sh", "#!/bin/sh\nexit 0\n");
  sb.git("add", "-A");
  expect(shellProblems(sb.project)).toContain("tracked shell file: plugins/other/scripts/extra.sh");
});
