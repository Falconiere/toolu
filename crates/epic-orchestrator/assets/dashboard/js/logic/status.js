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
};

const count = (value) => (Number.isFinite(value) && value > 0 ? value : 0);

export function agentState(issue, fallback) {
  if (["merged", "cleaning", "cleanup-incomplete", "abandoned"].includes(issue?.stage)) {
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

export function hostState(host, now = Date.now()) {
  if (host.paused || host.state === "paused") return HOSTS.paused;
  return cooldown(host.cooldown_until, now) ? HOSTS.cooling : HOSTS.rotation;
}

export function hostNote(host, now = Date.now()) {
  if (host.paused || host.state === "paused") return "No new launches.";
  const remaining = cooldown(host.cooldown_until, now);
  return remaining ? `usage limit · back in ${remaining}` : "Ready for new launches.";
}

export function waveLine(board) {
  const live = count(board.live);
  const queued = count(board.queued);
  const tail = `${live} LIVE · ${queued} QUEUED`;
  if (board.waves_total && board.current_wave == null && live === 0 && queued === 0) {
    return `WAVES COMPLETE · ${tail}`;
  }
  return `WAVE ${board.current_wave ?? "—"} OF ${board.waves_total ?? "—"} · ${tail}`;
}

export function issueRows(board) {
  const issues = new Map((board.issues ?? []).map((issue) => [issue.key, issue]));
  return (board.graph?.issues ?? []).map((node) => ({
    ...node,
    ...issues.get(node.key),
  }));
}

export function phaseLine(issue, wave) {
  if (issue.hold) return `HELD · with ${issue.hold.with ?? "another worker"}`;
  if (!issue.stage) return `QUEUED · WAVE ${wave ?? "—"}`;
  const phase = issue.phase?.toUpperCase() || issue.stage.toUpperCase();
  return issue.phase_age_ms == null
    ? phase
    : `${phase} · ${Math.floor(count(issue.phase_age_ms) / 60_000)}m`;
}

export function liveTitle(host, title) {
  const kind = host === "claude" ? "session" : host === "muse" ? "folder" : "title";
  return `${kind} ${title}`;
}

export function peerMachine(fleet, name) {
  return fleet?.machines?.find((machine) => !machine.local && machine.name === name);
}

export function peerDashboardUrl(raw) {
  if (!raw) return null;
  try {
    const url = new URL(raw);
    return ["http:", "https:"].includes(url.protocol) &&
      !url.username &&
      !url.password &&
      url.pathname === "/" &&
      !url.search &&
      !url.hash
      ? url.href
      : null;
  } catch {
    return null;
  }
}

export function tiles(boards, hosts, attention, now = Date.now()) {
  const issues = boards.flatMap((board) => board.issues ?? []);
  const live = issues.filter((issue) => ["running", "awaiting_merge"].includes(issue.stage));
  const total = boards.reduce(
    (sum, board) => sum + (board.graph?.issues?.length ?? board.issues?.length ?? 0),
    0,
  );
  const states = live.map((issue) => agentState(issue).kind);
  const plans = issues
    .map((issue) => issue.plan_summary)
    .filter((plan) => Number.isFinite(plan?.total));
  const hostItems = hosts.items ?? [];
  const attentionItems = attention.items ?? [];
  const oldest = Math.max(0, ...attentionItems.map((item) => count(item.age_ms)));
  return {
    workers: `${live.length} / ${total} ISSUES`,
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
