import { postAction } from "./api.js";
import {
  closeAnswer,
  initAnswer,
  openAnswer,
  showAnswerResult,
  submissionFor,
  syncAnswer,
} from "./answer-dialog.js";
import {
  LOCKED_BANNER,
  ROTATED_BANNER,
  TOKEN_KEY,
  actionBody,
  actionResult,
  answerBody,
  keyAttention,
  mintSubmissionId,
} from "./logic/control.js";
import { takeToken } from "./logic/fragment.js";
import { findIssue } from "./logic/status.js";
import { $ } from "./view/dom.js";

const control = { pending: false, toastTimer: undefined };
let current = () => ({});
let repaint = () => {};

// The token lives only in this tab's sessionStorage; privacy modes that throw read as locked.
function storage(action) {
  try {
    return action(sessionStorage);
  } catch {
    return null;
  }
}
export const readToken = () => storage((store) => store.getItem(TOKEN_KEY));
const dropToken = () => storage((store) => store.removeItem(TOKEN_KEY));

export const controlState = () => ({
  locked: !readToken(),
  pending: control.pending,
  peer: current().peer ?? null,
});

function showBanner(banner) {
  $("control-banner").hidden = !banner;
  $("control-banner-label").textContent = banner?.label ?? "";
  $("control-banner-text").textContent = banner?.text ?? "";
}

// A token URL opened in this tab, on load or as a hash change, moves to sessionStorage.
function storeToken() {
  const fragment = takeToken(location.hash);
  if (!fragment.token) return;
  storage((store) => store.setItem(TOKEN_KEY, fragment.token));
  history.replaceState(null, "", `${location.pathname}${location.search}${fragment.hash}`);
  showBanner(null);
  repaint();
}

// A 401 means the token rotated: forget it, unless a newer one arrived meanwhile.
export function rotated(token) {
  if (readToken() !== token) return;
  dropToken();
  showBanner(ROTATED_BANNER);
  repaint();
}

function toast(result) {
  clearTimeout(control.toastTimer);
  const box = $("toast");
  box.hidden = false;
  box.dataset.kind = result.kind;
  $("toast-label").textContent = result.label;
  $("toast-text").textContent = result.text;
  control.toastTimer = setTimeout(() => {
    box.hidden = true;
  }, 8000);
}

// One request at a time, never from a peer view, never without a token.
async function act(op, body, partner) {
  const token = readToken();
  if (current().peer || !token || control.pending) return null;
  control.pending = true;
  $("toast").hidden = true;
  repaint();
  const reply = await postAction(op, body, token);
  control.pending = false;
  if (reply.status === 401) rotated(token);
  repaint();
  return reply.status === 401
    ? { kind: "rotated", ...ROTATED_BANNER }
    : actionResult(op, body, reply.status, reply.body, partner);
}

function answerFor(key, opener) {
  const { docs, live, peer } = current();
  if (peer || !docs) return;
  openAnswer(
    {
      key,
      row: findIssue(docs.boards, key)?.row ?? { key },
      attention: keyAttention(docs.attention, key),
      liveStatus: (live?.items ?? []).find((item) => item.key === key)?.agent_status,
    },
    opener,
  );
  syncAnswer(controlState());
}

async function send(key, text) {
  // Same key+text while the dialog is open reuses the id so a retry cannot double-type.
  const result = await act(
    "answer",
    answerBody(key, text, submissionFor(key, text, mintSubmissionId)),
  );
  if (!result) return;
  // A dialog cancelled while the answer was pending cannot show the result; the toast can.
  if (result.kind !== "delivered" && $("answer-dialog").open) {
    showAnswerResult(result);
    return;
  }
  closeAnswer();
  if (result.kind !== "rotated") toast(result);
}

async function onClick(event) {
  const button = event.target.closest?.("button[data-action]");
  if (!button || button.disabled) return;
  const { action } = button.dataset;
  if (action === "answer") {
    answerFor(button.dataset.key, button);
    return;
  }
  const result = await act(action, actionBody(action, button.dataset), button.dataset.with);
  if (result && result.kind !== "rotated") toast(result);
}

// Renders rebuild the rows that hold buttons, and a button moved between pointerdown and
// pointerup gets no click. Replay it when the press starts and ends on the same control.
const press = { id: null, clicked: false };
const pressed = (event) => event.target.closest?.("button[data-control]");

function trackPress() {
  document.addEventListener("pointerdown", (event) => {
    press.id = pressed(event)?.dataset.control ?? null;
    press.clicked = false;
  });
  document.addEventListener("click", () => (press.clicked = true), true);
  document.addEventListener("pointerup", (event) => {
    const { id } = press;
    press.id = null;
    const button = pressed(event);
    if (!id || button?.dataset.control !== id) return;
    setTimeout(() => {
      if (!press.clicked) button.click();
    });
  });
}

export function initControl(state, onRepaint) {
  current = state;
  repaint = () => {
    onRepaint();
    syncAnswer(controlState());
  };
  storeToken();
  initAnswer(send);
  document.addEventListener("click", onClick);
  trackPress();
  window.addEventListener("hashchange", storeToken);
  $("control-toggle").addEventListener("click", () => {
    if (readToken()) {
      dropToken();
      showBanner(null);
    } else showBanner(LOCKED_BANNER);
    repaint();
  });
}
