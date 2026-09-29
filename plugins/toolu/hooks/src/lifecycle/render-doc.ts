/**
 * `render_doc` of toolu's session-start.sh (#263): a context doc with its
 * `{{token}}` placeholders replaced in a fixed order. Replacement is literal:
 * `&`, `|` and `$` in a project name land verbatim.
 */
import { readFileSync } from "node:fs";
import { stripTrailingNewlines } from "./bash-compat.ts";

/** `$(cat path)` with each `[token, value]` substituted in order; "" when unreadable. */
export function renderDoc(path: string, tokens: readonly (readonly [string, string])[]): string {
  let content: string;
  try {
    content = stripTrailingNewlines(readFileSync(path, "utf8"));
  } catch {
    return "";
  }
  for (const [token, value] of tokens) {
    content = content.replaceAll(`{{${token}}}`, () => value);
  }
  return content;
}
