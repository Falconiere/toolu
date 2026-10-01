import { lstatSync } from "node:fs";

/** The hook already runs under the launcher's resolved Bun; PATH is not required. */
export function invocation(wrapper: string): string {
  const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
  // A preserved user executable may be a shell script with its own interpreter.
  return lstatSync(wrapper).isSymbolicLink()
    ? `${quote(process.execPath)} ${quote(wrapper)}`
    : quote(wrapper);
}

/** Hooks and agent commands can receive different environments. Never expose the key. */
export function credentialNotice(): string {
  return process.env.TYPESAFE_API_KEY
    ? ""
    : "The Jev hook did not receive TYPESAFE_API_KEY. Before reporting Jev unavailable, check whether TYPESAFE_API_KEY is set in the command environment without printing its value; hook and command environments can differ. If absent there too, state the limitation once per task and use an explicit evidence fallback. Never invent a Jev result or read credentials from .env. ";
}
