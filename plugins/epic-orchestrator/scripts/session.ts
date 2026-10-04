/** Capture host-native session IDs; never substitute the host's unrelated last session. */
import { existsSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { runCommand } from "../hooks/dist/epic-runtime.js";
import type { HostKind } from "./hosts.ts";

const MAX_SESSION_FILES = 256;
const MAX_SESSION_BYTES = 65_536;
const MAX_SESSION_RECORDS = 32;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function vanished(error: unknown): boolean {
  return error instanceof Error && "code" in error && error.code === "ENOENT";
}

function validSessionId(kind: HostKind, value: unknown): value is string {
  if (typeof value !== "string") return false;
  if (kind === "opencode") return /^ses_[A-Za-z0-9]+$/.test(value);
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

function utcDayParts(epochMs: number): string[] {
  return new Date(epochMs).toISOString().slice(0, 10).split("-");
}

/** Start readiness can precede native session persistence (observed with Codex). */
export async function awaitSession(
  kind: HostKind,
  worktree: string,
  since: number,
): Promise<string> {
  const deadline = Date.now() + 10_000;
  const poll = async (): Promise<string> => {
    const id = await captureSession(kind, worktree, since);
    if (id) return id;
    if (Date.now() >= deadline || kind === "cursor")
      throw new Error(
        `cannot attest ${kind} session identity; inspect the owned pane before retry`,
      );
    return Bun.sleep(500).then(poll);
  };
  return poll();
}

export async function captureSession(
  kind: HostKind,
  worktree: string,
  since: number,
  env: NodeJS.ProcessEnv = process.env,
): Promise<string | null> {
  if (kind === "cursor") return null; // Installed CLI currently cannot attest session storage.
  if (kind === "opencode") {
    const result = await runCommand(["opencode", "session", "list", "--format", "json"], {
      cwd: worktree,
      timeoutMs: 10_000,
    });
    if (result.exitCode !== 0 || result.timedOut || result.truncated) return null;
    let data: unknown;
    try {
      data = JSON.parse(result.stdout);
    } catch {
      return null;
    }
    if (!Array.isArray(data)) return null;
    const matches = data.filter(
      (entry): entry is { directory: string; id: string; created: number } =>
        isRecord(entry) &&
        entry.directory === worktree &&
        validSessionId(kind, entry.id) &&
        typeof entry.created === "number" &&
        Number.isFinite(entry.created) &&
        entry.created >= since,
    );
    // More than one candidate requires explicit reconciliation, never guessing.
    return matches.length === 1 ? (matches[0] as { id: string }).id : null;
  }
  const roots =
    kind === "codex"
      ? [since, Date.now()].map((epochMs) =>
          join(env.CODEX_HOME || join(homedir(), ".codex"), "sessions", ...utcDayParts(epochMs)),
        )
      : [
          join(
            env.CLAUDE_CONFIG_DIR || join(homedir(), ".claude"),
            "projects",
            worktree.replace(/[^a-zA-Z0-9]/g, "-"),
          ),
        ];
  const matches = new Set<string>();
  let scanned = 0;
  for (const root of new Set(roots)) {
    if (!existsSync(root)) continue;
    try {
      for await (const path of new Bun.Glob("**/*.jsonl").scan({
        cwd: root,
        absolute: true,
      })) {
        if (++scanned > MAX_SESSION_FILES) return null;
        let records: string[];
        try {
          if (statSync(path).mtimeMs < since) continue;
          records = (await Bun.file(path).slice(0, MAX_SESSION_BYTES).text())
            .split("\n")
            .slice(0, MAX_SESSION_RECORDS);
        } catch (error) {
          if (vanished(error)) continue; // A host may rotate a session file during discovery.
          throw error;
        }
        for (const line of records) {
          if (!line) continue;
          try {
            const entry: unknown = JSON.parse(line);
            if (!isRecord(entry)) continue;
            if (kind === "codex") {
              const payload = entry.payload;
              if (
                entry.type === "session_meta" &&
                isRecord(payload) &&
                payload.cwd === worktree &&
                validSessionId(kind, payload.id)
              )
                matches.add(payload.id);
            }
            if (
              kind === "claude" &&
              entry.cwd === worktree &&
              validSessionId(kind, entry.sessionId)
            )
              matches.add(entry.sessionId);
          } catch (error) {
            if (!(error instanceof SyntaxError)) throw error;
          }
        }
      }
    } catch (error) {
      // A host may remove a session directory while it is scanned.
      if (!vanished(error)) throw error;
    }
  }
  return matches.size === 1 ? ([...matches][0] ?? null) : null;
}

/** Refuse session reuse unless durable host metadata names the exact conversation. */
export async function requireCapturedSession(
  kind: HostKind,
  worktree: string,
  since: number,
  expected: string,
  env: NodeJS.ProcessEnv = process.env,
): Promise<void> {
  const actual = await captureSession(kind, worktree, since, env);
  if (actual !== expected) throw new Error("started agent did not attach the recorded session");
}
