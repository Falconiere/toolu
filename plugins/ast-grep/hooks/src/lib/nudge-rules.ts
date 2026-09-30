/**
 * What search-nudge says, and when (#268). The Grep-tool rules are the bash
 * module's regexes unchanged. Bash/Shell commands are judged from the parsed
 * command line (#283 item 10): only a grep/rg that searches files counts, never
 * one that filters a pipe, and only its own arguments can make it structural.
 */
import { basename } from "node:path";
import { gitInvocation, type ShellAnalysis, type ShellCommand } from "@toolu/core/shell";
import { jqOr, jqRaw } from "./jq-text.ts";

/** What ast-grep is for this call: installed, not on PATH, or opted out in config. */
export type AstGrepState = "available" | "missing" | "opt-out";

export type NudgeKind = "grep-structural" | "bash-structural" | "bash-generic";

const STRUCT_RE =
  /(^|\s)(fn |impl |async fn|async |class |function |struct |trait |interface |type |pub (fn|struct|enum|trait|mod|type|async)|export (function|class|interface|type|const|default)|enum |mod |const fn|#\[derive|#\[cfg|#\[test|@Component|@Injectable|@Module|=>|-> Result|-> impl|dyn |Box<|Arc<|Vec<|Option<|where |for .*in )/u;

/** Non-code files are Grep's alone: ast-grep cannot search them. */
const NON_CODE_GLOB_RE =
  /\*\.(toml|md|markdown|json|jsonc|yaml|yml|txt|env|sql|sh|bash|zsh|fish|lock|cfg|ini|conf|csv|xml|html|svg|css|graphql|gql|proto|makefile|dockerfile)/iu;
const NON_CODE_TYPE_RE =
  /^(toml|json|yaml|md|markdown|html|css|sql|sh|bash|make|docker|config|xml|csv|graphql|proto)$/imu;
const NON_CODE_PATH_RE =
  /(\.claude\/|docs\/|\.github\/|infra\/|scripts\/|\.config|Makefile|Dockerfile|Cargo\.toml|package\.json|tsconfig)/iu;

const SEARCH_TOOLS = new Set(["grep", "rg", "ripgrep"]);

/** `$(echo "$input" | jq -r '.tool_input.<key> // ""')`. */
function field(input: Readonly<Record<string, unknown>>, key: string): string {
  return jqRaw(jqOr(input[key], ""));
}

/** The Grep tool: structural patterns on code files go to ast-grep. */
export function grepToolNudge(input: Readonly<Record<string, unknown>>): NudgeKind | undefined {
  const glob = field(input, "glob");
  const type = field(input, "type");
  const path = field(input, "path");
  if (glob !== "" && NON_CODE_GLOB_RE.test(glob)) return undefined;
  if (type !== "" && NON_CODE_TYPE_RE.test(type)) return undefined;
  if (path !== "" && NON_CODE_PATH_RE.test(path)) return undefined;
  return STRUCT_RE.test(field(input, "pattern")) ? "grep-structural" : undefined;
}

/** A grep, rg or ripgrep, or `git grep`: a command that searches text. */
function isSearch(command: ShellCommand): boolean {
  const name = command.argv[0];
  if (name === null || name === undefined) return false;
  if (SEARCH_TOOLS.has(basename(name))) return true;
  return gitInvocation(command)?.subcommand === "grep";
}

/** Reading a pipe filters another command's output; `xargs` hands it file names to search. */
function filtersPipe(command: ShellCommand): boolean {
  return command.pipeline.index > 0 && !command.wrappers.includes("xargs");
}

/** A Bash/Shell command line: grep/rg searching files goes to the proper tool. */
export function shellNudge(analysis: ShellAnalysis): NudgeKind | undefined {
  if (analysis.unknown) return undefined;
  const searches = analysis.commands.filter((c) => isSearch(c) && !filtersPipe(c));
  if (searches.length === 0) return undefined;
  const structural = searches.some((c) => c.texts.slice(1).some((arg) => STRUCT_RE.test(arg)));
  return structural ? "bash-structural" : "bash-generic";
}

const MESSAGES: Record<NudgeKind, Partial<Record<AstGrepState, string>>> = {
  "grep-structural": {
    available:
      'STOP: Structural code pattern detected. Use ast-grep: `ast-grep run --pattern "your pattern" --lang rust/typescript .` AST-aware matching is far more accurate. Grep is for exact literals on non-code files, or after ast-grep returned nothing.',
    missing:
      "WARN: structural code pattern detected but ast-grep is not installed. Falling back to Grep — expect false positives. Install ast-grep (`cargo install ast-grep` or `brew install ast-grep`) for AST-aware matching.",
  },
  "bash-structural": {
    available:
      'STOP: grep/rg for structural code search. Use ast-grep: `ast-grep run --pattern "your pattern" --lang rust/typescript .` grep/rg is for piping command output or non-code files.',
    missing:
      "WARN: structural grep/rg detected but ast-grep is not installed. Proceeding with grep/rg — expect false positives. Install ast-grep (`cargo install ast-grep` or `brew install ast-grep`) for AST-aware matching.",
  },
  // The Grep-tool-vs-bash-grep advice holds without ast-grep; only its ast-grep half drops.
  "bash-generic": {
    available:
      "grep/rg in Bash detected. Use ast-grep for structural patterns on code files, Grep tool for exact literals on non-code files. Bash grep/rg only for piping command output.",
    missing:
      "grep/rg in Bash detected. Use Grep tool for exact literals on non-code files. Bash grep/rg only for piping command output. (ast-grep not installed — structural matching unavailable.)",
    "opt-out":
      "grep/rg in Bash detected. Use Grep tool for exact literals on non-code files. Bash grep/rg only for piping command output.",
  },
};

/** The nudge for `kind` given ast-grep's state; none when the user opted out of a structural one. */
export function nudgeMessage(kind: NudgeKind, state: AstGrepState): string | undefined {
  return MESSAGES[kind][state];
}
