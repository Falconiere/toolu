/** Structured YAML frontmatter parse/serialize for generated Markdown. */
import { STRIPPED_FRONTMATTER_KEYS } from "./constants.ts";

export type FrontmatterRecord = Record<string, unknown>;

export type ParsedMarkdown = {
  frontmatter: FrontmatterRecord;
  body: string;
  strippedKeys: string[];
};

const FRONTMATTER_RE = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/;

function isRecord(value: unknown): value is FrontmatterRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function preserveQuotedHashInDescription(yaml: string): string {
  return yaml.replace(
    /^description:([ \t]+)([^\r\n]*)$/gm,
    (line, spacing: string, value: string) => {
      const plain = value.trim();
      if (!plain || /^["'|>{\[]/.test(plain)) return line;

      let quoted = false;
      let embeddedHash = false;
      let commentAt = value.length;
      for (let i = 0; i < value.length; i += 1) {
        if (value[i] === '"' && value[i - 1] !== "\\") quoted = !quoted;
        if (value[i] === "#" && /\s/.test(value[i - 1] ?? "")) {
          if (quoted) embeddedHash = true;
          else {
            commentAt = i;
            break;
          }
        }
      }
      if (!embeddedHash) return line;
      return `description:${spacing}${JSON.stringify(value.slice(0, commentAt).trim())}${
        commentAt < value.length ? ` ${value.slice(commentAt)}` : ""
      }`;
    },
  );
}

export function parseFrontmatter(source: string): ParsedMarkdown {
  const hit = FRONTMATTER_RE.exec(source);
  if (!hit) return { frontmatter: {}, body: source, strippedKeys: [] };
  const value: unknown = Bun.YAML.parse(preserveQuotedHashInDescription(hit[1] ?? ""));
  if (!isRecord(value)) throw new Error("frontmatter must be a YAML mapping");
  const frontmatter = { ...value };
  const strippedKeys: string[] = [];
  for (const key of Object.keys(frontmatter)) {
    if (STRIPPED_FRONTMATTER_KEYS.has(key)) {
      strippedKeys.push(key);
      delete frontmatter[key];
    }
  }
  if (frontmatter["allowed-tools"] !== undefined && frontmatter.tools === undefined) {
    frontmatter.tools = frontmatter["allowed-tools"];
    delete frontmatter["allowed-tools"];
  }
  return { frontmatter, body: hit[2] ?? "", strippedKeys: strippedKeys.sort() };
}

function ordered(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(ordered);
  if (!isRecord(value)) return value;
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map((key) => [key, ordered(value[key])]),
  );
}

export function serializeFrontmatter(frontmatter: FrontmatterRecord): string {
  const lines = Object.keys(frontmatter)
    .sort()
    .map((key) => {
      if (!/^[A-Za-z0-9_-]+$/.test(key)) throw new Error(`invalid frontmatter key: ${key}`);
      let value: string | undefined;
      try {
        value = JSON.stringify(ordered(frontmatter[key]));
      } catch (error) {
        throw new Error(`unsupported frontmatter value: ${key}`, { cause: error });
      }
      if (value === undefined) throw new Error(`unsupported frontmatter value: ${key}`);
      return `${key}: ${value}`;
    });
  return `---\n${lines.join("\n")}\n---\n`;
}
