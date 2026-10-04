import { expect, test } from "bun:test";
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { NPX_REFUSED_EXIT, docBlock, runScript, shQuote, writeShims } from "../doc-commands.ts";
import { ROOT } from "../scenarios-entry.ts";
import { ContractError } from "../schema.ts";

// The live docs.* scenarios run these blocks on the pinned host; here the
// extraction and the PATH shims run against the real docs and real shells (#363).

const INSTALL_DOC = readFileSync(join(ROOT, "docs/opencode.md"), "utf8");
const MIGRATION_DOC = readFileSync(join(ROOT, "docs/opencode-migration.md"), "utf8");

function marked(name: string, body: string): string {
  return `intro\n<!-- opencode-doc:${name}:start -->\n\n${body}\n<!-- opencode-doc:${name}:end -->\noutro\n`;
}

test.concurrent("the install and migration docs carry their executable blocks", () => {
  const quickstart = docBlock(INSTALL_DOC, "quickstart");
  expect(quickstart).toContain("npx @toolu/plugins install toolu context7 --host opencode");
  expect(quickstart).toContain("opencode debug skill | grep context7-context7");
  expect(quickstart).toContain("opencode run ");
  expect(docBlock(INSTALL_DOC, "manage")).toContain("npx @toolu/plugins remove toolu");
  expect(docBlock(MIGRATION_DOC, "migrate")).toContain("npx @toolu/plugins update --host opencode");
  expect(docBlock(MIGRATION_DOC, "rollback")).toContain("tar -xzf");
  expect(docBlock(MIGRATION_DOC, "migrate")).not.toContain("npx @toolu/plugins install --host");
});

test.concurrent("a block's markers must exist once and wrap exactly one bash fence", () => {
  const fence = "```bash\necho hi\n```";
  expect(docBlock(marked("one", fence), "one")).toBe("echo hi\n");
  const broken: Array<[string, string]> = [
    ["no markers", "```bash\necho hi\n```"],
    ["duplicate start", marked("one", fence) + marked("one", fence)],
    ["two fences", marked("one", `${fence}\n\n${fence}`)],
    ["prose between", marked("one", `run this:\n${fence}`)],
    ["not bash", marked("one", "```sh\necho hi\n```")],
    [
      "end before start",
      "<!-- opencode-doc:one:end -->\n```bash\necho hi\n```\n<!-- opencode-doc:one:start -->",
    ],
  ];
  for (const [label, text] of broken) {
    expect(() => docBlock(text, "one"), label).toThrow(ContractError);
    expect(() => docBlock(text, "one"), label).toThrow(/doc block one/);
  }
});

test.concurrent("the shims redirect only @toolu/plugins and opencode, and a failing step fails the block", async () => {
  using sb = createSandbox({});
  const log = sb.path("calls.log");
  const recorder = (tag: string): string => {
    const path = sb.path(`${tag}.sh`);
    writeFileSync(path, `#!/bin/sh\necho "${tag} $*" >> ${JSON.stringify(log)}\n`);
    chmodSync(path, 0o755);
    return path;
  };
  const shims = sb.path("bin");
  // `node <cli> args`: /bin/sh stands in for node, a recording script for the CLI bundle.
  writeShims(shims, { node: "/bin/sh", cli: recorder("cli"), opencode: recorder("opencode") });
  const opts = { cwd: sb.project, shims, env: {} };

  const ok = await runScript(
    "npx @toolu/plugins install toolu --host opencode\nopencode debug skill\n",
    opts,
  );
  expect(ok.exitCode).toBe(0);
  expect(readFileSync(log, "utf8")).toBe(
    "cli install toolu --host opencode\nopencode debug skill\n",
  );

  const refused = await runScript("npx cowsay hi\necho unreachable\n", opts);
  expect(refused.exitCode).toBe(NPX_REFUSED_EXIT);
  expect(refused.stderr).toContain("only @toolu/plugins is redirected, got: cowsay");
  expect(refused.stdout).not.toContain("unreachable");

  const piped = await runScript("false | cat\necho unreachable\n", opts);
  expect(piped.exitCode).not.toBe(0);
  expect(piped.stdout).not.toContain("unreachable");
});

test.concurrent("shim targets with shell metacharacters run verbatim", async () => {
  using sb = createSandbox({});
  // A directory whose name would expand or break inside double quotes.
  const odd = sb.path("we$HOME`id` 'dir");
  const log = sb.path("calls.log");
  const target = join(odd, "opencode.sh");
  mkdirSync(odd, { recursive: true });
  writeShims(sb.path("bin"), { node: "/usr/bin/env", cli: "true", opencode: target });
  writeFileSync(target, `#!/bin/sh\necho "opencode $*" >> ${shQuote(log)}\n`);
  chmodSync(target, 0o755);
  const res = await runScript("opencode --version\n", {
    cwd: sb.project,
    shims: sb.path("bin"),
    env: {},
  });
  expect(res.exitCode).toBe(0);
  expect(readFileSync(log, "utf8")).toBe("opencode --version\n");
  expect(shQuote("it's")).toBe(`'it'\\''s'`);
});

/** An `opencode` stand-in: the pinned version, a discovered leaf skill, and a run that fails. */
const FAILING_HOST = `#!/bin/sh
case "$1" in
  --version) echo 1.18.34 ;;
  debug) echo '[{"name":"context7-context7"}]' ;;
  run) echo "provider unreachable" >&2; exit 3 ;;
esac
`;

test.concurrent("the quick start removes its scratch file when a step fails, and never touches an existing one", async () => {
  using sb = createSandbox({});
  const host = sb.path("opencode.sh");
  writeFileSync(host, FAILING_HOST);
  chmodSync(host, 0o755);
  writeShims(sb.path("bin"), { node: "/usr/bin/env", cli: "true", opencode: host });
  const opts = { cwd: sb.project, shims: sb.path("bin"), env: {} };
  const quickstart = docBlock(INSTALL_DOC, "quickstart");

  const failed = await runScript(quickstart, opts);
  expect(failed.exitCode).toBe(3);
  expect(failed.stderr).toContain("provider unreachable");
  expect(existsSync(join(sb.project, ".env.toolu-check"))).toBe(false);

  writeFileSync(join(sb.project, ".env.toolu-check"), "MINE\n");
  const refused = await runScript(quickstart, opts);
  expect(refused.exitCode).toBe(1);
  expect(readFileSync(join(sb.project, ".env.toolu-check"), "utf8")).toBe("MINE\n");
});
