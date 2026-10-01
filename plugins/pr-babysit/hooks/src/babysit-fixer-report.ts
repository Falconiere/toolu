import { atomicWriteJson, fail, runCli, utcNow } from "./babysit/common.ts";

runCli(() => {
  const [report, status, ...rest] = process.argv.slice(2);
  if (!report) fail("usage", "fixer-report.js: <report-file> done|failed [--note <text>]");
  if (status !== "done" && status !== "failed")
    fail("usage", "fixer-report.js: status must be done or failed");
  if (rest.length && (rest.length !== 2 || rest[0] !== "--note"))
    fail("usage", `fixer-report.js: unknown argument: ${rest[0]}`);
  atomicWriteJson(report, { version: 1, status, note: rest.length ? rest[1] : null, at: utcNow() });
});
