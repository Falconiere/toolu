/**
 * The Claude Code status line:
 *   model | effort | ctx | <email domain> | <gate> | folder | branch [↑↓] [dirty] | <comemory> | <jev>
 * Byte-identical to the bash renderer it replaces, colours included.
 */
import type { Payload } from "./payload.ts";
import type { ProjectStatus } from "./collect.ts";

const CYAN = "\x1b[36m";
const GREEN = "\x1b[32m";
const YELLOW = "\x1b[33m";
const MAGENTA = "\x1b[35m";
const BLUE = "\x1b[34m";
const RED = "\x1b[31m";
const DIM = "\x1b[2m";
const BOLD = "\x1b[1m";
const RESET = "\x1b[0m";
const SEP = `${DIM} | ${RESET}`;

/** 13779513 → 13.7M, 45000 → 45k, 999 → 999; anything but digits → 0. */
function formatTokens(text: string): string {
  if (!/^[0-9]+$/.test(text)) return "0";
  const n = Number(text);
  if (n >= 1_000_000) {
    return `${Math.floor(n / 1_000_000)}.${Math.floor((n % 1_000_000) / 100_000)}M`;
  }
  return n >= 1000 ? `${Math.floor(n / 1000)}k` : String(n);
}

/** printf `%.0f`: round to nearest, exact halves to even. */
function roundHalfEven(x: number): number {
  const floor = Math.floor(x);
  const diff = x - floor;
  if (diff !== 0.5) return Math.round(x);
  return floor % 2 === 0 ? floor : floor + 1;
}

function contextSegment(payload: Payload): string {
  const tokens = `${formatTokens(payload.ctxUsed)}/${formatTokens(payload.ctxSize)}`;
  return /^[0-9]+(\.[0-9]+)?$/.test(payload.ctxPct)
    ? `${tokens} (${roundHalfEven(Number(payload.ctxPct))}%)`
    : tokens;
}

/** `↑A↓B` then `[+S ~U ?T]`, each part only when nonzero; appended to the branch inside a repo. */
function treeSuffix(status: ProjectStatus): string {
  const ab = `${status.ahead > 0 ? `↑${status.ahead}` : ""}${status.behind > 0 ? `↓${status.behind}` : ""}`;
  const { staged, unstaged, untracked } = status.working_tree;
  const parts = [
    staged > 0 ? `+${staged}` : "",
    unstaged > 0 ? `~${unstaged}` : "",
    untracked > 0 ? `?${untracked}` : "",
  ].filter((part) => part !== "");
  return (
    (ab === "" ? "" : `${DIM}${ab}${RESET}`) +
    (parts.length === 0 ? "" : `${YELLOW}[${parts.join(" ")}]${RESET}`)
  );
}

function jevSegment(jev: ProjectStatus["jev"]): string {
  if (jev.status === "ready") return `${BOLD}${GREEN}[JEV:READY]${RESET}`;
  if (jev.status === "unavailable") {
    return `${BOLD}${YELLOW}[JEV:UNAVAILABLE: ${jev.reason}]${RESET}`;
  }
  return "";
}

/** The line; `domain` is the Claude login's email domain, or "". */
export function renderLine(payload: Payload, status: ProjectStatus, domain: string): string {
  let line = `${CYAN}${payload.model}${RESET}`;
  const add = (segment: string): void => {
    if (segment !== "") line += `${SEP}${segment}`;
  };
  if (payload.effort !== "" && payload.effort !== "null") {
    add(`${YELLOW}effort:${payload.effort}${RESET}`);
  }
  add(`${MAGENTA}ctx:${contextSegment(payload)}${RESET}`);
  add(domain === "" ? "" : `${GREEN}${domain}${RESET}`);
  add(status.gate.status === "failing" ? `${BOLD}${RED}✗ gate:failing${RESET}` : "");
  add(status.folder === "" ? "" : `${BOLD}${status.folder}${RESET}`);
  add(status.branch === "" ? "" : `${BLUE}${status.branch}${RESET}`);
  if (status.repo_root !== "") line += treeSuffix(status);
  add(
    status.comemory_count === null
      ? ""
      : `${BOLD}${GREEN}[COMEMORY:${status.comemory_count}]${RESET}`,
  );
  add(jevSegment(status.jev));
  return line;
}
