import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { atomicWriteJson, fail, loadState, SlotLock, utcNow } from "./common";
import { GhError, ghRun } from "./gh";

type ReplyKind = "thread" | "conversation" | "review";
type ReplyOptions = {
  statePath: string;
  kind: ReplyKind;
  thread?: string;
  root?: string;
  inReplyTo?: string;
  commentId?: string;
  reviewId?: string;
  bodyPath: string;
  timeoutSeconds?: number;
};

function statePr(state: Record<string, unknown>): { repo: string; number: number; head: string } {
  const pr = state.pr as Record<string, unknown>;
  return { repo: String(state.repo), number: Number(state.number), head: String(pr.headSha ?? "") };
}

function ghFailure(error: unknown, source: string): never {
  if (error instanceof GhError) {
    fail(
      "api_error",
      `gh ${source} failed after ${error.attempts} attempt(s): ${error.lastMessage || `rc ${error.lastRc}`}`,
      {
        source,
        attempts: error.attempts,
        class: error.classification,
        lastMessage: error.lastMessage,
      },
    );
  }
  throw error;
}

function parseGhJson(text: string, source: string): Record<string, unknown> {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    fail("invalid_json", `${source}: response is not valid JSON`, { source });
  }
  if (value === null || typeof value !== "object" || Array.isArray(value))
    fail("invalid_json", `${source}: response is not an object`, { source });
  return value as Record<string, unknown>;
}

export async function replyThread(options: ReplyOptions): Promise<Record<string, unknown>> {
  const { statePath, kind, bodyPath } = options;
  const key =
    kind === "thread"
      ? `thread:${options.thread}@${options.inReplyTo}`
      : kind === "conversation"
        ? `conversation:${options.commentId}`
        : `review:${options.reviewId}`;
  const body = readFileSync(bodyPath, "utf8");
  const lock = new SlotLock(statePath);
  lock.acquire();
  try {
    const state = loadState(statePath);
    const actions = state.actions as { replied: Record<string, Record<string, unknown>> };
    const recorded = actions.replied[key];
    if (recorded !== undefined && recorded !== null)
      fail(
        "duplicate_reply",
        `reply-thread.sh: a reply to ${key} is already recorded; not posting again`,
        { key, recorded },
      );
    const { repo, number, head } = statePr(state);
    const endpoint =
      kind === "thread"
        ? `repos/${repo}/pulls/${number}/comments/${options.root}/replies`
        : `repos/${repo}/issues/${number}/comments`;
    const dir = mkdtempSync(join(tmpdir(), "pr-babysit-reply-"));
    let response: string;
    try {
      const payload = join(dir, "payload.json");
      writeFileSync(payload, JSON.stringify({ body }));
      response = await ghRun(
        ["api", "--method", "POST", endpoint, "--input", payload],
        options.timeoutSeconds === undefined ? {} : { timeoutSeconds: options.timeoutSeconds },
      );
    } catch (error) {
      ghFailure(error, "reply");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
    const reply = parseGhJson(response!, "reply");
    if (typeof reply.id !== "number")
      fail("invalid_json", "reply-thread.sh: reply response carried no comment id", {
        source: "reply",
      });
    actions.replied[key] = {
      commentId: reply.id,
      url: reply.html_url ?? null,
      at: utcNow(),
      headSha: head,
      kind,
    };
    atomicWriteJson(statePath, state);
    return { ok: true, key, commentId: reply.id, url: reply.html_url ?? null };
  } finally {
    lock.release();
  }
}

export async function resolveThread(options: {
  statePath: string;
  thread: string;
  timeoutSeconds?: number;
}): Promise<Record<string, unknown>> {
  const { statePath, thread } = options;
  const lock = new SlotLock(statePath);
  lock.acquire();
  try {
    const state = loadState(statePath);
    const actions = state.actions as { resolved: Record<string, Record<string, unknown>> };
    const prior = actions.resolved[thread] as Record<string, unknown> | undefined;
    if (prior?.confirmed === true)
      return {
        ok: true,
        thread,
        confirmed: true,
        attempts: 0,
        alreadyResolved: true,
        at: prior.at ?? null,
      };
    const mutation =
      "mutation($threadId:ID!){ resolveReviewThread(input:{threadId:$threadId}){ thread{ id isResolved } } }";
    let last: unknown = null;
    for (let attempt = 1; attempt <= 3; attempt += 1) {
      let response: string;
      try {
        response = await ghRun(
          ["api", "graphql", "-f", `threadId=${thread}`, "-f", `query=${mutation}`],
          options.timeoutSeconds === undefined ? {} : { timeoutSeconds: options.timeoutSeconds },
        );
      } catch (error) {
        ghFailure(error, "resolve");
      }
      const payload = parseGhJson(response!, "resolve");
      if (Array.isArray(payload.errors) && payload.errors.length > 0) {
        fail("invalid_json", "resolve-thread.sh: mutation response carried errors[]", {
          source: "resolve",
          errors: payload.errors,
        });
      }
      const data = payload.data as Record<string, unknown> | undefined;
      const resolved = data?.resolveReviewThread as Record<string, unknown> | undefined;
      last = resolved?.thread ?? null;
      if (
        last !== null &&
        typeof last === "object" &&
        (last as Record<string, unknown>).isResolved === true
      ) {
        const { head } = statePr(state);
        actions.resolved[thread] = {
          confirmed: true,
          at: utcNow(),
          attempts: attempt,
          headSha: head,
        };
        atomicWriteJson(statePath, state);
        return { ok: true, thread, confirmed: true, attempts: attempt };
      }
      process.stderr.write(
        `resolve-thread.sh: attempt ${attempt} returned isResolved=false for ${thread}; retrying\n`,
      );
      await Bun.sleep(1000);
    }
    fail(
      "resolve_unconfirmed",
      `resolve-thread.sh: ${thread} still unresolved after 3 attempt(s)`,
      { thread, attempts: 3, lastResponse: last },
    );
  } finally {
    lock.release();
  }
}
