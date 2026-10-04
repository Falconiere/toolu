import { fail, flag, parseFlags, runCli } from "./babysit/common.ts";
import { routeFix } from "./babysit/fixer-route.ts";

runCli(() => {
  const args = process.argv.slice(2);
  const raised: string[] = [];
  const withoutRaise: string[] = [];
  for (let i = 0; i < args.length; i += 1) {
    if (args[i] === "--raise") raised.push(args[++i] ?? "");
    else withoutRaise.push(args[i]!);
  }
  const flags = parseFlags(
    withoutRaise,
    "route-fix.js",
    ["--items", "--host", "--state-file", "--jev-answers-in", "--now"],
    ["--no-jev"],
  );
  const host = flag(flags, "--host");
  const itemsFile = flag(flags, "--items");
  if (!host) fail("usage", "route-fix.js: --host claude|codex|opencode required");
  if (!itemsFile) fail("usage", "route-fix.js: --items required");
  return routeFix({
    itemsFile,
    host,
    stateFile: flag(flags, "--state-file"),
    raise: raised,
    noJev: flags["--no-jev"] === true,
    answersFile: flag(flags, "--jev-answers-in"),
    now: flag(flags, "--now"),
  });
});
