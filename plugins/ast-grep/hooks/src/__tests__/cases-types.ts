/** Shapes of the ast-grep golden cases (#268): one hook call each, run through toolu's dispatcher. */
import type { PretoolHost } from "@toolu/conformance/harness/pretool";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import type { EnvPatch } from "@toolu/conformance/harness/spawn";

/** What search-nudge sees of ast-grep: installed, absent from PATH, or opted out in config. */
export type AstGrepState = "available" | "missing" | "opt-out";

export type NudgeCase = {
  readonly name: string;
  /** Default: Claude Code only. */
  readonly hosts?: readonly PretoolHost[];
  /** Default: `available`. */
  readonly state?: AstGrepState;
  readonly toolName: string;
  readonly toolInput: Record<string, unknown>;
};

export type SavingsCase = {
  readonly name: string;
  readonly hosts?: readonly PretoolHost[];
  /** The PostToolUse payload, minus `cwd` and `hook_event_name`. */
  readonly payload: (sb: Sandbox) => Record<string, unknown>;
  /** Extra hook environment, for registration and the call alike. */
  readonly env?: (sb: Sandbox) => EnvPatch;
  readonly setup?: (sb: Sandbox) => void;
};

export type ReportCase = {
  readonly name: string;
  /** Ledger body written to `ledger.jsonl`; absent means the argument names a missing file. */
  readonly ledger?: string;
  /** Pass no argument at all. */
  readonly noArgument?: boolean;
};

/** What the TypeScript module does where bash was wrong (#283 item 10 and its parse-based consequences). */
export type Deviation =
  | { readonly silent: true }
  | { readonly contains: string; readonly excludes?: string }
  | { readonly contains?: string; readonly excludes: string };
