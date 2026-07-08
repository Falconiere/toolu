// Bun HTTP server for the multi-project dashboard. Serves the SPA, a JSON
// snapshot at /api/state, and a Server-Sent-Events stream at /api/events; both
// accept ?project=<id> to select a project. State is produced by a change-gated
// watcher over the configured roots, so an idle poll costs a walk + stats, never
// a transcript parse. Read-only: any non-GET is 405, no route mutates anything.

import { spawnSync } from "node:child_process";
import { homedir } from "node:os";
import { join, normalize, sep } from "node:path";

import { claudeCodeSource } from "./activity/claude-code.ts";
import type { LiveActivitySource } from "./activity/source.ts";
import { buildSessionDetail, type MultiDashboardState } from "./aggregate.ts";
import { type DashboardConfig, loadConfig } from "./config.ts";
import { createWatcher } from "./watch.ts";

const JSON_HEADERS = { "content-type": "application/json; charset=utf-8" } as const;

const ENCODER = new TextEncoder();
const HEARTBEAT_MS = 15_000;
const PUBLIC_DIR = join(import.meta.dir, "public");
const CONTENT_TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
};

/** SSE frame carrying a full state snapshot. */
function stateFrame(state: MultiDashboardState): Uint8Array {
  return ENCODER.encode(`event: state\ndata: ${JSON.stringify(state)}\n\n`);
}

/** Inject `config`/`source` in tests; production uses loadConfig() + the Claude Code source. */
export interface ServerOptions {
  config?: DashboardConfig;
  source?: LiveActivitySource;
}

/** Start the dashboard server. Returns its port and a stop() that tears everything down. */
export function startServer(opts: ServerOptions = {}): { port: number; stop: () => void } {
  let cfg = opts.config;
  if (!cfg) {
    const loaded = loadConfig();
    // A malformed config silently reverts roots to [] (empty board); the warning
    // is the only trace, so it must reach the operator.
    if (loaded.warning) console.error(`toolu dashboard: ${loaded.warning}`);
    cfg = loaded.config;
  }
  const source = opts.source ?? claudeCodeSource;
  const watcher = createWatcher(cfg, source);
  const clients = new Set<ReadableStreamDefaultController<Uint8Array>>();

  const empty = (): MultiDashboardState => ({
    projects: [],
    selected: null,
    serverTime: new Date().toISOString(),
  });

  /** Apply an optional selection and return fresh-or-current state. */
  const selectAndBuild = (projectId: string | null): MultiDashboardState => {
    if (projectId !== null) watcher.setSelected(projectId);
    return watcher.tick(Date.now()) ?? watcher.current() ?? empty();
  };

  const broadcast = (state: MultiDashboardState): void => {
    const frame = stateFrame(state);
    for (const c of clients) {
      try {
        c.enqueue(frame);
      } catch {
        clients.delete(c);
      }
    }
  };

  const serveStatic = async (name: string): Promise<Response> => {
    const full = normalize(join(PUBLIC_DIR, name));
    if (full !== PUBLIC_DIR && !full.startsWith(PUBLIC_DIR + sep)) {
      return new Response("forbidden", { status: 403 });
    }
    const file = Bun.file(full);
    if (!(await file.exists())) return new Response("not found", { status: 404 });
    const ext = name.slice(name.lastIndexOf("."));
    return new Response(file, {
      headers: { "content-type": CONTENT_TYPES[ext] ?? "application/octet-stream" },
    });
  };

  const openEventStream = (projectId: string | null): Response => {
    let heartbeat: ReturnType<typeof setInterval> | undefined;
    let self: ReadableStreamDefaultController<Uint8Array> | undefined;
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        self = controller;
        clients.add(controller);
        controller.enqueue(stateFrame(selectAndBuild(projectId)));
        heartbeat = setInterval(() => {
          try {
            controller.enqueue(ENCODER.encode(": heartbeat\n\n"));
          } catch {
            if (heartbeat) clearInterval(heartbeat);
            clients.delete(controller);
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
    const url = new URL(req.url);
    const path = url.pathname;
    const project = url.searchParams.get("project");
    if (path === "/" || path === "/index.html") return serveStatic("index.html");
    if (path.startsWith("/static/")) return serveStatic(path.slice("/static/".length));
    if (path === "/api/state") {
      return new Response(JSON.stringify(selectAndBuild(project)), { headers: JSON_HEADERS });
    }
    if (path === "/api/session") {
      const session = url.searchParams.get("session");
      if (!project || !session) {
        return new Response(JSON.stringify({ error: "project and session required" }), {
          status: 400,
          headers: JSON_HEADERS,
        });
      }
      // A malformed id (path-traversal attempt) makes the store throw; surface it
      // as a 400, never a 500, so a bad id reads as a client error, not a crash.
      try {
        const detail = buildSessionDetail(project, session, Date.now(), cfg.agentStuckSeconds);
        return new Response(JSON.stringify(detail), { headers: JSON_HEADERS });
      } catch (err) {
        return new Response(JSON.stringify({ error: `invalid project or session: ${String(err)}` }), {
          status: 400,
          headers: JSON_HEADERS,
        });
      }
    }
    if (path === "/api/events") return openEventStream(project);
    return new Response("not found", { status: 404 });
  };

  const server = Bun.serve({ port: cfg.port, fetch: handle, idleTimeout: 0 });
  const port = server.port;
  if (port === undefined) throw new Error("Bun.serve did not assign a port");

  // Advisory machine-global port file — a failed write must not stop the server.
  void Bun.write(join(homedir(), ".claude", "tmp", "dashboard.port"), String(port)).catch(() => {
    /* advisory only */
  });

  // Initial build, then a change-gated poll that only broadcasts on real changes.
  // Every layer under tick() is written to never throw (discovery, ledger reads,
  // git, transcript parsing all degrade to empty/null), but a throw escaping an
  // interval callback would kill the whole process — so the poll guards anyway
  // and keeps serving the last good state.
  const safeTick = (): MultiDashboardState | null => {
    try {
      return watcher.tick(Date.now());
    } catch (err) {
      console.error(`dashboard: tick failed — serving last state (${String(err)})`);
      return null;
    }
  };
  const first = safeTick();
  if (first) broadcast(first);
  const poll = setInterval(() => {
    const next = safeTick();
    if (next) broadcast(next);
  }, cfg.pollMs);

  return {
    port,
    stop: () => {
      clearInterval(poll);
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

// CLI entry: start, print the URL, optionally open the browser. Only when run directly.
if (import.meta.main) {
  const { port } = startServer();
  const url = `http://localhost:${port}`;
  console.log(url);
  if (process.argv.includes("--open") && process.platform === "darwin") {
    spawnSync("open", [url]);
  }
}
