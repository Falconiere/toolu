import { fail, flag, parseFlags, runCli } from "./babysit/common.ts";
import { dispatchFix } from "./babysit/fixer-dispatch.ts";

runCli(() => {
  const [sub, ...rest] = process.argv.slice(2);
  if (sub !== "start" && sub !== "wait" && sub !== "cleanup")
    fail("usage", "dispatch-fix.js: subcommand must be start, wait or cleanup");
  const flags = parseFlags(
    rest,
    "dispatch-fix.js",
    ["--state-file", "--plan", "--items", "--repo-root", "--branch", "--base", "--timeout-seconds"],
    ["--dry-run"],
  );
  const timeoutText = flag(flags, "--timeout-seconds");
  if (timeoutText && !/^\d+$/.test(timeoutText))
    fail("usage", "dispatch-fix.js: --timeout-seconds must be a whole number");
  return dispatchFix({
    sub,
    stateFile: flag(flags, "--state-file"),
    planFile: flag(flags, "--plan"),
    itemsFile: flag(flags, "--items"),
    repoRoot: flag(flags, "--repo-root"),
    branch: flag(flags, "--branch"),
    base: flag(flags, "--base"),
    ...(timeoutText ? { timeoutSeconds: Number(timeoutText) } : {}),
    dryRun: flags["--dry-run"] === true,
  });
});
