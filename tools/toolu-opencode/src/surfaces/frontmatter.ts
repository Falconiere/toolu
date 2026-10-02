/**
 * Frontmatter readers for OpenCode surfaces (#345).
 *
 * `parseCanonical` reads the generator's own form (`scripts/lib/frontmatter.ts`):
 * one `key: <JSON>` line per key, keys sorted. Anything else is a packaging
 * error, so it fails instead of guessing.
 *
 * `frontmatterName` reads a user's `SKILL.md` the way the pinned host does
 * (`gray-matter` over js-yaml, retried after the host's colon sanitizer): a BOM
 * is ignored, CRLF works, and a parse failure, including a duplicated key that
 * js-yaml rejects, yields no name, because the host then skips that file.
 */
const FENCE = /^---\r?\n([\s\S]*?)\r?\n---[ \t]*(?:\r?\n|$)([\s\S]*)$/;
const CANONICAL_LINE = /^([A-Za-z0-9_-]+): (.+)$/;
const TOP_LEVEL_KEY = /^(?:"([^"]*)"|'([^']*)'|([^\s#'"][^:]*?))\s*:(?:\s|$)/;

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

/** The host's retry: an unquoted value containing `:` becomes a block scalar. */
function sanitize(head: string): string {
  return head
    .split(/\r?\n/)
    .flatMap((line) => {
      if (line.trim().startsWith("#") || line.trim() === "" || /^\s+/.test(line)) return [line];
      const entry = /^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(.*)$/.exec(line);
      const value = entry?.[2]?.trim() ?? "";
      if (entry === null || value === "" || value === ">" || value === "|") return [line];
      if (value.startsWith('"') || value.startsWith("'") || !value.includes(":")) return [line];
      return [`${entry[1]}: |-`, `  ${value}`];
    })
    .join("\n");
}

function hasDuplicateKey(head: string): boolean {
  const seen = new Set<string>();
  for (const line of head.split(/\r?\n/)) {
    const hit = TOP_LEVEL_KEY.exec(line);
    if (hit === null) continue;
    const key = hit[1] ?? hit[2] ?? hit[3] ?? "";
    if (seen.has(key)) return true;
    seen.add(key);
  }
  return false;
}

function parseYaml(head: string): unknown {
  try {
    return Bun.YAML.parse(head);
  } catch {
    return Bun.YAML.parse(sanitize(head));
  }
}

/** A skill file's frontmatter `name`, as the host would read it; undefined when it would not. */
export function frontmatterName(text: string): string | undefined {
  const hit = FENCE.exec(text.charCodeAt(0) === 0xfeff ? text.slice(1) : text);
  if (hit === null) return undefined;
  const head = hit[1] ?? "";
  if (hasDuplicateKey(head)) return undefined;
  let data: unknown;
  try {
    data = parseYaml(head);
  } catch {
    return undefined;
  }
  if (typeof data !== "object" || data === null || Array.isArray(data)) return undefined;
  const name: unknown = Reflect.get(data, "name");
  return typeof name === "string" ? name : undefined;
}
