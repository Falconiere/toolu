const AGENTS = {
  working: { kind: "working", label: "● WORKING" },
  blocked: { kind: "blocked", label: "▲ BLOCKED" },
  idle: { kind: "idle", label: "/ IDLE" },
  done: { kind: "done", label: "✓ DONE" },
  unknown: { kind: "unknown", label: "? UNKNOWN" },
  queued: { kind: "queued", label: "○ QUEUED" },
};

const HOSTS = {
  rotation: { kind: "rotation", label: "● IN ROTATION" },
  cooling: { kind: "cooling", label: "◷ COOLING" },
  paused: { kind: "paused", label: "‖ PAUSED" },
  pausedCooling: { kind: "paused", label: "‖ PAUSED · COOLING" },
};

export const DONE_STAGES = ["merged", "cleaning", "cleanup-incomplete", "abandoned"];
export const LIVE_STAGES = ["running", "awaiting_merge"];

const pad = (value) => String(value).padStart(2, "0");

export function clock(iso, seconds = false) {
  const at = new Date(iso ?? Number.NaN);
  if (Number.isNaN(at.getTime())) return "—";
  const parts = [at.getHours(), at.getMinutes()].concat(seconds ? [at.getSeconds()] : []);
  return parts.map(pad).join(":");
}

const count = (value) => (Number.isFinite(value) && value > 0 ? value : 0);

export function agentState(issue, fallback) {
  if (DONE_STAGES.includes(issue?.stage) || issue?.closed) {
    return AGENTS.done;
  }
  if (!issue?.stage) return AGENTS.queued;
  const status = issue.agent_status === "unknown" ? fallback : issue.agent_status;
  return AGENTS[status] ?? AGENTS.unknown;
}

export function cooldown(until, now = Date.now()) {
  if (!until) return null;
  const seconds = Math.ceil((Date.parse(until) - now) / 1000);
  if (!Number.isFinite(seconds) || seconds <= 0) return null;
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

const hostPaused = (host) => Boolean(host.paused || host.state === "paused");

export function hostState(host, now = Date.now()) {
  const cooling = Boolean(cooldown(host.cooldown_until, now));
  if (hostPaused(host)) return cooling ? HOSTS.pausedCooling : HOSTS.paused;
  return cooling ? HOSTS.cooling : HOSTS.rotation;
}

// The engine signs CLI pauses `user`.
const pauser = (by) => (by === "user" ? "cli" : by || "unknown");

export function hostNote(host, now = Date.now()) {
  const remaining = cooldown(host.cooldown_until, now);
  if (hostPaused(host)) {
    if (remaining) return "Resume clears both.";
    return `paused by ${pauser(host.paused_by)} · ${clock(host.paused_at)} · ${count(host.running)} still running`;
  }
  return remaining ? `usage limit · back in ${remaining}` : "Ready for new launches.";
}

// Resume clears a pause and a cooldown alike (#653).
export const hostResumable = (host, now = Date.now()) =>
  hostPaused(host) || Boolean(cooldown(host.cooldown_until, now));

// Every wave done: the engine reports no current wave, nothing live and nothing queued.
export const epicComplete = (epic) =>
  Boolean(epic?.waves_total) &&
  epic.current_wave == null &&
  count(epic.live) === 0 &&
  count(epic.queued) === 0;

export function waveLine(board) {
  const tail = `${count(board.live)} LIVE · ${count(board.queued)} QUEUED`;
  if (epicComplete(board)) return `WAVES COMPLETE · ${tail}`;
  return `WAVE ${board.current_wave ?? "—"} OF ${board.waves_total ?? "—"} · ${tail}`;
}

// `current_wave` is the lowest wave still holding work, so a never-launched issue in an
// earlier wave was closed before launch.
const passed = (board, wave) =>
  Boolean(board.waves_total) &&
  wave != null &&
  (board.current_wave == null || wave < board.current_wave);

// Each graph node merged with its issue record.
export function issueRows(board) {
  const issues = new Map((board.issues ?? []).map((issue) => [issue.key, issue]));
  return (board.graph?.issues ?? []).map((node) => {
    const issue = issues.get(node.key);
    return { ...node, ...issue, closed: !issue?.stage && passed(board, node.wave) };
  });
}

// One worker's board row, plus its board.
export function findIssue(boards, key) {
  for (const board of boards ?? []) {
    const row = issueRows(board).find((issue) => issue.key === key);
    if (row) return { board, row };
  }
  return null;
}

export function phaseLine(issue, wave) {
  if (issue.hold) return `HELD · with ${issue.hold.with ?? "another worker"}`;
  if (issue.closed) return "CLOSED BEFORE LAUNCH";
  if (!issue.stage) return `QUEUED · WAVE ${wave ?? "—"}`;
  const phase = issue.phase?.toUpperCase() || issue.stage.toUpperCase();
  return issue.phase_age_ms == null
    ? phase
    : `${phase} · ${Math.floor(count(issue.phase_age_ms) / 60_000)}m`;
}

// Engine-launched Claude Code workers run with `-n <key>`, so their title is the key, not a summary.
export const statusTitle = (live, key) =>
  live?.title && live.title.trim() !== key ? live.title : null;

export function liveTitle(host, title) {
  const kind = host === "claude" ? "session" : host === "muse" ? "folder" : "title";
  return `${kind} ${title}`;
}

export function peerMachine(fleet, name) {
  return fleet?.machines?.find((machine) => !machine.local && machine.name === name);
}

// A link the page may render: plain http or https, with no credentials.
export function linkUrl(raw) {
  if (typeof raw !== "string") return null;
  try {
    const url = new URL(raw);
    return ["http:", "https:"].includes(url.protocol) && !url.username && !url.password
      ? url.href
      : null;
  } catch {
    return null;
  }
}

export function peerDashboardUrl(raw) {
  const href = linkUrl(raw);
  if (!href) return null;
  const url = new URL(href);
  return url.pathname === "/" && !url.search && !url.hash ? href : null;
}

// `live` is the live-agent document; a worker it does not list keeps the board's own status.
export function tiles(boards, hosts, attention, now = Date.now(), live = null) {
  const sampled = new Map((live?.items ?? []).map((item) => [item.key, item.agent_status]));
  const issues = boards.flatMap((board) => board.issues ?? []);
  const running = issues.filter((issue) => LIVE_STAGES.includes(issue.stage));
  const total = boards.reduce(
    (sum, board) => sum + (board.graph?.issues?.length ?? board.issues?.length ?? 0),
    0,
  );
  const states = running.map((issue) => agentState(issue, sampled.get(issue.key)).kind);
  const plans = issues
    .map((issue) => issue.plan_summary)
    .filter((plan) => Number.isFinite(plan?.total));
  const hostItems = hosts.items ?? [];
  const attentionItems = attention.items ?? [];
  const oldest = Math.max(0, ...attentionItems.map((item) => count(item.age_ms)));
  return {
    workers: `${running.length} / ${total} ISSUES`,
    workerNote: `${states.filter((state) => state === "working").length} working, ${states.filter((state) => state === "blocked").length} blocked, ${states.filter((state) => state === "idle").length} idle.`,
    attention: `${attentionItems.length} OPEN`,
    attentionNote: attentionItems.length
      ? `Oldest open for ${Math.floor(oldest / 60_000)}m.`
      : "Nothing waiting on you.",
    hosts: `${hostItems.filter((host) => hostState(host, now).kind === "rotation").length} / ${hostItems.length}`,
    plans: `${plans.reduce((sum, plan) => sum + count(plan.green), 0)} / ${plans.reduce((sum, plan) => sum + count(plan.total), 0)}`,
    planNote: `Across ${plans.length} worker plans.`,
  };
}
