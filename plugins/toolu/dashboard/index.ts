// Read-only realtime kanban dashboard over the plan-ledger. Bun.serve on an
// ephemeral port; serves static assets, exposes ledger state as JSON, and pushes
// updates over SSE. All serving/wiring lives here; ledger reading + derivation
// lives in state.ts and path resolution in resolve.ts (keeps this file < 300).
// Every server instance owns its own client hub + paths (no module globals) so
// concurrent instances — e.g. parallel tests — never clobber each other.

import { spawnSync } from "node:child_process";
import { statSync, watch, type FSWatcher } from "node:fs";
import { dirname, basename, join, extname, resolve, sep } from "node:path";

import { buildState, type DashboardState } from "./state.ts";
import { resolveLedgerPath, resolveRepoRoot } from "./resolve.ts";

const PUBLIC_DIR = join(import.meta.dir, "public");
const HEARTBEAT_MS = 15_000;
const POLL_MS = 1_000;
const DEBOUNCE_MS = 150;
const ENCODER = new TextEncoder();

const CONTENT_TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
};

/** Optional pinned paths for tests (inject a temp ledger/repo); when unset the
 *  engine is shelled on every resolve so a real branch switch is picked up. */
export interface Pinned {
  ledgerPath?: string;
  repoRoot?: string;
}

/** Serialize a payload as one SSE `event: state` frame. */
function stateFrame(state: DashboardState): Uint8Array {
  return ENCODER.encode(`event: state\ndata: ${JSON.stringify(state)}\n\n`);
}

/** Serve a file from public/ with the right content-type, or 404. Resolves the
 *  request path and rejects anything that escapes public/ — never rewrites the
 *  path (string-stripping `..` is bypassable, e.g. `....//` -> `../`). */
async function serveStatic(relPath: string): Promise<Response> {
  const resolved = resolve(join(PUBLIC_DIR, relPath));
  if (resolved !== PUBLIC_DIR && !resolved.startsWith(PUBLIC_DIR + sep)) {
    return new Response("not found", { status: 404 });
  }
  const file = Bun.file(resolved);
  if (!(await file.exists())) return new Response("not found", { status: 404 });
  const type = CONTENT_TYPES[extname(resolved).toLowerCase()] ?? "application/octet-stream";
  return new Response(file, { headers: { "content-type": type } });
}

/** Trailing-edge debounce: only the last call within the window runs. */
function debounce(fn: () => void, ms: number): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  return () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(fn, ms);
  };
}

/** Start the dashboard server on an ephemeral port. Tests may pin a temp
 *  ledger/repo and must call stop() in afterAll; the CLI entry below uses it. */
export function startServer(opts: Pinned = {}): { port: number; stop: () => void } {
  // Per-instance client hub — a disconnected client is dropped so broadcast never throws.
  const clients = new Set<ReadableStreamDefaultController<Uint8Array>>();

  // Re-resolve each call so a branch switch is picked up; pinned (tests) wins.
  const ledgerPathNow = (): string | null => opts.ledgerPath ?? resolveLedgerPath();
  const repoRootNow = (): string | null => opts.repoRoot ?? resolveRepoRoot();

  const currentState = (): DashboardState => {
    const ledgerPath = ledgerPathNow();
    const repoRoot = repoRootNow();
    if (!ledgerPath || !repoRoot) {
      return { ledger: null, currentDiffSha: null, steps: [], serverTime: new Date().toISOString() };
    }
    return buildState({ ledgerPath, repoRoot });
  };

  const broadcast = (): void => {
    const frame = stateFrame(currentState());
    for (const controller of clients) {
      try {
        controller.enqueue(frame);
      } catch {
        clients.delete(controller);
      }
    }
  };

  // SSE: emit current state immediately, then heartbeat; cancel drops the client.
  const openEventStream = (): Response => {
    let heartbeat: ReturnType<typeof setInterval> | undefined;
    let self: ReadableStreamDefaultController<Uint8Array> | undefined;
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        self = controller;
        clients.add(controller);
        controller.enqueue(stateFrame(currentState()));
        heartbeat = setInterval(() => {
          try {
            controller.enqueue(ENCODER.encode(": heartbeat\n\n"));
          } catch {
            clients.delete(controller);
            if (heartbeat) clearInterval(heartbeat);
          }
        }, HEARTBEAT_MS);
      },
      cancel() {
        if (heartbeat) clearInterval(heartbeat);
        if (self) clients.delete(self);
      },
    });
    return new Response(stream, {
      headers: {
        "content-type": "text/event-stream; charset=utf-8",
        "cache-control": "no-cache",
        connection: "keep-alive",
      },
    });
  };

  // Route a single GET request. POST/etc. -> 405 (read-only by contract).
  const handle = async (req: Request): Promise<Response> => {
    if (req.method !== "GET") return new Response("method not allowed", { status: 405 });
    const path = new URL(req.url).pathname;
    if (path === "/" || path === "/index.html") return serveStatic("index.html");
    if (path.startsWith("/static/")) return serveStatic(path.slice("/static/".length));
    if (path === "/api/state") {
      return new Response(JSON.stringify(currentState()), {
        headers: { "content-type": "application/json; charset=utf-8" },
      });
    }
    if (path === "/api/events") return openEventStream();
    return new Response("not found", { status: 404 });
  };

  // Watch the ledger's PARENT dir (the writer renames via mv, so a file-path
  // watch misses updates), filter by basename, re-arm on each event, and run a
  // stat-mtime poll fallback that also re-resolves the path each tick.
  const fire = debounce(broadcast, DEBOUNCE_MS);
  let watcher: FSWatcher | undefined;
  let watchedDir = "";
  let lastMtime = 0;

  const arm = (ledgerPath: string): void => {
    const dir = dirname(ledgerPath);
    const base = basename(ledgerPath);
    if (watcher && watchedDir === dir) return;
    if (watcher) watcher.close();
    watchedDir = dir;
    try {
      watcher = watch(dir, (_event, filename) => {
        if (!filename || basename(filename.toString()) === base) {
          arm(ledgerPath);
          fire();
        }
      });
      watcher.on("error", () => {
        if (watcher) watcher.close();
        watcher = undefined;
        watchedDir = "";
      });
    } catch {
      watcher = undefined;
      watchedDir = "";
    }
  };

  const tick = (): void => {
    const ledgerPath = ledgerPathNow();
    if (!ledgerPath) return;
    arm(ledgerPath);
    try {
      const mtime = statSync(ledgerPath).mtimeMs;
      if (mtime !== lastMtime) {
        lastMtime = mtime;
        fire();
      }
    } catch {
      // Ledger absent yet — nothing to compare; the watcher catches creation.
    }
  };

  const server = Bun.serve({ port: 0, fetch: handle, idleTimeout: 0 });
  const port = server.port;
  if (port === undefined) throw new Error("Bun.serve did not assign a port");

  // Advisory port file — a failed write must not stop the server. Bun.write is
  // async, so swallow the rejection on the Promise (a sync try/catch can't).
  const root = repoRootNow();
  if (root) {
    void Bun.write(join(root, ".claude", "tmp", "dashboard.port"), String(port)).catch(
      () => {
        /* advisory only */
      },
    );
  }

  tick();
  const poll = setInterval(tick, POLL_MS);

  return {
    port,
    stop: () => {
      clearInterval(poll);
      if (watcher) watcher.close();
      for (const c of clients) {
        try {
          c.close();
        } catch {
          /* already closed */
        }
      }
      clients.clear();
      server.stop(true);
    },
  };
}

// CLI entry: start, print the URL, optionally open the browser. Only runs when
// invoked directly (not when imported by a test).
if (import.meta.main) {
  const { port } = startServer();
  const url = `http://localhost:${port}`;
  console.log(url);
  if (process.argv.includes("--open") && process.platform === "darwin") {
    spawnSync("open", [url]);
  }
}
