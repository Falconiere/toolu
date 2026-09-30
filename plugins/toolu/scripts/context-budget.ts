/** Enforce context-window budgets on harness-injected text.
 *
 * Every session, Claude Code loads the Session Protocol + per-language docs and
 * all skill `description` fields into the model's context. This guard caps that
 * recurring footprint so it cannot silently regrow, and asserts that trimming
 * never drops a skill's discriminating trigger phrases (which would stop it
 * auto-firing).
 *
 * Fails CLOSED: a missing/renamed target file, or a description that can't be
 * parsed, is RED — never a silent pass.
 *
 * Usage: bun context-budget.ts [docs|skills]   (no arg = all) */

import { spawnSync } from "node:child_process";
import { readFileSync, statSync } from "node:fs";

/** One reported line: `ok` goes to stdout, `RED` to stderr. */
export type Line = { red: boolean; text: string };

const ok = (text: string): Line => ({ red: false, text });
const red = (text: string): Line => ({ red: true, text });

/** Text of a regular file, or null when absent (`[ -f ]`). */
function readRegular(path: string): string | null {
  try {
    return statSync(path).isFile() ? readFileSync(path, "utf8") : null;
  } catch {
    return null;
  }
}

/** `wc -w` in a UTF-8 locale: runs of non-whitespace, where no-break and other
 * Unicode spaces also separate words (as macOS and GNU wc count them). */
function wordCount(text: string): number {
  return text.split(/\s+/).filter((word) => word !== "").length;
}

/** Word count of a file, or null when the file is missing. */
export function countWords(path: string): number | null {
  const text = readRegular(path);
  return text === null ? null : wordCount(text);
}

const FOLDED_MARKERS = new Set(["", ">", ">-", "|", "|-"]);

/** The YAML frontmatter `description` value: single-line, or a folded/block
 * scalar joined with single spaces. "" when there is none; null when the file
 * is absent. */
export function extractDescription(path: string): string | null {
  const text = readRegular(path);
  if (text === null) return null;
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();
  let folded = false;
  let buf = "";
  for (const line of lines) {
    if (!folded) {
      if (!line.startsWith("description:")) continue;
      const value = line.slice("description:".length).replace(/^[ \t]*/, "");
      if (!FOLDED_MARKERS.has(value)) return value;
      folded = true;
      continue;
    }
    if (line === "---" || /^[A-Za-z0-9_-]+:/.test(line)) break;
    const part = line.replace(/^[ \t]+/, "");
    buf = buf === "" ? part : `${buf} ${part}`;
  }
  return folded ? buf : "";
}

export function checkDoc(root: string, name: string, path: string, budget: number): Line[] {
  const n = countWords(`${root}/${path}`);
  if (n === null) return [red(`${name}: MISSING ${path}`)];
  return n <= budget
    ? [ok(`${name} ${n}w (<= ${budget})`)]
    : [red(`${name} ${n}w EXCEEDS ${budget} (${path})`)];
}

export function checkSkill(
  root: string,
  name: string,
  path: string,
  budget: number,
  phrases: string[],
): Line[] {
  const desc = extractDescription(`${root}/${path}`);
  if (desc === null) return [red(`${name}: MISSING ${path}`)];
  if (desc === "") return [red(`${name}: empty/unparseable description (${path})`)];
  const n = wordCount(desc);
  const out = [
    n <= budget
      ? ok(`${name} desc ${n}w (<= ${budget})`)
      : red(`${name} desc ${n}w EXCEEDS ${budget} (${path})`),
  ];
  for (const phrase of phrases) {
    if (phrase !== "" && !desc.includes(phrase)) {
      out.push(red(`${name}: missing trigger phrase "${phrase}" (${path})`));
    }
  }
  return out;
}

/** Word ceilings for the `hooks/docs/*.md` the SessionStart hook injects, in check order. */
export const DOC_BUDGETS: readonly (readonly [name: string, budget: number])[] = [
  ["session-start", 110],
  ["model-routing", 90],
  ["post-compaction", 28],
  ["session-start-ts", 30],
  ["session-start-rust", 36],
  ["session-start-python", 36],
];

function runDocs(root: string): Line[] {
  return DOC_BUDGETS.flatMap(([name, budget]) =>
    checkDoc(root, name, `plugins/toolu/hooks/docs/${name}.md`, budget),
  );
}

function runSkills(root: string): Line[] {
  return [
    // Trimmed skills: word ceiling + trigger phrases that MUST survive the trim.
    ...checkSkill(
      root,
      "delivery-flow",
      "plugins/delivery-flow/skills/delivery-flow/SKILL.md",
      50,
      ["implement and deliver", "real-data execution", "PR"],
    ),
    ...checkSkill(root, "brainstorm", "plugins/brainstorm/skills/brainstorm/SKILL.md", 50, [
      "brainstorm",
      "trade-offs",
    ]),
    ...checkSkill(root, "deep-research", "plugins/toolu/skills/deep-research/SKILL.md", 90, [
      "deep research",
      "cited report",
    ]),
    ...checkSkill(root, "toolu-review", "plugins/toolu-review/skills/review/SKILL.md", 65, [
      "review before push",
    ]),
    // Already-lean skills: word ceiling only, lock against regrowth.
    ...checkSkill(root, "ast-grep", "plugins/ast-grep/skills/ast-grep/SKILL.md", 40, []),
    ...checkSkill(root, "jev", "plugins/jev/skills/jev/SKILL.md", 60, []),
  ];
}

/** A non-empty `CONTEXT_BUDGET_ROOT`, else the git toplevel, else "" (every target then reads MISSING). */
function resolveRoot(): string {
  const fromEnv = process.env.CONTEXT_BUDGET_ROOT;
  if (fromEnv) return fromEnv;
  const res = spawnSync("git", ["rev-parse", "--show-toplevel"], { encoding: "utf8" });
  return res.status === 0 ? res.stdout.replace(/\n+$/, "") : "";
}

function main(argv: string[]): number {
  const mode = argv[0] ?? "all";
  const root = resolveRoot();
  let lines: Line[];
  if (mode === "docs") lines = runDocs(root);
  else if (mode === "skills") lines = runSkills(root);
  else if (mode === "all") lines = [...runDocs(root), ...runSkills(root)];
  else {
    console.error("usage: context-budget.ts [docs|skills]");
    return 2;
  }
  for (const line of lines) {
    if (line.red) process.stderr.write(`RED  ${line.text}\n`);
    else process.stdout.write(`ok   ${line.text}\n`);
  }
  return lines.some((line) => line.red) ? 1 : 0;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
