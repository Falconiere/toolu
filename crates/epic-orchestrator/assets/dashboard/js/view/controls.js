import {
  ACTION_LABELS,
  attentionAction,
  controlDisabled,
  drawerActions,
  liveAgent,
} from "../logic/control.js";
import { findIssue, hostResumable } from "../logic/status.js";
import { $, element } from "./dom.js";

const NAMES = { answer: "Answer", release: "Release hold on", ack: "Acknowledge" };

function actionButton(action, label, name, data, disabled) {
  const button = element("button", "control-button", label);
  button.type = "button";
  button.dataset.action = action;
  Object.assign(button.dataset, data);
  button.dataset.control = [action, ...Object.values(data)].join(":");
  button.setAttribute("aria-label", name);
  button.disabled = disabled;
  return button;
}

export const focusedControl = () => document.activeElement?.dataset?.control ?? null;

export function restoreControl(id) {
  if (!id || focusedControl() === id) return;
  [...document.querySelectorAll("button[data-control]")]
    .find((button) => button.dataset.control === id)
    ?.focus({ preventScroll: true });
}

function renderChip(locked, peer) {
  $("control-state").textContent = locked ? "○ CONTROL LOCKED" : "● CONTROL UNLOCKED";
  $("control-chip").classList.toggle("control-unlocked", !locked);
  const toggle = $("control-toggle");
  toggle.hidden = Boolean(peer);
  toggle.textContent = locked ? "[ UNLOCK ]" : "[ LOCK ]";
  toggle.setAttribute("aria-label", locked ? "Unlock control" : "Lock control");
}

function fillEpics(boards, off) {
  for (const slot of document.querySelectorAll(".epic-control-slot[data-epic]")) {
    const board = boards.find((item) => item.key === slot.dataset.epic);
    const action = board?.paused ? "resume" : "pause";
    const label = board?.paused ? "RESUME EPIC" : "PAUSE EPIC";
    const name = `${board?.paused ? "Resume" : "Pause"} epic ${slot.dataset.epic}`;
    slot.replaceChildren(actionButton(action, label, name, { epic: slot.dataset.epic }, off));
  }
}

function fillHosts(hosts, off, now) {
  for (const slot of document.querySelectorAll(".host-control-slot[data-host]")) {
    const host = hosts.find((item) => item.host === slot.dataset.host);
    const resume = host && hostResumable(host, now);
    slot.replaceChildren(
      actionButton(
        resume ? "host-resume" : "host-pause",
        resume ? "RESUME" : "PAUSE",
        `${resume ? "Resume" : "Pause"} host ${slot.dataset.host}`,
        { host: slot.dataset.host },
        off,
      ),
    );
  }
}

function fillAttention(model, off) {
  for (const slot of document.querySelectorAll(".attention-action-slot")) {
    const { kind, key, state } = slot.dataset;
    const action = attentionAction(kind, key, state);
    if (!action) {
      slot.replaceChildren(element("span", "attention-state", `✓ ${state.toUpperCase()}`));
      continue;
    }
    const row = findIssue(model.docs.boards, key)?.row;
    const noAgent = action === "answer" && !liveAgent(row);
    const name = `${NAMES[action]} ${key || kind}`;
    slot.replaceChildren(
      actionButton(action, ACTION_LABELS[action], name, { key, kind }, off || noAgent),
    );
  }
}

// `model`: {docs, live, peer, locked, pending, now}.
export function renderControls(model) {
  const peer = Boolean(model.peer);
  const off = controlDisabled(model);
  renderChip(model.locked, peer);
  for (const id of ["pause-all", "resume-all"]) {
    $(id).hidden = peer;
    $(id).disabled = off;
  }
  if (peer || !model.docs) {
    for (const slot of document.querySelectorAll(
      ".epic-control-slot, .host-control-slot, .attention-action-slot",
    )) {
      slot.replaceChildren();
    }
    return;
  }
  fillEpics(model.docs.boards ?? [], off);
  fillHosts(model.docs.hosts?.items ?? [], off, model.now);
  fillAttention(model, off);
}

// The drawer's ANSWER →, RELEASE HOLD and ACK; a peer drawer gets none.
export function renderDrawerActions(model) {
  const target = $("drawer-actions");
  const focus = focusedControl();
  if (model.peer) {
    target.replaceChildren();
    return;
  }
  target.replaceChildren(
    ...drawerActions(model).map(({ action, label, disabled }) =>
      actionButton(
        action,
        label,
        `${NAMES[action]} ${model.issue.key}`,
        { key: model.issue.key, kind: model.attention?.kind ?? "", from: "drawer" },
        disabled,
      ),
    ),
  );
  restoreControl(focus);
}
