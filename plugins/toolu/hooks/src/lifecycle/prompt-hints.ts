/**
 * The per-prompt hints of toolu's UserPromptSubmit hook (#263), a port of
 * `user-prompt-submit.sh`'s regex ladder. Bash ERE `=~` anchors to the whole
 * prompt and has no `\b`, so WB/WE wrap each alternation to keep `impl` out
 * of `implement` and `move` out of `remove`.
 */

const WB = "(^|[^a-z])";
const WE = "([^a-z]|$)";

function words(alternation: string): RegExp {
  return new RegExp(`${WB}(${alternation})${WE}`);
}

const TRIVIAL =
  /^(y|n|yes|no|ok|sure|thanks|thank you|go ahead|looks good|lgtm|correct|exactly|right|done|nah|nope|yep|yup|continue)[.!?]?$/;
const VAGUE = /^(fix|help|debug|check|look|see|run|do|try)[ \t\n\v\f\r]*$/;
const GATE_TOPIC = words("fix|resolve|error|warning|test|lint|check|type");
const STRUCTURAL = words(
  "pattern|struct|trait|interface|all functions|all methods|every function|every method|syntax|code structure|signature|return type|where clause|lifetime|closure|macro|decorator|annotation",
);
const INTENTS: readonly (readonly [RegExp, string])[] = [
  [
    words("rename|move|extract|split"),
    "Rename: find all refs (ast-grep + Grep on configs) before rewriting.",
  ],
  [words("test|spec|coverage"), "Tests: real-world data only, NO mocks."],
  [words("fix|debug|error|bug|issue"), "Fix in code. Never suppress with disable comments."],
  [words("delete|remove|clean up"), "Verify no deps before removing."],
  [words("review|audit"), "Review: forbidden syntax, quality gates, test coverage."],
];
const SCALE = words("migrate|codebase-wide|throughout|end-to-end");
const BRAINSTORM = words(
  "brainstorms?|designs?|scopes?|approach(es)?|architectures?|trade-?offs?|redesigns?|overhauls?",
);
const NEW_THING = new RegExp(`${WB}new[ \\t\\n\\v\\f\\r]+(feature|workflow|system)${WE}`);
const RESEARCH = words(
  "latest|docs for|library docs|api reference|api docs|changelog|release notes|best practices?|look up|search the web|web search|how to use",
);

export type PromptGate = "skip" | "block" | "hint";

/** Trivial replies and slash commands get nothing; a one-word verb is blocked. */
export function promptGate(lower: string): PromptGate {
  if (TRIVIAL.test(lower) || lower.startsWith("/")) return "skip";
  return VAGUE.test(lower) ? "block" : "hint";
}

/** A failing gate is worth mentioning unless the prompt is already about fixing. */
export function mentionsGateTopic(lower: string): boolean {
  return GATE_TOPIC.test(lower);
}

export type HintOptions = { astGrep: boolean; research: boolean };

/** At most one intent hint; the most specific pattern wins. */
function intentHint(lower: string, astGrep: boolean): string | undefined {
  if (STRUCTURAL.test(lower)) {
    return astGrep
      ? "Structural pattern: use `ast-grep run --pattern` (not Grep)."
      : "WARN: ast-grep not installed — install via brew/cargo for structural matching.";
  }
  return INTENTS.find(([pattern]) => pattern.test(lower))?.[1];
}

/** The intent hint, then the independent nudges, in the bash order. */
export function promptHints(lower: string, options: HintOptions): string[] {
  const hints: (string | undefined)[] = [
    intentHint(lower, options.astGrep),
    SCALE.test(lower)
      ? "Possibly large task — if it splits into genuinely independent units, consider decomposing it; if it is really one thread of work, just do it. The orchestrator skill has the test for which."
      : undefined,
    BRAINSTORM.test(lower) || NEW_THING.test(lower)
      ? "Scope may be unresolved — consider the `brainstorm` skill for material design choices; skip it when the request is already bounded or mechanical."
      : undefined,
    options.research && RESEARCH.test(lower)
      ? "External research — delegate to the research-agent subagent (uses native web search and fetch) to keep main context lean."
      : undefined,
  ];
  return hints.filter((hint) => hint !== undefined);
}
