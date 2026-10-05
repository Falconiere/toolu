import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { captureSession, requireCapturedSession } from "../session.ts";

const FIRST = "0199a272-0b4e-7cc3-8c4f-071c81275655";
const SECOND = "0199a272-0b4e-7cc3-8c4f-071c81275656";

function codexSession(root: string, worktree: string, since: number, id: string): void {
  const day = new Date(since).toISOString().slice(0, 10).split("-");
  const sessions = join(root, "sessions", ...day);
  mkdirSync(sessions, { recursive: true });
  writeFileSync(
    join(sessions, `${id}.jsonl`),
    `${JSON.stringify({ type: "session_meta", payload: { id, cwd: worktree } })}\n`,
  );
}

function cursorChat(
  config: string,
  worktree: string,
  id: string,
  meta: Record<string, unknown> | null,
): void {
  const chat = join(config, "chats", createHash("md5").update(worktree).digest("hex"), id);
  mkdirSync(chat, { recursive: true });
  if (meta) writeFileSync(join(chat, "meta.json"), JSON.stringify(meta));
}

test.concurrent("captured Cursor chat metadata attests only the exact recorded session", async () => {
  using sb = createSandbox();
  const since = Date.now() - 1_000;
  const config = join(sb.root, "cursor");
  cursorChat(config, sb.project, FIRST, {
    schemaVersion: 1,
    createdAtMs: since + 300,
    hasConversation: false,
    cwd: sb.project,
  });
  const env = { CURSOR_CONFIG_DIR: config };

  expect(await captureSession("cursor", sb.project, since, env)).toBe(FIRST);
  await expect(
    requireCapturedSession("cursor", sb.project, since, FIRST, env),
  ).resolves.toBeUndefined();
  await expect(requireCapturedSession("cursor", sb.project, since, SECOND, env)).rejects.toThrow(
    "did not attach the recorded session",
  );
});

test.concurrent("Cursor capture skips older, foreign and unwritten chats", async () => {
  using sb = createSandbox();
  const since = Date.now() - 1_000;
  const config = join(sb.root, "xdg", "cursor");
  cursorChat(config, sb.project, FIRST, { createdAtMs: since + 10, cwd: sb.project });
  cursorChat(config, sb.project, SECOND, { createdAtMs: since - 60_000, cwd: sb.project });
  cursorChat(config, sb.project, "0199a272-0b4e-7cc3-8c4f-071c81275657", {
    createdAtMs: since + 10,
    cwd: join(sb.project, "other"),
  });
  cursorChat(config, sb.project, "0199a272-0b4e-7cc3-8c4f-071c81275658", null);
  cursorChat(config, sb.project, "not-a-chat-id", { createdAtMs: since + 10, cwd: sb.project });

  const env = { XDG_CONFIG_HOME: join(sb.root, "xdg") };
  expect(await captureSession("cursor", sb.project, since, env)).toBe(FIRST);
});

test.concurrent("ambiguous or absent Cursor chats never guess a session", async () => {
  using sb = createSandbox();
  const since = Date.now() - 1_000;
  const config = join(sb.root, "cursor");
  const env = { CURSOR_CONFIG_DIR: config };
  expect(await captureSession("cursor", sb.project, since, env)).toBeNull();

  cursorChat(config, sb.project, FIRST, { createdAtMs: since + 10, cwd: sb.project });
  cursorChat(config, sb.project, SECOND, { createdAtMs: since + 20, cwd: sb.project });
  expect(await captureSession("cursor", sb.project, since, env)).toBeNull();
});

test.concurrent("captured Codex metadata attests only the exact recorded session", async () => {
  using sb = createSandbox();
  const since = Date.now() - 1_000;
  const root = join(sb.root, "codex");
  codexSession(root, sb.project, since, FIRST);
  const env = { CODEX_HOME: root };

  expect(await captureSession("codex", sb.project, since, env)).toBe(FIRST);
  await expect(
    requireCapturedSession("codex", sb.project, since, FIRST, env),
  ).resolves.toBeUndefined();
  await expect(requireCapturedSession("codex", sb.project, since, SECOND, env)).rejects.toThrow(
    "did not attach the recorded session",
  );
});

test.concurrent("ambiguous Codex metadata never guesses a session", async () => {
  using sb = createSandbox();
  const since = Date.now() - 1_000;
  const root = join(sb.root, "codex");
  codexSession(root, sb.project, since, FIRST);
  codexSession(root, sb.project, since, SECOND);
  expect(await captureSession("codex", sb.project, since, { CODEX_HOME: root })).toBeNull();
});

test.concurrent("a session file that vanishes during discovery is skipped", async () => {
  using sb = createSandbox();
  const since = Date.now() - 1_000;
  const root = join(sb.root, "codex");
  codexSession(root, sb.project, since, FIRST);
  const day = new Date(since).toISOString().slice(0, 10).split("-");
  symlinkSync(join(root, "rotated.jsonl"), join(root, "sessions", ...day, "rotated.jsonl"));
  expect(await captureSession("codex", sb.project, since, { CODEX_HOME: root })).toBe(FIRST);
});
