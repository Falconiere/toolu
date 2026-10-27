import { DONE_STAGES } from "./status.js";

const STEPS = {
  green: { kind: "green", label: "✓ GREEN" },
  running: { kind: "running", label: "● IN PROGRESS" },
  pending: { kind: "pending", label: "○ TODO" },
  red: { kind: "red", label: "✕ FAILING" },
  stale: { kind: "stale", label: "↺ STALE" },
};

const NOTICES = {
  peer: {
    kind: "peer",
    label: "◇ OPEN DASHBOARD TO WATCH OR CONTROL",
    text: "Screens, live status and actions stay on the peer. This view shows the plan only.",
  },
  locked: {
    kind: "locked",
    label: "○ CONTROL LOCKED",
    text: "Screen reads need the token. Open the token URL the engine printed.",
  },
  closed: {
    kind: "stopped",
    label: "/ AGENT NOT RUNNING",
    text: "The agent pane is closed. Plans and phases still update.",
  },
  unavailable: {
    kind: "unavailable",
    label: "? LIVE VIEW UNAVAILABLE",
    text: "herdr not reachable. Plans and phases still update.",
  },
};

export const PLAN_UNREADABLE = {
  kind: "unreadable",
  label: "● PLAN UNREADABLE",
  text: "The ledger failed to parse. Phase history shown instead.",
};
export const PLAN_UNAVAILABLE =
  "Plan unavailable. The worker document could not be read; retrying.";
export const NO_PLAN = "No plan yet. Phase history below until the worker writes its ledger.";

export const stepMark = (status) =>
  Object.hasOwn(STEPS, status) ? STEPS[status] : { kind: "unknown", label: "? UNKNOWN" };

export const planLine = (plan) => `${plan?.summary?.green ?? 0}/${plan?.summary?.total ?? 0} GREEN`;

export function nextStep(plan) {
  if (!plan?.next) return null;
  return (plan.steps ?? []).find((step) => step.id === plan.next)?.title || plan.next;
}

const blocker = (ref) => (ref.includes("#") ? ref.slice(ref.lastIndexOf("#")) : ref);

function stoppedNotice(issue) {
  if (!issue?.stage) {
    const blockers = (issue?.open_blockers ?? []).map(blocker);
    const wait = blockers.length ? `Queued until ${blockers.join(", ")} merges.` : "Queued.";
    return { ...NOTICES.closed, text: `${wait} The screen appears once a host claims it.` };
  }
  if (DONE_STAGES.includes(issue.stage)) {
    const phase = (issue.phase || issue.stage).toUpperCase();
    return { ...NOTICES.closed, text: `Finished · last phase ${phase}.` };
  }
  return null;
}

// `screen` is `{status, body}` from the screen route, `{error}` on a failed request, or null.
export function screenNotice({ peer, issue, token, screen }) {
  if (peer) return NOTICES.peer;
  const stopped = stoppedNotice(issue);
  if (stopped) return stopped;
  if (!token || [401, 403].includes(screen?.status)) return NOTICES.locked;
  if (!screen) return null;
  if (screen.body?.state === "not_running") return NOTICES.closed;
  if (screen.error || screen.status !== 200 || screen.body?.available === false) {
    return NOTICES.unavailable;
  }
  return null;
}

export const wantsScreen = (context) =>
  !context.peer && !stoppedNotice(context.issue) && Boolean(context.token);

export function attachCommand(screen) {
  const body = screen?.body;
  return body?.agent_status === "blocked" && typeof body.attach === "string" && body.attach.trim()
    ? body.attach
    : null;
}

const pad = (value) => String(value).padStart(2, "0");

export function clock(iso, seconds = false) {
  const at = new Date(iso ?? Number.NaN);
  if (Number.isNaN(at.getTime())) return "—";
  const parts = [at.getHours(), at.getMinutes()].concat(seconds ? [at.getSeconds()] : []);
  return parts.map(pad).join(":");
}

export function phaseTimeline(history, issue) {
  const rows = (history ?? []).map((row) => ({
    phase: (row.phase || "—").toUpperCase(),
    at: clock(row.at),
  }));
  if (!rows.length && !issue?.stage) rows.push({ phase: "QUEUED", at: "—" });
  return rows.map((row, index) => ({ ...row, current: index === rows.length - 1 }));
}

// shortcut: the board names only the epic's repository, so a PR in another repository links there; pass the issue's repository once the API serves it.
export function prUrl(issueUrl, pr) {
  if (!Number.isInteger(pr) || pr <= 0 || !issueUrl) return null;
  try {
    const url = new URL(issueUrl);
    const repo = url.pathname.match(/^(\/[^/]+\/[^/]+)\/issues\/\d+\/?$/);
    return ["http:", "https:"].includes(url.protocol) && repo
      ? `${url.origin}${repo[1]}/pull/${pr}`
      : null;
  } catch {
    return null;
  }
}

export function attentionLine(item, now = Date.now(), sampledAt = now) {
  const age = Math.max(0, (item.age_ms ?? 0) + now - sampledAt);
  return `▲ ${(item.kind || "unknown").toUpperCase()} · OPEN ${Math.floor(age / 60_000)}m`;
}

export const epicLine = (key, epic) =>
  `${key} · EPIC #${String(epic ?? "")
    .split("-")
    .at(-1)}`;
