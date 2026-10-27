import { agentState } from "./status.js";

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
const LIVE_STAGES = new Set(["running", "awaiting_merge"]);

export const ACTION_LABELS = { answer: "[ ANSWER ]", release: "[ RELEASE ]", ack: "[ ACK ]" };

// `answer` and `release` need a key; a keyless item can only be acknowledged by its kind.
export function attentionAction(kind, key) {
  if (!key) return "ack";
  if (ANSWER_KINDS.has(kind)) return "answer";
  return kind === "coupling-hold" ? "release" : "ack";
}

export const liveAgent = (issue, liveStatus) =>
  LIVE_STAGES.has(issue?.stage) && agentState(issue, liveStatus).kind !== "done";

export const controlDisabled = ({ locked, pending }) => Boolean(locked || pending);

export const sendDisabled = ({ locked, pending, text }) =>
  controlDisabled({ locked, pending }) || !String(text ?? "").trim();

// A coupling hold exists only before launch, so RELEASE HOLD and ACK never wait for an agent.
export function drawerActions({ issue, attention, locked, pending, liveStatus }) {
  const off = controlDisabled({ locked, pending });
  const actions = [
    { action: "answer", label: "ANSWER →", disabled: off || !liveAgent(issue, liveStatus) },
  ];
  if (issue?.hold) actions.push({ action: "release", label: "RELEASE HOLD", disabled: off });
  if (attention) actions.push({ action: "ack", label: "ACK", disabled: off });
  return actions;
}

// The op and JSON body a `data-action` button sends; `answer` opens the dialog instead.
export function actionBody(action, { epic, host, key, kind } = {}) {
  if (action === "pause" || action === "resume") return epic ? { epic } : {};
  if (action === "host-pause" || action === "host-resume") return { host };
  if (action === "ack") return key ? { key } : { kind };
  return { key };
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

const HOST_RECORDS = { "host-pause": "host-paused", "host-resume": "host-resumed" };

export function journalLine(op, body) {
  const target =
    op === "pause" || op === "resume"
      ? body.epic || "all"
      : (body.host ?? body.key ?? body.kind ?? "");
  return `${HOST_RECORDS[op] ?? op} ${target} by=web`;
}

// herdr refusals read `… herdr <method>: <code>: <message>`; other refusals are plain text.
export function refusalCode(error) {
  const text = String(error ?? "");
  return text.match(/herdr [^:\s]+: ([a-z][a-z0-9_]*):/)?.[1] ?? text;
}

export function actionResult(op, body, status, reply) {
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
  if (op !== "answer") {
    return {
      kind: "sent",
      label: `✓ SENT ${journalLine(op, body)}`,
      text: "The effect lands through the journal stream.",
    };
  }
  return reply?.delivered === "uncertain"
    ? { kind: "uncertain", label: "▲ UNCERTAIN", text: "Delivery uncertain; check the screen." }
    : { kind: "delivered", label: "✓ DELIVERED", text: "Attention marked answered." };
}

export const counter = (text) => `${String(text ?? "").length} / ${TEXT_LIMIT}`;
