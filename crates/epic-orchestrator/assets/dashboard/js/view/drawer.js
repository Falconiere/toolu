import {
  NO_PLAN,
  PLAN_UNAVAILABLE,
  PLAN_UNREADABLE,
  attachCommand,
  attentionLine,
  clock,
  epicLine,
  nextStep,
  phaseTimeline,
  planLine,
  prUrl,
  screenNotice,
  stepMark,
} from "../logic/drawer.js";
import { agentState, liveTitle, phaseLine, statusTitle } from "../logic/status.js";
import { $, element, mark, text } from "./dom.js";
import { showPeerLink } from "./peer.js";

const show = (id, visible) => {
  $(id).hidden = !visible;
};

// Each render rebuilds the list; keep the reader's scroll position in it.
function refill(list, data, build) {
  const signature = JSON.stringify(data);
  if (list.dataset.signature === signature) return;
  list.dataset.signature = signature;
  const top = list.scrollTop;
  list.replaceChildren(...data.map(build));
  list.scrollTop = top;
}

function renderHeader(model, issue) {
  text("drawer-epic", epicLine(model.key, model.board?.key));
  show("drawer-paused", Boolean(model.board?.paused));
  text("drawer-title", issue.title || model.key);
  const meta = $("drawer-meta");
  meta.replaceChildren();
  for (const [label, value] of [
    ["HOST", issue.host],
    ["MODEL", issue.model],
    ["EFFORT", issue.effort],
  ]) {
    const pair = element("div");
    pair.append(element("dt", "", label), element("dd", "", value || "—"));
    meta.append(pair);
  }
  text("drawer-phase", phaseLine(issue, issue.wave));
  const link = $("drawer-pr");
  const href = prUrl(model.board?.url, issue.pr);
  link.hidden = !href;
  if (href) {
    link.href = href;
    link.textContent = `PR #${issue.pr} ↗`;
  }
}

function renderStatus(model, issue) {
  const line = $("drawer-status");
  line.replaceChildren(mark(agentState(issue, model.live?.agent_status)));
  if (model.peer) line.append(element("span", "", "live status on peer"));
  else if (statusTitle(model.live, model.key)) {
    line.append(element("span", "", liveTitle(model.live.host || issue.host, model.live.title)));
  }
}

function renderAttention(model) {
  show("drawer-attention", Boolean(model.attention));
  if (!model.attention) return;
  text("drawer-attention-head", attentionLine(model.attention, model.now, model.sampledAt));
  text("drawer-attention-text", model.attention.note || "Needs attention.");
}

function renderPlan(plan, loaded, failed) {
  const ready = Boolean(plan && !plan.error);
  show("drawer-plan-bar", ready && plan.summary?.total > 0);
  show("drawer-next", ready && Boolean(nextStep(plan)));
  text("drawer-plan-line", ready ? planLine(plan) : "");
  const note = $("drawer-plan-note");
  note.replaceChildren();
  if (!loaded) note.append(failed ? PLAN_UNAVAILABLE : "Loading plan…");
  else if (plan?.error) note.append(mark(PLAN_UNREADABLE), ` ${PLAN_UNREADABLE.text}`);
  else if (!plan) note.append(NO_PLAN);
  if (!ready) {
    refill($("drawer-steps"), [], () => null);
    return;
  }
  const bar = $("drawer-plan-bar");
  bar.max = Math.max(1, plan.summary?.total ?? 0);
  bar.value = Math.min(plan.summary?.green ?? 0, bar.max);
  text("drawer-next", `NEXT ${nextStep(plan) ?? ""}`);
  refill($("drawer-steps"), plan.steps ?? [], (step) => {
    const status = stepMark(step.status);
    const row = element("li", `drawer-step step-${status.kind}`);
    row.append(mark(status), element("span", "step-title", step.title || step.id));
    return row;
  });
}

function renderScreen(model, issue) {
  const notice = screenNotice({ ...model, issue });
  const body = model.screen?.body;
  text(
    "drawer-screen-meta",
    model.peer ? "LOCAL ONLY" : `POLLED 2s · ${clock(body?.sampled_at, true)}`,
  );
  show("drawer-notice", Boolean(notice));
  if (notice) {
    text("drawer-notice-label", notice.label);
    text("drawer-notice-text", notice.text);
  }
  showPeerLink($("drawer-peer-link"), model.peer ? model.machine : null);
  const attach = notice ? null : attachCommand(model.screen);
  show("drawer-blocked", Boolean(attach));
  if (attach && $("drawer-attach").textContent !== `$ ${attach}`) {
    text("drawer-attach", `$ ${attach}`);
    $("drawer-copy").dataset.command = attach;
  }
  show("drawer-screen-text", !notice);
  const screen = $("drawer-screen-text");
  const value = typeof body?.text === "string" ? body.text : "Reading the screen…";
  if (notice || screen.textContent === value) return;
  // Follow the newest lines, as a terminal does, unless the reader scrolled up.
  const following = screen.scrollHeight - screen.scrollTop - screen.clientHeight < 24;
  screen.textContent = value;
  if (following) screen.scrollTop = screen.scrollHeight;
}

function renderPhases(issue) {
  refill($("drawer-phase-list"), phaseTimeline(issue.phase_history, issue), (row) => {
    const item = element("li", row.current ? "phase-row phase-current" : "phase-row");
    if (row.current) item.setAttribute("aria-current", "step");
    item.append(element("span", "", row.phase), element("time", "", row.at));
    return item;
  });
}

export function renderDrawer(model) {
  const issue = { ...model.doc, ...model.row };
  $("drawer").setAttribute("aria-label", `Worker ${model.key}`);
  show("drawer", true);
  $("workspace").classList.add("drawer-open");
  renderHeader(model, issue);
  renderStatus(model, issue);
  renderAttention(model);
  renderPlan(model.doc?.plan, Boolean(model.doc), model.docFailed);
  renderScreen(model, issue);
  renderPhases(issue);
  // Attention keys carry `data-issue` too; only the board row is the current one.
  for (const row of document.querySelectorAll("#board button[data-issue]")) {
    if (row.dataset.issue === model.key) row.setAttribute("aria-current", "true");
    else row.removeAttribute("aria-current");
  }
}

export function revealDrawer() {
  if (matchMedia("(max-width: 900px)").matches) $("drawer").scrollIntoView({ block: "start" });
}

export function hideDrawer() {
  show("drawer", false);
  $("workspace").classList.remove("drawer-open");
  text("drawer-attach", "");
  $("drawer-actions").replaceChildren();
  $("drawer-screen-text").textContent = "";
  for (const row of document.querySelectorAll("button[aria-current]")) {
    row.removeAttribute("aria-current");
  }
}

export function bindDrawer(onClose) {
  $("drawer-close").addEventListener("click", onClose);
  const button = $("drawer-copy");
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(button.dataset.command ?? "");
      button.textContent = "COPIED ✓";
    } catch {
      button.textContent = "SELECT AND COPY";
    }
    setTimeout(() => {
      button.textContent = "[ COPY ]";
    }, 2000);
  });
}
