/** Minimal YAML frontmatter parse/serialize for surface generation (#206). */
import { STRIPPED_FRONTMATTER_KEYS } from "./constants.ts";

export type FrontmatterRecord = Record<string, string>;

export type ParsedMarkdown = {
  frontmatter: FrontmatterRecord;
  body: string;
  strippedKeys: string[];
};

const FRONTMATTER_RE = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/;

function parseScalarLine(line: string): { key: string; value: string } | null {
  const match = /^([A-Za-z0-9_-]+):\s*(.*)$/.exec(line);
  if (!match) {
    return null;
  }
  return { key: match[1], value: match[2].trim() };
}

export function parseFrontmatter(source: string): ParsedMarkdown {
  const hit = FRONTMATTER_RE.exec(source);
  if (!hit) {
    return { frontmatter: {}, body: source, strippedKeys: [] };
  }
  const rawBlock = hit[1];
  const body = hit[2];
  const frontmatter: FrontmatterRecord = {};
  const strippedKeys: string[] = [];
  let currentKey: string | null = null;
  let folded = "";

  const flush = (): void => {
    if (!currentKey) {
      return;
    }
    if (STRIPPED_FRONTMATTER_KEYS.has(currentKey)) {
      strippedKeys.push(currentKey);
    } else {
      frontmatter[currentKey] = folded.trim();
    }
    currentKey = null;
    folded = "";
  };

  for (const line of rawBlock.split("\n")) {
    if (line.startsWith("  ") && currentKey) {
      folded = folded ? `${folded} ${line.trim()}` : line.trim();
      continue;
    }
    flush();
    const scalar = parseScalarLine(line);
    if (!scalar) {
      continue;
    }
    if (scalar.value === ">-" || scalar.value === "|") {
      currentKey = scalar.key;
      folded = "";
      continue;
    }
    if (STRIPPED_FRONTMATTER_KEYS.has(scalar.key)) {
      strippedKeys.push(scalar.key);
    } else {
      frontmatter[scalar.key] = scalar.value;
    }
  }
  flush();

  if (frontmatter["allowed-tools"] && !frontmatter.tools) {
    frontmatter.tools = frontmatter["allowed-tools"];
    delete frontmatter["allowed-tools"];
  }

  return { frontmatter, body, strippedKeys };
}

export function serializeFrontmatter(frontmatter: FrontmatterRecord): string {
  const keys = Object.keys(frontmatter).sort();
  const lines = keys.map((key) => `${key}: ${frontmatter[key]}`);
  return `---\n${lines.join("\n")}\n---\n`;
}
