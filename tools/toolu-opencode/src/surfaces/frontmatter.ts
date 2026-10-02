/**
 * Frontmatter readers for OpenCode surfaces (#345).
 *
 * `parseCanonical` reads the generator's own form (`scripts/lib/frontmatter.ts`):
 * one `key: <JSON>` line per key, keys sorted. Anything else is a packaging
 * error, so it fails instead of guessing.
 *
 * `frontmatterName` reads a user's `SKILL.md` the way the pinned host does: with
 * the host's own `gray-matter@4.0.3` (js-yaml 3), retried after the host's colon
 * sanitizer. A file the host would skip, such as a duplicated key or a
 * non-string description, yields no name.
 */
import matter from "gray-matter";

const FENCE = /^---\r?\n([\s\S]*?)\r?\n---[ \t]*(?:\r?\n|$)([\s\S]*)$/;
const CANONICAL_LINE = /^([A-Za-z0-9_-]+): (.+)$/;

export type Canonical =
  | { ok: true; data: Record<string, unknown>; body: string }
  | { ok: false; reason: string };

function canonicalValue(line: string): { key: string; value: unknown } | string {
  const hit = CANONICAL_LINE.exec(line);
  if (hit === null) return `not a "key: <JSON>" line: ${line}`;
  const [, key = "", json = ""] = hit;
  try {
    return { key, value: JSON.parse(json) };
  } catch {
    return `value of "${key}" is not JSON`;
  }
}

/** The generator's canonical frontmatter and the body after it. */
export function parseCanonical(text: string): Canonical {
  const hit = FENCE.exec(text);
  if (hit === null) return { ok: false, reason: "no frontmatter fence" };
  const [, head = "", body = ""] = hit;
  const data: Record<string, unknown> = {};
  let previous = "";
  for (const line of head.split("\n")) {
    const parsed = canonicalValue(line);
    if (typeof parsed === "string") return { ok: false, reason: parsed };
    if (parsed.key <= previous) return { ok: false, reason: `keys not sorted at "${parsed.key}"` };
    previous = parsed.key;
    data[parsed.key] = parsed.value;
  }
  return { ok: true, data, body };
}

/**
 * The host's retry (`ConfigMarkdown.sanitize`, opencode-ai@1.18.34): an unquoted
 * top-level value containing `:` becomes a block scalar.
 */
function sanitize(content: string): string {
  const match = /^---\r?\n([\s\S]*?)\r?\n---/.exec(content);
  const head = match?.[1];
  if (head === undefined) return content;
  const result = head.split(/\r?\n/).flatMap((line) => {
    if (line.trim().startsWith("#") || line.trim() === "" || /^\s+/.test(line)) return [line];
    const entry = /^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(.*)$/.exec(line);
    const value = entry?.[2]?.trim() ?? "";
    if (entry === null || value === "" || value === ">" || value === "|") return [line];
    if (value.startsWith('"') || value.startsWith("'") || !value.includes(":")) return [line];
    return [`${entry[1]}: |-`, `  ${value}`];
  });
  return content.replace(head, () => result.join("\n"));
}

/** `gray-matter` data, retried after the host's sanitizer, as `ConfigMarkdown.parse` does. */
function matterData(text: string): unknown {
  try {
    return matter(text).data;
  } catch {
    return matter(sanitize(text)).data;
  }
}

/**
 * A skill file's frontmatter `name`, as the host would register it: parsed by the
 * host's own library and accepted only with a string `name` and a string or
 * absent `description` (`isSkillFrontmatter`). Undefined when the host would skip it.
 */
export function frontmatterName(text: string): string | undefined {
  let data: unknown;
  try {
    data = matterData(text);
  } catch {
    return undefined;
  }
  if (typeof data !== "object" || data === null || Array.isArray(data)) return undefined;
  const name: unknown = Reflect.get(data, "name");
  const description: unknown = Reflect.get(data, "description");
  if (description !== undefined && typeof description !== "string") return undefined;
  return typeof name === "string" ? name : undefined;
}
