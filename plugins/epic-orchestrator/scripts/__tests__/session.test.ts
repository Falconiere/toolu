import { expect, test } from "bun:test";
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
