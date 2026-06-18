// Read-only kanban client. Subscribes to /api/events (SSE), renders each ledger
// step as a card in the column for its status, and animates column moves with
// the FLIP technique plus pulse/shake/flash feedback. No framework, no build.

const COLUMNS = ["pending", "running", "red", "green"];

const els = {
  board: document.getElementById("board"),
  empty: document.getElementById("empty"),
  status: document.getElementById("status"),
  plan: document.getElementById("plan"),
  cols: Object.fromEntries(COLUMNS.map((c) => [c, document.getElementById(`col-${c}`)])),
  counts: Object.fromEntries(
    COLUMNS.map((c) => [c, document.querySelector(`[data-count="${c}"]`)]),
  ),
};

// Remember the last column of each card so we can FLIP-animate real moves.
const lastColumn = new Map();
let lastAllGreen = false;

/** Map a derived step to its column key. */
function columnOf(step) {
  if (step.status === "running") return "running";
  if (step.status === "red") return "red";
  if (step.status === "green") return "green";
  return "pending";
}

/** Build (or update) a card element for a step. */
function renderCard(step) {
  let card = document.getElementById(`card-${step.id}`);
  if (!card) {
    card = document.createElement("article");
    card.className = "card";
    card.id = `card-${step.id}`;
    card.innerHTML =
      '<div class="id"></div><div class="title"></div><div class="activity"></div>';
  }
  card.dataset.status = step.status;
  card.classList.toggle("stale", Boolean(step.stale));
  card.classList.toggle("stuck", Boolean(step.stuck));
  let badge = "";
  if (step.stale) badge = ' <span class="badge">↻ stale</span>';
  else if (step.stuck) badge = ' <span class="badge">⚠ stuck</span>';
  card.querySelector(".id").textContent = step.id;
  card.querySelector(".title").innerHTML = escapeHtml(step.title) + badge;
  const activity = card.querySelector(".activity");
  activity.textContent = step.activity || "";
  activity.hidden = !step.activity;
  return card;
}

/** Minimal HTML escaping for untrusted title text. */
function escapeHtml(s) {
  return String(s).replace(
    /[&<>"']/g,
    (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c],
  );
}

/** Render a full state payload, FLIP-animating any card that changed column. */
function render(state) {
  els.plan.textContent = state.ledger ? state.ledger.plan_doc : "—";
  const steps = state.steps || [];
  els.empty.hidden = steps.length > 0;
  els.board.hidden = steps.length === 0;

  // FLIP step 1: record current positions before the DOM mutates.
  const first = new Map();
  for (const step of steps) {
    const card = document.getElementById(`card-${step.id}`);
    if (card) first.set(step.id, card.getBoundingClientRect());
  }

  const counts = { pending: 0, running: 0, red: 0, green: 0 };
  const seen = new Set();
  for (const step of steps) {
    const col = columnOf(step);
    counts[col]++;
    seen.add(step.id);
    const card = renderCard(step);
    const prevCol = lastColumn.get(step.id);
    els.cols[col].appendChild(card);
    // Shake only on the transition into "red" (not when it stays blocked).
    if (col === "red" && prevCol !== "red") {
      card.classList.remove("shake");
      void card.offsetWidth;
      card.classList.add("shake");
    }
    lastColumn.set(step.id, col);
  }

  // Drop cards for steps that vanished (e.g. branch switch).
  for (const id of [...lastColumn.keys()]) {
    if (!seen.has(id)) {
      document.getElementById(`card-${id}`)?.remove();
      lastColumn.delete(id);
    }
  }

  for (const c of COLUMNS) els.counts[c].textContent = String(counts[c]);

  // FLIP step 2: invert + play for cards that moved.
  for (const step of steps) {
    const card = document.getElementById(`card-${step.id}`);
    const prev = first.get(step.id);
    if (!card || !prev) continue;
    const now = card.getBoundingClientRect();
    const dx = prev.left - now.left;
    const dy = prev.top - now.top;
    if (dx === 0 && dy === 0) continue;
    card.classList.remove("flip");
    card.style.transform = `translate(${dx}px, ${dy}px)`;
    requestAnimationFrame(() => {
      card.classList.add("flip");
      card.style.transform = "";
    });
  }

  // All-green flash on the transition into a fully-done board.
  const allGreen = steps.length > 0 && steps.every((s) => s.status === "green" && !s.stale);
  if (allGreen && !lastAllGreen) {
    els.board.classList.remove("flash");
    void els.board.offsetWidth;
    els.board.classList.add("flash");
  }
  lastAllGreen = allGreen;
}

/** Connect to the SSE stream and re-render on each `state` event. */
function connect() {
  const source = new EventSource("/api/events");
  source.addEventListener("open", () => setConn("open", "live"));
  source.addEventListener("error", () => setConn("closed", "reconnecting…"));
  source.addEventListener("state", (ev) => {
    setConn("open", "live");
    try {
      render(JSON.parse(ev.data));
    } catch {
      /* ignore a malformed frame; the next one wins */
    }
  });
}

function setConn(stateName, label) {
  els.status.dataset.state = stateName;
  els.status.textContent = label;
}

connect();
