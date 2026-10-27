import { stripAnsi } from "../logic/logs.js";
import { $, element, text } from "./dom.js";

const atBottom = (node) => node.scrollHeight - node.scrollTop - node.clientHeight < 24;

function renderList(items, selected) {
  const list = $("drawer-log-list");
  const signature = JSON.stringify([items, selected]);
  if (list.dataset.signature === signature) return;
  list.dataset.signature = signature;
  const focused = document.activeElement?.dataset.logId;
  const top = list.scrollTop;
  list.replaceChildren(
    ...items.map((item) => {
      const row = element("li");
      const button = element(
        "button",
        "",
        `${item.source === "job" ? "JOB" : "CLAUDE TASK"} · ${item.state === "running" ? "● RUNNING" : "✓ DONE"} · ${stripAnsi(item.label)}`,
      );
      button.type = "button";
      button.dataset.logId = item.id;
      button.setAttribute("aria-pressed", String(item.id === selected));
      row.append(button);
      return row;
    }),
  );
  list.scrollTop = top;
  if (focused)
    [...list.querySelectorAll("button")]
      .find((button) => button.dataset.logId === focused)
      ?.focus();
}

export function renderLogs(model) {
  const { logs, peer, token } = model;
  const items = logs.items ?? [];
  const note = peer
    ? "◇ LOCAL ONLY · Open the peer dashboard to follow its worker logs."
    : !token
      ? "○ TOKEN REQUIRED · Open the token URL to read worker logs."
      : logs.error
        ? `? LOGS UNAVAILABLE · ${logs.error}`
        : logs.items === null
          ? "Loading logs…"
          : items.length === 0
            ? "No background logs for this worker."
            : logs.unavailable
              ? `? LOG UNAVAILABLE · ${logs.unavailable}`
              : logs.truncated
                ? "Earlier output was removed; showing the kept tail."
                : "";
  text("drawer-logs-note", note);
  renderList(peer || !token ? [] : items, logs.id);
  const output = $("drawer-log-text");
  output.hidden = Boolean(peer || !token || !logs.id || logs.unavailable);
  if (output.hidden) {
    $("drawer-log-jump").hidden = true;
    return;
  }
  const selectedChanged = output.dataset.selected !== logs.id;
  const following = selectedChanged || atBottom(output);
  output.dataset.selected = logs.id;
  const value = stripAnsi(logs.raw);
  if (output.textContent !== value) output.textContent = value;
  if (following) output.scrollTop = output.scrollHeight;
  $("drawer-log-jump").hidden = atBottom(output);
}

export function bindLogs(onSelect) {
  $("drawer-log-list").addEventListener("click", (event) => {
    const id = event.target.closest?.("button[data-log-id]")?.dataset.logId;
    if (id) onSelect(id);
  });
  const output = $("drawer-log-text");
  output.addEventListener("scroll", () => {
    $("drawer-log-jump").hidden = atBottom(output);
  });
  $("drawer-log-jump").addEventListener("click", () => {
    output.scrollTop = output.scrollHeight;
    $("drawer-log-jump").hidden = true;
  });
}
