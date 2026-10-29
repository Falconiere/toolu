import { DONE_STAGES } from "./status.js";

export const TOKEN_KEY = "toolu.token";
export const TEXT_LIMIT = 4000;

export const LOCKED_BANNER = {
  label: "○ CONTROL LOCKED",
  text: "Watching only. Open the token URL the engine printed to pause, resume, answer or release. Treat that URL like a password.",
};
export const ROTATED_BANNER = {
  label: "● 401 · TOKEN ROTATED",
  text: "Control is locked. Nothing was paused. Open the new token URL to unlock.",
};
export const WORKING_WARNING =
  "▲ The agent is working. The host may queue this or apply it mid-turn.";

// Engine attention kinds a worker asks a question with; `judgment` is a journal kind, not one.
const ANSWER_KINDS = new Set(["needs-human", "stuck", "help", "contract-changed"]);

export const ACTION_LABELS = { answer: "[ ANSWER ]", release: "[ RELEASE ]", ack: "[ ACK ]" };

// Acknowledged and answered items stay listed until they clear; only open ones take an action.
export const attentionOpen = (item) => Boolean(item) && (!item.state || item.state === "open");

// A key can hold several items, some already handled; its open one is the one to act on.
export function keyAttention(attention, key) {
  const items = (attention?.items ?? []).filter((item) => item.key === key);
  return items.find(attentionOpen) ?? items[0];
}

// `answer` and `release` need a key; a keyless item can only be acknowledged by its kind.
export function attentionAction(kind, key, state = "open") {
  if (state !== "open") return null;
  if (!key) return "ack";
  if (ANSWER_KINDS.has(kind)) return "answer";
  return kind === "coupling-hold" ? "release" : "ack";
}

// The engine's `has_live_agent`: launched and not finished. An agent status of `done` only
// ends a turn (Codex reports it after every turn), so it still takes an answer.
export const liveAgent = (issue) => Boolean(issue?.stage) && !DONE_STAGES.includes(issue.stage);

export const controlDisabled = ({ locked, pending }) => Boolean(locked || pending);

export const sendDisabled = ({ locked, pending, text }) =>
  controlDisabled({ locked, pending }) || !String(text ?? "").trim();

// A coupling hold exists only before launch, so RELEASE HOLD and ACK never wait for an agent.
export function drawerActions({ issue, attention, locked, pending }) {
  const off = controlDisabled({ locked, pending });
  const actions = [{ action: "answer", label: "ANSWER →", disabled: off || !liveAgent(issue) }];
  if (issue?.hold) actions.push({ action: "release", label: "RELEASE HOLD", disabled: off });
  if (attentionOpen(attention)) actions.push({ action: "ack", label: "ACK", disabled: off });
  return actions;
}

// One intentional answer click: a fresh id the engine dedupes retries against.
export function mintSubmissionId() {
  if (globalThis.crypto?.randomUUID) {
    return globalThis.crypto.randomUUID();
  }
  return `s${Date.now().toString(36)}${Math.random().toString(36).slice(2, 10)}`;
}

// The op and JSON body a `data-action` button sends; `answer` opens the dialog instead.
export function actionBody(action, { epic, host, key, kind } = {}) {
  if (action === "pause" || action === "resume") return epic ? { epic } : {};
  if (action === "host-pause" || action === "host-resume") return { host };
  if (action === "ack") return key ? { key } : { kind };
  return { key };
}

// The JSON body `answer` POSTs: key, text, and a stable submission id for this click.
export function answerBody(key, text, submissionId = mintSubmissionId()) {
  return { key, text, submission_id: submissionId };
}

// The token rides only in the Authorization header, never in the path.
export function actionRequest(op, body, token) {
  return {
    path: `/api/v1/actions/${encodeURIComponent(op)}`,
    init: {
      method: "POST",
      cache: "no-store",
      headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
      body: JSON.stringify(body),
    },
  };
}

// The record names the engine journals: `ack` writes `acknowledged`, host ops their past tense.
const RECORDS = { ack: "acknowledged", "host-pause": "host-paused", "host-resume": "host-resumed" };

// `partner` is the hold's other issue, which the engine's `release` note names before `by=`.
export function journalLine(op, body, partner = "") {
  const target =
    op === "pause" || op === "resume"
      ? body.epic || "all"
      : (body.host ?? body.key ?? body.kind ?? "");
  const note = op === "release" && partner ? `${partner} by=web` : "by=web";
  return `${RECORDS[op] ?? op} ${target} ${note}`;
}

// herdr refusals read `… herdr <method>: <code>: <message>`; other refusals are plain text.
export function refusalCode(error) {
  const text = String(error ?? "");
  return text.match(/herdr [^:\s]+: ([a-z][a-z0-9_]*):/)?.[1] ?? text;
}

export function actionResult(op, body, status, reply, partner = "") {
  if (status === 409) {
    const error = String(reply?.error ?? "");
    const text = op === "answer" ? "The engine refused the answer. Attach from a terminal." : error;
    return { kind: "refused", label: `● REFUSED · ${refusalCode(error)}`, text, detail: error };
  }
  if (status !== 200) {
    return {
      kind: "failed",
      label: `● FAILED · ${reply?.error ?? (status ? `HTTP ${status}` : "network")}`,
      text: "Nothing changed on the page; the action may still apply (see the journal).",
    };
  }
  // Post-admit HTTP timeouts use the same uncertain shape for every op.
  if (reply?.delivered === "uncertain") {
    return { kind: "uncertain", label: "▲ UNCERTAIN", text: "Delivery uncertain; check the screen." };
  }
  if (op !== "answer") {
    return {
      kind: "sent",
      label: `✓ SENT ${journalLine(op, body, partner)}`,
      text: "The effect lands through the journal stream.",
    };
  }
  return { kind: "delivered", label: "✓ DELIVERED", text: "Attention marked answered." };
}

export const counter = (text) => `${String(text ?? "").length} / ${TEXT_LIMIT}`;
