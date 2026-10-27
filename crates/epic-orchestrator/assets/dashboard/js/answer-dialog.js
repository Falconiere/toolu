import { WORKING_WARNING, counter, sendDisabled } from "./logic/control.js";
import { agentState } from "./logic/status.js";
import { $, mark } from "./view/dom.js";

const dialog = { key: null, opener: null, state: { locked: true, pending: false } };

const syncSend = () => {
  $("answer-send").disabled = sendDisabled({ ...dialog.state, text: $("answer-text").value });
};

// After each control change: Send follows the lock and pending rules; a peer closes the dialog.
export function syncAnswer(state) {
  dialog.state = state;
  if (state.peer) closeAnswer();
  syncSend();
}

export function openAnswer({ key, row, attention, liveStatus }, opener) {
  dialog.key = key;
  dialog.opener = opener;
  const status = agentState(row, liveStatus);
  $("answer-title").textContent = `Answer ${key}`;
  $("answer-status").replaceChildren(mark(status));
  $("answer-asked").textContent = attention?.note || "No open question; this sends a direction.";
  $("answer-warning").textContent = WORKING_WARNING;
  $("answer-warning").hidden = status.kind !== "working";
  $("answer-text").value = "";
  $("answer-count").textContent = counter("");
  $("answer-result").hidden = true;
  syncSend();
  $("answer-dialog").showModal();
  $("answer-text").focus();
}

export function closeAnswer() {
  if ($("answer-dialog").open) $("answer-dialog").close();
}

export function showAnswerResult(result) {
  const box = $("answer-result");
  box.hidden = false;
  box.dataset.kind = result.kind;
  $("answer-result-label").textContent = result.label;
  $("answer-result-text").textContent =
    result.detail && result.detail !== result.text
      ? `${result.text} Engine: ${result.detail}`
      : result.text;
}

// Focus goes back to the opener, or to its re-rendered twin with the same control id.
function restoreFocus() {
  const { opener, key } = dialog;
  dialog.key = null;
  if (opener?.isConnected) {
    opener.focus();
    return;
  }
  const answers = [...document.querySelectorAll('button[data-action="answer"]')];
  (
    answers.find((button) => button.dataset.control === opener?.dataset.control) ??
    answers.find((button) => button.dataset.key === key)
  )?.focus();
}

export function initAnswer(onSend) {
  $("answer-text").addEventListener("input", () => {
    $("answer-count").textContent = counter($("answer-text").value);
    syncSend();
  });
  $("answer-cancel").addEventListener("click", closeAnswer);
  $("answer-send").addEventListener("click", () => {
    if (!$("answer-send").disabled && dialog.key) onSend(dialog.key, $("answer-text").value);
  });
  $("answer-dialog").addEventListener("close", restoreFocus);
}
