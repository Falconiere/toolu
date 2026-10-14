// AC-8 evidence for #453 (plain JavaScript: the gate's reach check governs
// every tracked TypeScript file, and this one-off check is no product code): render the shipped dashboard in headless Chrome
// against real `toolu epic serve` engines and check the fleet view.
//
// Engine B binds 0.0.0.0 with a status token. Engine A lists B twice (right
// and wrong token) and a closed port; engine C has no peers. Chrome is driven
// through the DevTools protocol, so the checks read the rendered `.shell`
// (no script text). Saves A's rendered shell beside this file.
// Manual check: `bun docs/toolu/evidence/epic-fleet-dashboard.js`; needs
// Chrome (set CHROME) and target/debug/toolu (`cargo build -p toolu-cli`).
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { dirname, join } from "node:path";

const here = dirname(new URL(import.meta.url).pathname);
const toolu = join(here, "../../../target/debug/toolu");
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
for (const [path, hint] of [[toolu, "run cargo build -p toolu-cli"], [chrome, "set CHROME"]]) {
  if (!existsSync(path)) {
    console.error(`missing ${path}: ${hint}`);
    process.exit(2);
  }
}
const work = mkdtempSync("/tmp/fleet.");
const children = [];

function freePort() {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : 0;
      server.close(() => resolve(port));
    });
  });
}

function envOf(dir, extra = {}) {
  return {
    PATH: process.env.PATH ?? "/usr/bin:/bin",
    HOME: join(dir, "home"),
    TOOLU_RESOURCE_HOME: join(dir, "res"),
    TOOLU_CONFIG_DIR: join(dir, "cfg"),
    ...extra,
  };
}

/** A registered epic whose graph holds one issue. */
function seed(dir, epic, issue) {
  for (const sub of ["home", "res", "cfg", join(epic, "status")]) mkdirSync(join(dir, sub), { recursive: true });
  writeFileSync(join(dir, "res/registry.json"), JSON.stringify({ epics: [{ key: epic, state_dir: join(dir, epic) }] }));
  const graph = { issues: [{ key: issue, title: "Fleet evidence", wave: 1, open_blockers: [] }] };
  writeFileSync(join(dir, epic, "graph.json"), JSON.stringify(graph));
}

/** Start `toolu epic serve` and wait for its HTTP listener. */
async function engine(dir, bind, extra = {}) {
  const child = Bun.spawn([toolu, "epic", "serve", "--http", bind], {
    env: envOf(dir, extra), stdout: "ignore", stderr: "ignore",
  });
  children.push(child);
  const port = bind.split(":").at(-1);
  for (let attempt = 0; attempt < 200; attempt += 1) {
    try {
      await fetch(`http://127.0.0.1:${port}/healthz`);
      return;
    } catch {
      await Bun.sleep(50);
    }
  }
  throw new Error(`engine on ${bind} did not start`);
}

/** Record execution so the issue runs. */
async function report(dir, epic, issue) {
  const status = join(dir, epic, "status", `${issue}.json`);
  const child = Bun.spawn([toolu, "epic", "report", "--status-file", status, "execution", "--note", "evidence"], {
    env: envOf(dir), stdout: "ignore", stderr: "inherit",
  });
  if ((await child.exited) !== 0) throw new Error(`report ${issue} failed`);
}

/** The rendered page shell after 3 s, after clicking machine tab `tab` when given. */
async function render(port, tab) {
  const profile = mkdtempSync(join(work, "chrome-"));
  const debug = await freePort();
  const browser = Bun.spawn([chrome, "--headless=new", "--disable-gpu", "--no-first-run",
    `--user-data-dir=${profile}`, `--remote-debugging-port=${debug}`, "about:blank"], { stdout: "ignore", stderr: "ignore" });
  try {
    let target;
    for (let attempt = 0; attempt < 100 && !target; attempt += 1) {
      try {
        const reply = await fetch(`http://127.0.0.1:${debug}/json/new?http://127.0.0.1:${port}/`, { method: "PUT" });
        target = await reply.json();
      } catch {
        await Bun.sleep(100);
      }
    }
    if (!target) throw new Error("Chrome DevTools did not start");
    await Bun.sleep(3000);
    const socket = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((resolve, reject) => {
      socket.onopen = resolve;
      socket.onerror = () => reject(new Error("DevTools socket failed"));
    });
    let next = 0;
    const evaluate = (expression) => new Promise((resolve) => {
      next += 1;
      const id = next;
      socket.addEventListener("message", (event) => {
        const message = JSON.parse(String(event.data));
        if (message.id === id) resolve(String(message.result?.result?.value ?? ""));
      });
      socket.send(JSON.stringify({ id, method: "Runtime.evaluate", params: { expression, returnByValue: true } }));
    });
    if (tab) {
      await evaluate(`[...document.querySelectorAll('#machines button')].find(b => b.firstChild?.textContent === ${JSON.stringify(tab)})?.click()`);
      await Bun.sleep(2000);
    }
    const html = await evaluate("document.querySelector('.shell').outerHTML");
    socket.close();
    return html;
  } finally {
    browser.kill();
    await browser.exited;
  }
}

let failed = 0;
function check(text, needle, label) {
  if (text.includes(needle)) {
    console.log(`ok: ${label}`);
  } else {
    console.error(`FAIL: ${label}`);
    failed += 1;
  }
}

try {
  const [a, b, c] = ["a", "b", "c"].map((name) => join(work, name));
  const [pa, pb, pc, closed] = [await freePort(), await freePort(), await freePort(), await freePort()];
  seed(b, "repo-500", "repo-501");
  await engine(b, `0.0.0.0:${pb}`, { TOOLU_EPIC_STATUS_TOKEN: "evidence-right" });
  await report(b, "repo-500", "repo-501");
  seed(a, "repo-402", "repo-449");
  const peers = [
    { name: "server", url: `http://127.0.0.1:${pb}` },
    { name: "vault", url: `http://127.0.0.1:${pb}` },
    { name: "nas", url: `http://127.0.0.1:${closed}` },
  ];
  writeFileSync(join(a, "cfg/toolu.config.json"), JSON.stringify({ version: 1, epic: { peers } }));
  const tokens = JSON.stringify({ server: "evidence-right", vault: "evidence-wrong" });
  await engine(a, `127.0.0.1:${pa}`, { TOOLU_EPIC_PEER_TOKENS: tokens });
  await report(a, "repo-402", "repo-449");
  seed(c, "repo-600", "repo-601");
  await engine(c, `127.0.0.1:${pc}`);

  const page = await render(pa);
  writeFileSync(join(here, "epic-fleet-dashboard.html"), `${page}\n`);
  const peerIssue = await (await fetch(`http://127.0.0.1:${pa}/api/v1/fleet/peers/server/issues/repo-501`)).text();
  const server = await render(pa, "server");
  const vault = await render(pa, "vault");
  const lone = await render(pc);

  check(page, '<dl id="fleet-summary" class="summary">', "fleet summary shown");
  check(page, '<dd id="fleet-machines">2 / 4</dd>', "2 of 4 machines reachable");
  check(page, "This machine", "local tab");
  check(page, ">server<small>Live</small>", "server tab live");
  check(page, "Unauthorized: token rejected", "wrong-token peer");
  check(page, "Unreachable · never seen", "closed peer");
  check(page, "repo-449", "local issue on the board");
  check(peerIssue, '"key":"repo-501"', "peer issue through the proxy");
  check(server, '<h1 id="title">Epic status · server</h1>', "server view selected");
  check(server, "repo-501", "server board through the proxy");
  check(vault, "vault: Unauthorized: token rejected", "vault banner instead of panels");
  check(vault, '<div id="panels" hidden="">', "vault panels hidden");
  check(lone, '<div id="machines" class="tabs machines" role="tablist" aria-label="Machines" hidden="">', "no peers: switcher hidden");
  check(lone, '<dl id="fleet-summary" class="summary" hidden="">', "no peers: fleet summary hidden");
} finally {
  for (const child of children) child.kill();
  await Promise.all(children.map((child) => child.exited));
  rmSync(work, { recursive: true, force: true });
}
process.exit(failed === 0 ? 0 : 1);
