/**
 * One session's usage rollup from its transcript file set (main transcript +
 * subagent transcripts): parse each line tolerantly (a malformed or truncated
 * line is skipped), keep assistant messages, dedup by message.id keeping the
 * FINAL streamed frame, price each at its model rate, bucket by LOCAL day.
 *
 * `tokens` is the rate-limit-pacing total (input + output + cache_write);
 * cache_read is tracked separately — ~98% of volume but billed ~0.1x. Project
 * identity (.cwd) is surfaced raw. Order-sensitive sums follow message.id order,
 * the order the jq implementation summed in, so costs match to the last bit.
 */
import { readFileSync } from "node:fs";
import { get, list } from "../../json-path.ts";
import { field, messageCost, rates } from "./pricing.ts";
import { localDay } from "./root.ts";

type Message = {
  id: string;
  day: string;
  model: string;
  skill: string | null;
  tools: string[];
  tokens: number;
  in: number;
  out: number;
  cache_read: number;
  cache_write: number;
  cost: number;
  unknown: boolean;
  cwd: unknown;
};

type Sums = {
  tokens: number;
  input: number;
  output: number;
  cache_read: number;
  cache_write: number;
  cost: number;
};

type Rollup = {
  messages: number;
  cwd: unknown;
  totals: Sums;
  by_day: Record<string, Sums>;
  by_model: Record<string, Sums>;
  tools: Record<string, number>;
  phases: Record<string, number>;
  unknown_models: string[];
};

/** jq `fromdateiso8601 | strflocaltime("%Y-%m-%d")` after dropping fractional seconds; null when unparseable. */
function bucket(timestamp: unknown): string | null {
  if (typeof timestamp !== "string") return null;
  const iso = timestamp.replace(/\.[0-9]+Z$/, "Z");
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(iso)) return null;
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? null : localDay(date);
}

function parseLine(line: string): unknown {
  try {
    return JSON.parse(line);
  } catch {
    return null;
  }
}

function toMessage(entry: unknown): Message | null {
  if (typeof entry !== "object" || entry === null || Array.isArray(entry)) return null;
  const id = get(entry, "message", "id");
  const timestamp = get(entry, "timestamp");
  if (
    get(entry, "type") !== "assistant" ||
    id === undefined ||
    id === null ||
    timestamp === undefined ||
    timestamp === null
  )
    return null;
  const day = bucket(timestamp);
  if (day === null) return null;
  const rawModel = get(entry, "message", "model");
  const model = typeof rawModel === "string" ? rawModel : "";
  const usage = get(entry, "message", "usage") ?? {};
  const r = rates(model);
  const skill = get(entry, "attributionSkill");
  return {
    id: typeof id === "string" ? id : JSON.stringify(id),
    day,
    model,
    skill: typeof skill === "string" ? skill : null,
    tools: list(get(entry, "message", "content"))
      .filter((c) => get(c, "type") === "tool_use")
      .map((c) => String(get(c, "name"))),
    tokens:
      field(usage, "input_tokens") +
      field(usage, "output_tokens") +
      field(usage, "cache_creation_input_tokens"),
    in: field(usage, "input_tokens"),
    out: field(usage, "output_tokens"),
    cache_read: field(usage, "cache_read_input_tokens"),
    cache_write: field(usage, "cache_creation_input_tokens"),
    cost: messageCost(usage, r),
    unknown: r.unknown === true,
    cwd: get(entry, "cwd") ?? null,
  };
}

function sums(msgs: readonly Message[]): Sums {
  const total = (pick: (m: Message) => number): number => msgs.reduce((acc, m) => acc + pick(m), 0);
  return {
    tokens: total((m) => m.tokens),
    input: total((m) => m.in),
    output: total((m) => m.out),
    cache_read: total((m) => m.cache_read),
    cache_write: total((m) => m.cache_write),
    cost: total((m) => m.cost),
  };
}

function byCodepoint(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

function groupSums(msgs: readonly Message[], key: (m: Message) => string): Record<string, Sums> {
  const keys = [...new Set(msgs.map(key))].toSorted(byCodepoint);
  return Object.fromEntries(keys.map((k) => [k, sums(msgs.filter((m) => key(m) === k))]));
}

function counts(values: readonly string[]): Record<string, number> {
  const keys = [...new Set(values)].toSorted(byCodepoint);
  return Object.fromEntries(keys.map((k) => [k, values.filter((v) => v === k).length]));
}

/** Last frame per message.id, ordered by id (jq `group_by(.id) | map(.[-1])`). */
function dedup(all: readonly Message[]): Message[] {
  const last = new Map<string, Message>();
  for (const m of all) last.set(m.id, m);
  return [...last.keys()].toSorted(byCodepoint).flatMap((id) => {
    const m = last.get(id);
    return m === undefined ? [] : [m];
  });
}

export function usageRollup(files: readonly string[]): Rollup {
  // A newline after each file, so one transcript's last line never fuses with the next's first.
  const text = files.map((file) => {
    try {
      return `${readFileSync(file, "utf8")}\n`;
    } catch (err: unknown) {
      // Skipped like the shell harness did, but said out loud: a missing transcript
      // makes the rollup incomplete.
      console.warn(
        `usage: could not read ${file}: ${err instanceof Error ? err.message : String(err)}`,
      );
      return "\n";
    }
  });
  const msgs = dedup(
    text
      .join("")
      .split("\n")
      .map(parseLine)
      .flatMap((e) => toMessage(e) ?? []),
  );
  return {
    messages: msgs.length,
    cwd: msgs.map((m) => m.cwd).find((c) => c !== null) ?? null,
    totals: sums(msgs),
    by_day: groupSums(msgs, (m) => m.day),
    by_model: groupSums(msgs, (m) => m.model),
    tools: counts(msgs.flatMap((m) => m.tools)),
    phases: counts(msgs.flatMap((m) => (m.skill === null ? [] : [m.skill.replace(/^toolu:/, "")]))),
    unknown_models: [...new Set(msgs.filter((m) => m.unknown).map((m) => m.model))].toSorted(
      byCodepoint,
    ),
  };
}
