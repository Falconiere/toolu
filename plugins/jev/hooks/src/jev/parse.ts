import { readFileSync } from "node:fs";
import { CliExit } from "@toolu/core/cli";

export const USAGE = `jev.sh <command> [options]

Commands:
  noul   <instructions>    Yes/no judgment -> probability of yes
  choice <instructions>    Pick one option -> choice + probabilities + confidence
  score  <instructions>    Rate on ordered levels -> score + legend + confidence
  ask    <questions-json>  Many typed questions in ONE call (file path, or - for stdin)

Shared options:
  -s, --state VALUE   State to judge: literal text, @FILE, or - for stdin  [required]
  -m, --model NAME    Model (default: jev-latest)
      --id NAME       Question id in the answer map (default: q)
      --raw           Print the whole response body instead of just .answers

noul:    --true DESC / --false DESC   what a yes / a no means
choice:  -o, --option KEY=DESC        repeatable, 2..255 (bare -o KEY sends no description)
score:   -l, --level DESC             repeatable, 2..10, lowest level first`;

type Question = { type: "noul" | "choice" | "score"; instructions: string; criteria?: unknown };
export interface Call {
  state: unknown;
  model: string;
  questions: Record<string, Question>;
  raw: boolean;
}
const fail = (message: string): never => {
  throw new CliExit(1, `jev: ${message}`);
};
const usage = (): never => {
  throw new CliExit(1, USAGE);
};
const required = (args: readonly string[], index: number, name: string): string =>
  args[index + 1] ?? fail(`${name} needs a value`);
const stripLf = (value: string): string => value.replace(/\n+$/, "");
const read = (path: string): string => {
  try {
    return stripLf(readFileSync(path, "utf8"));
  } catch {
    return fail(`cannot read ${path}`);
  }
};
const fromStdin = async (): Promise<string> => stripLf(await Bun.stdin.text());

function structured(text: string): unknown {
  try {
    const value: unknown = JSON.parse(text);
    if (value !== null && typeof value === "object") return value;
  } catch {
    /* Plain text remains a string. */
  }
  return text;
}

export async function parse(argv: readonly string[]): Promise<Call> {
  const [command = "", ...args] = argv;
  if (command === "" || command === "-h" || command === "--help" || command === "help") usage();
  if (!["noul", "choice", "score", "ask"].includes(command))
    throw new CliExit(1, `jev: unknown command: ${command}\n${USAGE}`);
  let stateRaw: string | undefined;
  let model = "jev-latest",
    id = "q",
    raw = false,
    instructions = "",
    source = "";
  const criteria: Record<string, string | null> = {};
  const levels: string[] = [];
  let trueDesc = "",
    falseDesc = "",
    stdinTaken = false;
  const claimStdin = () => {
    if (stdinTaken) fail("only one option can read stdin");
    stdinTaken = true;
  };
  for (let i = 0; i < args.length; i++) {
    const arg = args[i] ?? "";
    if (arg === "-s" || arg === "--state") {
      stateRaw = required(args, i, "--state");
      if (stateRaw === "-") claimStdin();
      i++;
    } else if (arg === "-m" || arg === "--model") {
      model = required(args, i, "--model");
      i++;
    } else if (arg === "--id") {
      if (command === "ask")
        fail("ask takes its question ids from the payload; --id does not apply");
      id = required(args, i, "--id");
      i++;
    } else if (arg === "--raw") raw = true;
    else if (command === "noul" && (arg === "--true" || arg === "--false")) {
      const value = required(args, i, arg);
      if (arg === "--true") trueDesc = value;
      else falseDesc = value;
      i++;
    } else if (command === "choice" && (arg === "-o" || arg === "--option")) {
      const value = required(args, i, "--option");
      const split = value.indexOf("=");
      const key = split < 0 ? value : value.slice(0, split);
      if (!key) fail("--option needs a KEY (KEY=DESCRIPTION)");
      criteria[key] = split < 0 ? null : value.slice(split + 1);
      i++;
    } else if (command === "score" && (arg === "-l" || arg === "--level")) {
      levels.push(required(args, i, "--level"));
      i++;
    } else if (command === "ask" && arg === "-") {
      if (source) fail("unexpected argument: -");
      source = "-";
      claimStdin();
    } else if (arg.startsWith("-")) fail(`unknown option: ${arg}`);
    else if (command === "ask") {
      if (source) fail(`unexpected argument: ${arg}`);
      source = arg;
    } else {
      if (instructions) fail(`unexpected argument: ${arg}`);
      instructions = arg;
    }
  }
  if (command === "ask") {
    if (!source) usage();
  } else if (!instructions) usage();
  const stateSource = stateRaw ?? fail("--state is required");
  let state: unknown = stateSource;
  if (stateSource === "-") state = structured(await fromStdin());
  else if (stateSource.startsWith("@")) state = structured(read(stateSource.slice(1)));
  let questions: Record<string, Question>;
  if (command === "ask") {
    const text = source === "-" ? await fromStdin() : read(source);
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch {
      fail(`invalid JSON in ${source}`);
    }
    const object =
      parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)
        ? parsed
        : fail("questions must be a JSON object");
    if (Object.keys(object).length === 0) fail("questions must not be empty");
    questions = object as Record<string, Question>;
  } else {
    let question: Question;
    if (command === "noul") {
      const sides: Record<string, string> = {};
      if (trueDesc) sides.true = trueDesc;
      if (falseDesc) sides.false = falseDesc;
      question = {
        type: "noul",
        instructions,
        ...(Object.keys(sides).length ? { criteria: sides } : {}),
      };
    } else if (command === "choice") {
      if (Object.keys(criteria).length < 2) fail("choice needs at least 2 options");
      if (Object.keys(criteria).length > 255) fail("choice accepts at most 255 options");
      question = { type: "choice", instructions, criteria };
    } else {
      if (levels.length < 2) fail("score needs at least 2 levels");
      if (levels.length > 10) fail("score accepts at most 10 levels");
      question = { type: "score", instructions, criteria: levels };
    }
    questions = { [id]: question };
  }
  return { state, model, questions, raw };
}
