// Print the rendered `.shell` of a page after its scripts ran, through the
// DevTools protocol of a headless Chrome. With a tab label, click the
// machine tab of that name first. Usage: bun <this> <chrome> <url> <ms> [tab]
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [chrome, url, wait, tab] = process.argv.slice(2);
if (!chrome || !url) {
  console.error("usage: epic-fleet-dashboard-dom.ts <chrome> <url> [ms]");
  process.exit(2);
}
const profile = mkdtempSync(join(tmpdir(), "fleet-chrome-"));
const port = 9300 + Math.floor(Math.random() * 500);
const browser = Bun.spawn(
  [chrome, "--headless=new", "--disable-gpu", "--no-first-run", `--user-data-dir=${profile}`,
    `--remote-debugging-port=${port}`, "about:blank"],
  { stdout: "ignore", stderr: "ignore" },
);
try {
  let target: { webSocketDebuggerUrl: string } | undefined;
  for (let attempt = 0; attempt < 100 && !target; attempt += 1) {
    try {
      const reply = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURI(url)}`, { method: "PUT" });
      target = await reply.json();
    } catch {
      await Bun.sleep(100);
    }
  }
  if (!target) throw new Error("Chrome DevTools did not start");
  await Bun.sleep(Number(wait ?? 3000));
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    socket.onopen = resolve;
    socket.onerror = () => reject(new Error("DevTools socket failed"));
  });
  let next = 0;
  const evaluate = (expression: string) => new Promise<string>((resolve) => {
    next += 1;
    const id = next;
    socket.addEventListener("message", (event) => {
      const message = JSON.parse(String(event.data));
      if (message.id === id) resolve(String(message.result?.result?.value ?? ""));
    });
    socket.send(JSON.stringify({ id, method: "Runtime.evaluate", params: { expression, returnByValue: true } }));
  });
  if (tab) {
    const click = `[...document.querySelectorAll('#machines button')].find(b => b.firstChild?.textContent === ${JSON.stringify(tab)})?.click()`;
    await evaluate(click);
    await Bun.sleep(2000);
  }
  console.log(await evaluate("document.querySelector('.shell').outerHTML"));
  socket.close();
} finally {
  browser.kill();
  await browser.exited;
  rmSync(profile, { recursive: true, force: true });
}
