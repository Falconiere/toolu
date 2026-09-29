/**
 * UserPromptSubmit golden cases (#263): each hint and nudge, the word-boundary
 * false positives the bash regexes were tuned against, and the stdin shapes
 * `jq -r '.prompt // ""'` turns into a prompt.
 */
import type { LifecycleCase } from "./lifecycle-cases.ts";

const FAILING = {
  ".claude/tmp/quality-gate-status.json": '{"status":"failing","reason":"lint errors in a.ts"}',
};

function ups(
  name: string,
  prompt: unknown,
  extra: Omit<LifecycleCase, "name" | "hook" | "stdin"> = {},
) {
  return {
    name: `user-prompt-submit: ${name}`,
    hook: "user-prompt-submit",
    stdin: JSON.stringify({ hook_event_name: "UserPromptSubmit", prompt }),
    ...extra,
  } as const;
}

function raw(name: string, stdin: string) {
  return { name: `user-prompt-submit: ${name}`, hook: "user-prompt-submit", stdin } as const;
}

const SKIPS: readonly LifecycleCase[] = [
  ups("trivial yes", "yes"),
  ups("trivial with punctuation", "Thanks!"),
  ups("trivial go ahead", "go ahead"),
  ups("trivial needs the whole prompt", "yes please"),
  ups("trivial with a backslash", "ok\\"),
  ups("trivial takes one punctuation mark", "ok!!"),
  ups("object prompt", { text: "fix it" }),
  ups("non-ASCII capitals are not lowercased", "FİX the Äpp"),
  ups("slash command", "/delivery-flow:delivery-flow build it"),
  ups("vague fix is blocked", "fix"),
  ups("vague with trailing spaces is blocked", "help   "),
  ups("vague uppercase is blocked", "DEBUG"),
  ups("trailing newlines are stripped like $(…)", "fix\n\n"),
  ups("empty prompt", ""),
  ups("null prompt", null),
  ups("false prompt", false),
  ups("numeric prompt", 42),
  raw("prompt key missing", '{"hook_event_name":"UserPromptSubmit"}'),
  raw("invalid JSON", "{nope"),
  raw("empty stdin", ""),
];

const INTENTS: readonly LifecycleCase[] = [
  ups("plain feature prompt has empty context", "implement a new feature for the dashboard"),
  ups("structural without ast-grep", "find every function that returns a Result"),
  ups("structural with ast-grep", "list all methods on the trait", { astGrep: true }),
  ups("structural with ast-grep skill disabled", "show the signature", {
    astGrep: true,
    userConfig: { skills: { "ast-grep": false } },
  }),
  ups("rename", "rename the helper and move it"),
  ups("tests", "add coverage for the parser"),
  ups("fix", "debug the crash on startup"),
  ups("delete", "clean up the old logs"),
  ups("review", "audit the auth module"),
  ups("one intent when several match", "fix and remove the failing test review the code"),
  ups("implement is not impl", "implement the parser"),
  ups("remove is not move", "remove the old flag"),
  ups("fast is not ast", "make it fast"),
  ups("dropdown is not drop", "add a dropdown"),
  ups("preview is not review", "preview the page"),
  ups("uppercase matches", "FIX THE BUG IN LOGIN"),
  ups("multi-line prompt", "please\nfix the bug\n"),
];

const NUDGES: readonly LifecycleCase[] = [
  ups("scale nudge", "migrate the whole codebase to bun"),
  ups("scale nudge with an intent", "fix the build end-to-end"),
  ups("everyday refactor gets no scale nudge", "refactor this function across two files"),
  ups("auditor is not audit", "the auditor notes are in the doc"),
  ups("brainstorm nudge", "brainstorm the approach for caching"),
  ups("brainstorm plural and trade-off", "compare designs and tradeoffs"),
  ups("brainstorm new feature", "a new  feature for exports"),
  ups("brainstorm inflection stays silent", "we redesigned and scoped it"),
  ups("research nudge", "what are the latest release notes for react"),
  ups("research with intent", "look up the api docs then fix the call"),
  ups("research off by toggle", "search the web for bun best practice", {
    userConfig: { agents: { "research-agent": false } },
  }),
  ups("codebase question gets no research nudge", "where is the config loaded"),
  ups("jira word", "check the jira board"),
  ups("atlassian link", "see https://acme.atlassian.net/browse/ABC-123"),
  ups("issue key with context word", "pick up ABC-42 from the sprint"),
  ups("key-shaped tokens without context", "UTF-8 and GPT-4 encoding"),
  ups("lowercase key does not count", "abc-42 is on the backlog list"),
  ups(
    "every nudge at once",
    "brainstorm how to migrate jira ticket ABC-1 docs for the latest api reference",
  ),
];

const PROJECT: readonly LifecycleCase[] = [
  ups("failing gate hint", "add a button", { untracked: FAILING }),
  ups("fix suppresses the gate hint", "fix the gate", { untracked: FAILING }),
  ups("prefix does not suppress the gate hint", "add a prefix", { untracked: FAILING }),
  ups("passing gate is silent", "add a button", {
    untracked: { ".claude/tmp/quality-gate-status.json": '{"status":"passing"}' },
  }),
  ups("gate without reason", "add a button", {
    untracked: { ".claude/tmp/quality-gate-status.json": '{"status":"failing","reason":false}' },
  }),
  ups("gate reason object", "add a button", {
    untracked: {
      ".claude/tmp/quality-gate-status.json": '{"status":"failing","reason":{"file":"a.ts"}}',
    },
  }),
  ups("malformed gate file", "add a button", {
    untracked: { ".claude/tmp/quality-gate-status.json": "{broken" },
  }),
  ups("claude context.sh", "implement a new feature", {
    untracked: { ".claude/context.sh": 'printf "CTX:%s\\n\\n" "$PROMPT"\n' },
  }),
  ups("codex context.sh", "implement a new feature", {
    host: "codex",
    untracked: { ".codex/context.sh": "echo CODEX_PROJECT_CONTEXT_MARKER\n" },
  }),
  ups("failing context.sh keeps its output", "add coverage", {
    untracked: { ".claude/context.sh": "echo partial; exit 3\n", ...FAILING },
  }),
  ups("outside a git repo", "rename the thing", { git: false }),
  ups("disabled by config", "fix the bug", {
    userConfig: { hooks: { "user-prompt-submit": false } },
  }),
];

export const USER_PROMPT_SUBMIT_CASES: readonly LifecycleCase[] = [
  ...SKIPS,
  ...INTENTS,
  ...NUDGES,
  ...PROJECT,
];
