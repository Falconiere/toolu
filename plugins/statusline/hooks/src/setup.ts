/**
 * `/statusline:setup`: idempotently wire the statusline into Claude Code's
 * `settings.json`. It never clobbers a custom statusLine (`--force` replaces
 * one), backs the file up to `settings.json.bak` before any write, is a no-op
 * once wired, and upgrades the pre-Bun `bash …/statusline.sh` command, which
 * can no longer run the published Bun program. The first word of the output is
 * the STATUS token the command reads: CREATED, WIRED, ALREADY, REFUSED or ERROR.
 */
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname } from "node:path";
import { asObject } from "./statusline/json.ts";
import { MARKER, desiredCommand, isLegacyCommand, settingsPath } from "./statusline/settings.ts";

type Outcome = { code: number; lines: string[] };

/** The settings document, `{}` when the file is absent or empty, or an ERROR outcome. */
function load(settings: string): { data: Record<string, unknown>; created: boolean } | Outcome {
  if (!existsSync(settings) || statSync(settings).size === 0) return { data: {}, created: true };
  let doc: unknown;
  try {
    doc = JSON.parse(readFileSync(settings, "utf8"));
  } catch (error: unknown) {
    const detail = error instanceof Error ? error.message : String(error);
    return { code: 1, lines: [`ERROR could not parse ${settings}: ${detail} — not touching it`] };
  }
  const data = asObject(doc);
  if (data === undefined) {
    return { code: 1, lines: [`ERROR ${settings} is not a JSON object — not touching it`] };
  }
  return { data, created: false };
}

function save(settings: string, data: Record<string, unknown>): Outcome | undefined {
  try {
    mkdirSync(dirname(settings), { recursive: true });
    if (existsSync(settings)) copyFileSync(settings, `${settings}.bak`);
    writeFileSync(settings, `${JSON.stringify(data, null, 2)}\n`);
    return undefined;
  } catch (error: unknown) {
    const detail = error instanceof Error ? error.message : String(error);
    return { code: 1, lines: [`ERROR could not write ${settings}: ${detail}`] };
  }
}

function setup(force: boolean): Outcome {
  const settings = settingsPath(process.env);
  const block = { type: "command", command: desiredCommand(process.env) };
  const loaded = load(settings);
  if ("code" in loaded) return loaded;
  const current = loaded.data["statusLine"];
  const command = asObject(current)?.["command"];
  const legacy = isLegacyCommand(command);
  const commandText = typeof command === "string" ? command : JSON.stringify(command ?? "");
  if (asObject(current) !== undefined && commandText.includes(MARKER) && !legacy) {
    return {
      code: 0,
      lines: ["ALREADY statusLine already points at the statusline plugin — nothing to do."],
    };
  }
  if (current !== undefined && current !== null && !legacy && !force) {
    return {
      code: 3,
      lines: [
        "REFUSED a different statusLine is already set in settings.json.",
        `  current: ${JSON.stringify(current)}`,
        "  re-run with --force to replace it, or set it manually:",
        `    "statusLine": ${JSON.stringify(block)}`,
      ],
    };
  }
  const failed = save(settings, { ...loaded.data, statusLine: block });
  if (failed !== undefined) return failed;
  if (loaded.created) {
    return {
      code: 0,
      lines: [
        `CREATED wrote ${settings} with the statusLine wired. Restart the session to see it.`,
      ],
    };
  }
  const done = legacy
    ? `WIRED updated statusLine in ${settings} to run the Bun statusline directly (backup: settings.json.bak). Restart the session to see it.`
    : `WIRED added statusLine to ${settings} (backup: settings.json.bak). Restart the session to see it.`;
  return { code: 0, lines: [done] };
}

const outcome = setup(process.argv.slice(2).some((arg) => arg === "--force" || arg === "-f"));
process.stdout.write(`${outcome.lines.join("\n")}\n`);
process.exitCode = outcome.code;
