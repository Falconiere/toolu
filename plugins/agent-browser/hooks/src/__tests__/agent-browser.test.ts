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
if (process.env.STUB_ECHO) process.stdout.write("out:" + (await Bun.stdin.text()));
if (process.env.STUB_SIGNAL) process.kill(process.pid, process.env.STUB_SIGNAL);
if (process.env.STUB_WAIT) {
  process.on("SIGTERM", () => {
    appendFileSync(process.env.AB_LOG, "got SIGTERM\\n");
    process.exit(7);
  });
  setInterval(() => {}, 1000);
} else {
  process.exit(Number(process.env.STUB_EXIT ?? 0));
}
`,
);
chmodSync(STUB, 0o755);

afterAll(() => rmSync(dir, { recursive: true, force: true }));
beforeEach(() => writeFileSync(LOG, ""));

/** Polls `done` every 20 ms for up to 5 s. */
async function until(done: () => boolean, deadline = Date.now() + 5000): Promise<void> {
  if (done() || Date.now() > deadline) return;
  await Bun.sleep(20);
  return until(done, deadline);
}

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

test("stdin and stdout pass straight through to and from the binary", async () => {
  const env = { ...process.env, AB_LOG: LOG, AGENT_BROWSER_BIN: STUB, STUB_ECHO: "1" };
  const child = Bun.spawn([BUNDLE, "eval"], { env, stdin: "pipe", stdout: "pipe" });
  void child.stdin.write("document.title");
  void child.stdin.end();
  const [stdout, status] = await Promise.all([new Response(child.stdout).text(), child.exited]);
  expect(status).toBe(0);
  expect(stdout).toBe("out:document.title");
});

test("an empty or bare-name AGENT_BROWSER_BIN resolves agent-browser on PATH", async () => {
  const path = `${dir}:${process.env["PATH"] ?? ""}`;
  const empty = await wrap(["click", "@e1"], { AGENT_BROWSER_BIN: "", PATH: path });
  expect(empty.status).toBe(0);
  expect(empty.argv).toBe("click @e1\n");
  writeFileSync(LOG, "");
  const bare = await wrap(["click", "@e2"], { AGENT_BROWSER_BIN: "agent-browser", PATH: path });
  expect(bare.status).toBe(0);
  expect(bare.argv).toBe("click @e2\n");
});

test("--raw still needs the binary", async () => {
  const run = await wrap(["--raw", "snapshot"], { AGENT_BROWSER_BIN: join(dir, "absent") });
  expect(run.status).toBe(127);
  expect(run.argv).toBe("");
});

test("SIGTERM to the wrapper reaches the binary, whose status the wrapper returns", async () => {
  const env = { ...process.env, AB_LOG: LOG, AGENT_BROWSER_BIN: STUB, STUB_WAIT: "1" };
  const child = Bun.spawn([BUNDLE, "open", "https://example.test"], { env, stdout: "pipe" });
  // Wait until the binary is up (it logs its argv first thing).
  await until(() => readFileSync(LOG, "utf8").includes("open"));
  child.kill("SIGTERM");
  expect(await child.exited).toBe(7);
  expect(readFileSync(LOG, "utf8")).toBe("open https://example.test\ngot SIGTERM\n");
});
