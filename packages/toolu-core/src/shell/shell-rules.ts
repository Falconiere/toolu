/**
 * Argv rules per simple command (#284, #283 item 4): a rule's first token must
 * name the command and every later token must appear later in its argv. It is
 * tried on the words as written and on the unwrapped command, so `sudo node -e`
 * matches both `sudo` and `node -e`, and `cd /tmp && node -e …` matches through
 * its second command.
 */
import { basename } from "node:path";
import type { ShellCommand } from "./shell-types.ts";

function argvMatches(
  argv: readonly (string | null)[],
  head: string,
  tail: readonly string[],
): boolean {
  const [name, ...rest] = argv;
  if (name === null || name === undefined) return false;
  const named = head.includes("/") ? name === head : basename(name) === head;
  return named && tail.every((token) => rest.includes(token));
}

/** Whether `command` matches `rule` ("node -e", "cargo test", "biome"). An empty rule matches nothing. */
export function matchesRule(command: ShellCommand, rule: string): boolean {
  const [head, ...tail] = rule.trim().split(/\s+/);
  if (head === undefined || head === "") return false;
  return argvMatches(command.words, head, tail) || argvMatches(command.argv, head, tail);
}
