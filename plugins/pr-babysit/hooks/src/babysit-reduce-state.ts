#!/usr/bin/env bun
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join } from "node:path";
import { reduceState } from "./babysit/reduce";

type Options = Record<string, string>;

function failure(code: string, message: string, extra: object = {}): never {
  process.stdout.write(
    `${JSON.stringify({ version: 1, errors: [{ code, message, ...extra }] })}\n`,
  );
  process.exit(code === "usage" ? 2 : 3);
}

function atomicJson(path: string, value: unknown): void {
  const dir = dirname(path);
  mkdirSync(dir, { recursive: true });
  const temp = join(dir, `.${basename(path)}.tmp.${process.pid}.${crypto.randomUUID()}`);
  try {
    writeFileSync(temp, `${JSON.stringify(value)}\n`);
    renameSync(temp, path);
  } catch (error) {
    try {
      unlinkSync(temp);
    } catch {
      /* already removed */
    }
    throw error;
  }
}

const args = process.argv.slice(2);
const options: Options = {};
const recognized = new Set([
  "--snapshot",
  "--state",
  "--now",
  "--state-out",
  "--result-out",
  "--state-path",
  "--snapshot-path",
]);
for (let index = 0; index < args.length; index += 2) {
  const flag = args[index]!;
  if (!recognized.has(flag)) failure("usage", `reduce-state.sh: unknown argument: ${flag}`);
  options[flag] = args[index + 1] ?? "";
}
const snapshotPath = options["--snapshot"] ?? "";
const statePath = options["--state"] ?? "";
const now = options["--now"] ?? "";
if (!snapshotPath) failure("usage", "reduce-state.sh: --snapshot <path> required");
if (!statePath) failure("usage", "reduce-state.sh: --state <path> required");
if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(now ?? ""))
  failure(
    "usage",
    "reduce-state.sh: --now must be an ISO-8601 UTC timestamp (YYYY-MM-DDTHH:MM:SSZ)",
  );
if (!existsSync(snapshotPath))
  failure("invalid_json", `reduce-state.sh: snapshot not found: ${snapshotPath}`, {
    source: "snapshot",
  });
let snapshot: any;
try {
  snapshot = JSON.parse(readFileSync(snapshotPath, "utf8"));
} catch {
  failure("invalid_json", `reduce-state.sh: snapshot is not valid JSON: ${snapshotPath}`, {
    source: "snapshot",
  });
}
if (
  snapshot?.version !== 1 ||
  typeof snapshot.repo !== "string" ||
  typeof snapshot.number !== "number" ||
  typeof snapshot.head?.sha !== "string" ||
  !snapshot.pr ||
  typeof snapshot.pr !== "object" ||
  Array.isArray(snapshot.pr) ||
  !Array.isArray(snapshot.threads)
) {
  failure("invalid_json", "reduce-state.sh: snapshot is not a version-1 pr-babysit snapshot", {
    source: "snapshot",
  });
}
let previous: any = null;
if (existsSync(statePath)) {
  try {
    previous = JSON.parse(readFileSync(statePath, "utf8"));
  } catch {
    failure("state_malformed", `reduce-state.sh: state file is not valid JSON: ${statePath}`, {
      source: "state",
    });
  }
  if (previous?.version !== 2)
    failure("state_malformed", `reduce-state.sh: state file is not version 2: ${statePath}`, {
      source: "state",
      version: previous?.version ?? null,
    });
  if (previous.repo !== snapshot.repo || previous.number !== snapshot.number)
    failure(
      "slot_mismatch",
      `reduce-state.sh: state belongs to ${previous.repo}#${previous.number}, snapshot is ${snapshot.repo}#${snapshot.number}`,
      {
        source: "state",
        state: { repo: previous.repo, number: previous.number },
        snapshot: { repo: snapshot.repo, number: snapshot.number },
      },
    );
}
let output;
try {
  output = reduceState(
    snapshot,
    previous,
    now,
    options["--state-path"] || statePath,
    options["--snapshot-path"] || snapshotPath,
  );
} catch {
  failure("invalid_json", `reduce-state.sh: reducer failed on ${snapshotPath}`, {
    source: "reduce",
  });
}
if (options["--state-out"]) {
  try {
    atomicJson(options["--state-out"], output.state);
  } catch {
    failure("invalid_json", `reduce-state.sh: could not write ${options["--state-out"]}`, {
      source: "state_out",
    });
  }
}
if (options["--result-out"]) {
  try {
    atomicJson(options["--result-out"], output.result);
  } catch {
    failure("invalid_json", `reduce-state.sh: could not write ${options["--result-out"]}`, {
      source: "result_out",
    });
  }
} else process.stdout.write(`${JSON.stringify(output.result)}\n`);
