/**
 * The optional-tool warning and the proactive-use mandate of toolu's
 * SessionStart (#263). A mandate needs the plugin installed (or an
 * indeterminate install record), its skill enabled, and its tool usable:
 * ast-grep on PATH, or the CLI wrapper exa-search / context7 publish under the
 * config root (exa-search also needs EXA_API_KEY).
 */
import { join } from "node:path";
import { enabled, type LoadedConfig } from "@toolu/core/config";
import type { HostName } from "@toolu/core/host";
import { isExecutable, onPath } from "./bash-compat.ts";
import { pluginActive } from "./plugin-presence.ts";

type Env = Record<string, string | undefined>;

export type MandateInput = { config: LoadedConfig; env: Env; host: HostName; configRoot: string };

export function hasAstGrep(env: Env): boolean {
  const path = env.PATH ?? "";
  return onPath("sg", path) || onPath("ast-grep", path);
}

export function missingToolsWarning(input: MandateInput): string | undefined {
  if (hasAstGrep(input.env) || !enabled(input.config, "skills", "ast-grep")) return undefined;
  return "WARN: optional tools missing — features that depend on them are disabled:\n  • ast-grep (structural code search)";
}

function wanted(input: MandateInput, skill: string): boolean {
  return (
    enabled(input.config, "skills", skill) && pluginActive(`${skill}@toolu`, input.env, input.host)
  );
}

function mandates(input: MandateInput): string[] {
  const out: string[] = [];
  if (hasAstGrep(input.env) && wanted(input, "ast-grep")) {
    out.push(
      "ast-grep (structural search) — for ANY search by code shape (signatures, call sites, impls, trait/interface usage, patterns) you MUST reach for `ast-grep run --pattern …` FIRST. Grep/ripgrep/sed are a FALLBACK ONLY — use them for plain-text literals in non-code files, or when a query genuinely cannot be expressed structurally. Never reach for them first on code.",
    );
  }
  const exa = join(input.configRoot, "exa-search", "search.sh");
  if ((input.env.EXA_API_KEY ?? "") !== "" && isExecutable(exa) && wanted(input, "exa-search")) {
    out.push(
      `exa-search (web search) — for ANY web search, code-example hunt, URL crawl, or topic research you MUST reach for \`"${exa}"\` FIRST (commands: search / crawl / similar — see the exa-search skill). Native web tools are a FALLBACK ONLY — use them when the wrapper errors or a URL needs your logged-in session.`,
    );
  }
  const ctx7 = join(input.configRoot, "context7", "search.sh");
  if (isExecutable(ctx7) && wanted(input, "context7")) {
    out.push(
      `context7 (library docs) — for ANY third-party library/framework question (API usage, current docs, code examples, version behavior) you MUST query \`"${ctx7}"\` FIRST (\`search <library>\` to resolve the ID, then \`docs <id> <query>\`) BEFORE answering from memory or searching the web. Web search is a FALLBACK ONLY when context7 lacks coverage.`,
    );
  }
  return out;
}

export function mandateBlock(input: MandateInput): string | undefined {
  const found = mandates(input);
  if (found.length === 0) return undefined;
  const lines = found.map((mandate) => `\n  • ${mandate}`).join("");
  return `MANDATORY — proactive plugin use (installed + available; NOT optional, do NOT ask permission):${lines}\n  • Propagation: these mandates bind EVERY agent in this session, not just the main thread. When you delegate via Task/subagents (nested ones too), carry the same requirement into the subagent's prompt — recall/save via comemory, ast-grep first with Grep/sed as fallback. Delegation never exempts the work.`;
}
