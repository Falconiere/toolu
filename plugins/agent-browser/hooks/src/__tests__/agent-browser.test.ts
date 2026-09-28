/**
 * The committed agent-browser bundle, run by path as the published symlink runs
 * it. The external `agent-browser` binary is replaced by a real recording
 * executable that logs its argv; the wrapper itself runs for real.
 */
import { afterAll, beforeEach, expect, test } from "bun:test";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const BUNDLE = join(import.meta.dir, "../../dist/agent-browser.js");
const dir = mkdtempSync(join(tmpdir(), "agent-browser-"));
const LOG = join(dir, "argv.log");
const STUB = join(dir, "agent-browser");

writeFileSync(
  STUB,
  `#!${process.execPath}
import { appendFileSync } from "node:fs";
appendFileSync(process.env.AB_LOG, process.argv.slice(2).join(" ") + "\\n");
if (process.env.STUB_SIGNAL) process.kill(process.pid, process.env.STUB_SIGNAL);
process.exit(Number(process.env.STUB_EXIT ?? 0));
`,
);
chmodSync(STUB, 0o755);

afterAll(() => rmSync(dir, { recursive: true, force: true }));
beforeEach(() => writeFileSync(LOG, ""));

async function wrap(args: readonly string[], extra: Record<string, string> = {}) {
  const env = { ...process.env, AB_LOG: LOG, AGENT_BROWSER_BIN: STUB, ...extra };
  const child = Bun.spawn([BUNDLE, ...args], { env, stdout: "pipe", stderr: "pipe" });
  const [stderr, status] = await Promise.all([new Response(child.stderr).text(), child.exited]);
  return { status, stderr, argv: readFileSync(LOG, "utf8") };
}

const SHAPED: Array<[string[], string]> = [
  [["snapshot"], "snapshot -i --json --max-output 4000 --content-boundaries"],
  [["snapshot", "--max-output", "500"], "snapshot -i --json --content-boundaries --max-output 500"],
  [["snapshot", "-a"], "snapshot --json --max-output 4000 --content-boundaries -a"],
  [["snapshot", "-s", "#main"], "snapshot --json --max-output 4000 --content-boundaries -s #main"],
  [["snapshot", "--json=false"], "snapshot -i --max-output 4000 --content-boundaries --json=false"],
  [["get", "text", "@e1"], "get --json --max-output 4000 --content-boundaries text @e1"],
  [["find", "role", "button"], "find --json --max-output 4000 role button"],
  [["diff", "--max-output=10"], "diff --json --max-output=10"],
  [["screenshot", "page.png"], "screenshot page.png"],
  [["click", "@e2"], "click @e2"],
  [["wibble", "x"], "wibble x"],
  [["--raw", "snapshot"], "snapshot"],
  [[], ""],
];

for (const [args, expected] of SHAPED) {
  test(`${JSON.stringify(args)} runs the binary as: ${expected || "(no args)"}`, async () => {
    const run = await wrap(args);
    expect(run.status).toBe(0);
    expect(run.argv).toBe(`${expected}\n`);
  });
}

test("the binary's exit status is the wrapper's", async () => {
  const run = await wrap(["click", "@e9"], { STUB_EXIT: "3" });
  expect(run.status).toBe(3);
});

test("a binary killed by a signal exits 128 + the signal number", async () => {
  const run = await wrap(["snapshot"], { STUB_SIGNAL: "SIGTERM" });
  expect(run.status).toBe(143);
});

test("an absent binary prints the install guide and exits 127", async () => {
  const bins = [join(dir, "absent"), "agent-browser-not-on-path-anywhere"];
  const runs = await Promise.all(bins.map((bin) => wrap(["snapshot"], { AGENT_BROWSER_BIN: bin })));
  for (const run of runs) {
    expect(run.status).toBe(127);
    expect(run.stderr).toStartWith("agent-browser not found — install: npm i -g agent-browser");
    expect(run.argv).toBe("");
  }
});
