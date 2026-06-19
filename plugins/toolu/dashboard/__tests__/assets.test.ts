// b3 check: the server delivers the public kanban assets. Starts the real server
// (pinned to a temp repo so it never touches the dev ledger) and asserts the HTML
// references app.js + styles.css and those static files load with sane types.

import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { DEFAULT_CONFIG } from "../config.ts";
import { startServer } from "../index.ts";

let repo: string;
let server: { port: number; stop: () => void };

beforeAll(() => {
  repo = mkdtempSync(join(tmpdir(), "dash-assets-"));
  // Asset delivery is independent of config; empty roots ⟹ no projects to scan.
  server = startServer({ config: { ...DEFAULT_CONFIG, roots: [], pollMs: 1000 } });
});

afterAll(() => {
  server.stop();
  rmSync(repo, { recursive: true, force: true });
});

describe("dashboard asset delivery", () => {
  test("GET / returns HTML referencing app.js and styles.css", async () => {
    const res = await fetch(`http://localhost:${server.port}/`);
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toContain("text/html");
    const html = await res.text();
    expect(html).toContain("/static/app.js");
    expect(html).toContain("/static/styles.css");
  });

  test("GET /static/app.js returns 200 with a javascript content-type", async () => {
    const res = await fetch(`http://localhost:${server.port}/static/app.js`);
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toContain("javascript");
    expect((await res.text()).length).toBeGreaterThan(0);
  });

  test("all React component modules are served as javascript", async () => {
    for (const mod of ["sidebar.js", "board.js", "tree.js", "view-model.js"]) {
      const res = await fetch(`http://localhost:${server.port}/static/${mod}`);
      expect(res.status).toBe(200);
      expect(res.headers.get("content-type")).toContain("javascript");
      expect((await res.text()).length).toBeGreaterThan(0);
    }
  });

  test("GET /static/styles.css returns 200 with a css content-type", async () => {
    const res = await fetch(`http://localhost:${server.port}/static/styles.css`);
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toContain("text/css");
    expect((await res.text()).length).toBeGreaterThan(0);
  });

  test("path-traversal via ....// is rejected, not collapsed into ../", async () => {
    // `..` is normalized away by URL before it reaches the server, so it never
    // exercises serveStatic. `....//` survives the wire, and a naive string-strip
    // collapses it to `../` (the bug). The server must 404 every climb and never
    // serve a file outside public/ (index.ts, one level up, contains "startServer").
    const sibling = await fetch(`http://localhost:${server.port}/static/....//index.ts`);
    expect(sibling.status).toBe(404);
    expect(await sibling.text()).not.toContain("startServer");

    const deep = await fetch(
      `http://localhost:${server.port}/static/${"....//".repeat(20)}etc/hosts`,
    );
    expect(deep.status).toBe(404);
  });
});
