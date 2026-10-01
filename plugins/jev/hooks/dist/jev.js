#!/usr/bin/env bun
// @bun

// packages/toolu-core/src/cli/cli.ts
class CliExit extends Error {
  code;
  stdout;
  constructor(code, message = "", stdout = "") {
    super(message);
    this.name = "CliExit";
    this.code = code;
    this.stdout = stdout;
  }
}
function isBrokenPipe(error) {
  return error instanceof Error && "code" in error && error.code === "EPIPE";
}
async function writeStdout(text) {
  if (text.length === 0)
    return;
  try {
    await Bun.write(Bun.stdout, text);
  } catch (error) {
    if (isBrokenPipe(error))
      throw new CliExit(141);
    throw error;
  }
}
async function writeStderr(text) {
  if (text !== "")
    await Bun.write(Bun.stderr, text.endsWith(`
`) ? text : `${text}
`);
}
async function report(exit) {
  let code = exit.code;
  try {
    await writeStdout(exit.stdout);
  } catch (error) {
    if (!(error instanceof CliExit))
      throw error;
    code = error.code;
  }
  await writeStderr(exit.message);
  return code;
}
async function runCli(main) {
  let code;
  try {
    code = await main();
  } catch (error) {
    if (error instanceof CliExit) {
      code = await report(error);
    } else {
      await writeStderr(error instanceof Error ? error.message : String(error));
      code = 1;
    }
  }
  process.exit(code);
}

// plugins/jev/hooks/src/jev/parse.ts
import { readFileSync } from "fs";
var USAGE = `jev.sh <command> [options]

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
var fail = (message) => {
  throw new CliExit(1, `jev: ${message}`);
};
var usage = () => {
  throw new CliExit(1, USAGE);
};
var required = (args, index, name) => args[index + 1] ?? fail(`${name} needs a value`);
var stripLf = (value) => value.replace(/\n+$/, "");
var read = (path) => {
  try {
    return stripLf(readFileSync(path, "utf8"));
  } catch {
    return fail(`cannot read ${path}`);
  }
};
var fromStdin = async () => stripLf(await Bun.stdin.text());
function structured(text) {
  try {
    const value = JSON.parse(text);
    if (value !== null && typeof value === "object")
      return value;
  } catch {}
  return text;
}
async function parse(argv) {
  const [command = "", ...args] = argv;
  if (command === "" || command === "-h" || command === "--help" || command === "help")
    usage();
  if (!["noul", "choice", "score", "ask"].includes(command))
    throw new CliExit(1, `jev: unknown command: ${command}
${USAGE}`);
  let stateRaw;
  let model = "jev-latest", id = "q", raw = false, instructions = "", source = "";
  const criteria = {};
  const levels = [];
  let trueDesc = "", falseDesc = "", stdinTaken = false;
  const claimStdin = () => {
    if (stdinTaken)
      fail("only one option can read stdin");
    stdinTaken = true;
  };
  for (let i = 0;i < args.length; i++) {
    const arg = args[i] ?? "";
    if (arg === "-s" || arg === "--state") {
      stateRaw = required(args, i, "--state");
      if (stateRaw === "-")
        claimStdin();
      i++;
    } else if (arg === "-m" || arg === "--model") {
      model = required(args, i, "--model");
      i++;
    } else if (arg === "--id") {
      if (command === "ask")
        fail("ask takes its question ids from the payload; --id does not apply");
      id = required(args, i, "--id");
      i++;
    } else if (arg === "--raw")
      raw = true;
    else if (command === "noul" && (arg === "--true" || arg === "--false")) {
      const value = required(args, i, arg);
      if (arg === "--true")
        trueDesc = value;
      else
        falseDesc = value;
      i++;
    } else if (command === "choice" && (arg === "-o" || arg === "--option")) {
      const value = required(args, i, "--option");
      const split = value.indexOf("=");
      const key = split < 0 ? value : value.slice(0, split);
      if (!key)
        fail("--option needs a KEY (KEY=DESCRIPTION)");
      Object.defineProperty(criteria, key, {
        value: split < 0 ? null : value.slice(split + 1),
        enumerable: true,
        writable: true,
        configurable: true
      });
      i++;
    } else if (command === "score" && (arg === "-l" || arg === "--level")) {
      levels.push(required(args, i, "--level"));
      i++;
    } else if (command === "ask" && arg === "-") {
      if (source)
        fail("unexpected argument: -");
      source = "-";
      claimStdin();
    } else if (arg.startsWith("-"))
      fail(`unknown option: ${arg}`);
    else if (command === "ask") {
      if (source)
        fail(`unexpected argument: ${arg}`);
      source = arg;
    } else {
      if (instructions)
        fail(`unexpected argument: ${arg}`);
      instructions = arg;
    }
  }
  if (command === "ask") {
    if (!source)
      usage();
  } else if (!instructions)
    usage();
  const stateSource = stateRaw ?? fail("--state is required");
  let state = stateSource;
  if (stateSource === "-")
    state = structured(await fromStdin());
  else if (stateSource.startsWith("@"))
    state = structured(read(stateSource.slice(1)));
  let questions;
  if (command === "ask") {
    const text = source === "-" ? await fromStdin() : read(source);
    let parsed;
    try {
      parsed = JSON.parse(text);
    } catch {
      fail(`invalid JSON in ${source}`);
    }
    const object = parsed !== null && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : fail("questions must be a JSON object");
    if (Object.keys(object).length === 0)
      fail("questions must not be empty");
    questions = object;
  } else {
    let question;
    if (command === "noul") {
      const sides = {};
      if (trueDesc)
        sides.true = trueDesc;
      if (falseDesc)
        sides.false = falseDesc;
      question = {
        type: "noul",
        instructions,
        ...Object.keys(sides).length ? { criteria: sides } : {}
      };
    } else if (command === "choice") {
      if (Object.keys(criteria).length < 2)
        fail("choice needs at least 2 options");
      if (Object.keys(criteria).length > 255)
        fail("choice accepts at most 255 options");
      question = { type: "choice", instructions, criteria };
    } else {
      if (levels.length < 2)
        fail("score needs at least 2 levels");
      if (levels.length > 10)
        fail("score accepts at most 10 levels");
      question = { type: "score", instructions, criteria: levels };
    }
    questions = { [id]: question };
  }
  return { state, model, questions, raw };
}

// plugins/jev/hooks/src/jev/response.ts
var record = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
var probability = (value) => typeof value === "number" && value >= 0 && value <= 1;
var sameKeys = (a, b) => JSON.stringify(Object.keys(a).sort()) === JSON.stringify(Object.keys(b).sort());
function distribution(value, keys) {
  if (!record(value) || JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...keys].sort()))
    return false;
  const numbers = Object.values(value);
  return numbers.every(probability) && Math.abs(numbers.reduce((sum, n) => sum + n, 0) - 1) < 0.000001;
}
function project(call, body) {
  let response;
  try {
    response = JSON.parse(body);
  } catch {
    throw new CliExit(1, "jev: invalid response: expected a typed answer for every question");
  }
  const invalid = () => {
    throw new CliExit(1, "jev: invalid response: expected a typed answer for every question");
  };
  const envelope = record(response) ? response : invalid();
  if (typeof envelope.model !== "string" || !envelope.model || !record(envelope.usage) || !Number.isInteger(envelope.usage.input_tokens) || envelope.usage.input_tokens < 0 || !Number.isInteger(envelope.usage.output_tokens) || envelope.usage.output_tokens < 0 || !record(envelope.answers) || !sameKeys(envelope.answers, call.questions))
    invalid();
  const answers = envelope.answers;
  for (const [id, question] of Object.entries(call.questions)) {
    if (!record(question))
      invalid();
    if (question.type !== "noul" && question.type !== "choice" && question.type !== "score")
      invalid();
    const answer = record(answers[id]) ? answers[id] : invalid();
    if (answer.type !== question.type)
      invalid();
    if (question.type === "noul") {
      if (!probability(answer.noul))
        invalid();
    } else if (question.type === "choice") {
      if (!record(question.criteria) || !probability(answer.confidence) || !distribution(answer.probabilities, Object.keys(question.criteria)) || typeof answer.choice !== "string" || !Object.hasOwn(question.criteria, answer.choice))
        invalid();
    } else if (question.type === "score") {
      if (!Array.isArray(question.criteria))
        invalid();
      const levels = question.criteria.map((_, i) => String(i));
      if (!probability(answer.confidence) || typeof answer.score !== "number" || answer.score < 0 || answer.score > levels.length - 1 || !record(answer.legend) || JSON.stringify(Object.keys(answer.legend).sort()) !== JSON.stringify([...levels].sort()) || !distribution(answer.probabilities, levels))
        invalid();
    }
  }
  return call.raw ? `${JSON.stringify(response, null, 2)}
` : `${JSON.stringify(answers)}
`;
}

// plugins/jev/hooks/src/jev.ts
var URL = "https://api.typesafe.ai/v1/systemone";
var retryable = (status) => status === 408 || status === 429 || status >= 500;
var pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
function delay(response, attempt) {
  const seconds = response?.headers.get("retry-after");
  const millis = response?.headers.get("retry-after-ms");
  let value = 2 ** (attempt - 1);
  if (seconds !== null && seconds !== undefined && /^\d+$/.test(seconds)) {
    if (seconds.length > 2)
      return null;
    value = Number(seconds);
  } else if (millis !== null && millis !== undefined && /^\d+$/.test(millis)) {
    if (millis.length > 8)
      return null;
    value = Math.ceil(Number(millis) / 1000);
  }
  if (!Number.isFinite(value) || value > 60)
    return null;
  return Math.max(value, 2 ** (attempt - 1)) * 1000;
}
function transportCode(error) {
  if (!(error instanceof Error))
    return 1;
  if (error.name === "TimeoutError" || error.name === "AbortError")
    return 28;
  const cause = "cause" in error ? error.cause : undefined;
  const code = cause !== null && typeof cause === "object" && "code" in cause ? cause.code : undefined;
  switch (code) {
    case "ENOTFOUND":
      return 6;
    case "ECONNREFUSED":
    case "EHOSTUNREACH":
      return 7;
    case "ETIMEDOUT":
      return 28;
    case "ECONNRESET":
      return 56;
    case "EPIPE":
      return 55;
    default:
      return 1;
  }
}
function timeout() {
  const n = Number(process.env.JEV_TIMEOUT ?? "60");
  return Number.isFinite(n) && n >= 0 ? n * 1000 : 60000;
}
async function post(key, body) {
  for (let attempt = 1;attempt <= 3; attempt++) {
    let response;
    try {
      response = await fetch(URL, {
        method: "POST",
        redirect: "manual",
        headers: {
          Authorization: `Bearer ${key}`,
          "Content-Type": "application/json",
          Accept: "application/json"
        },
        body,
        signal: AbortSignal.timeout(timeout())
      });
      const text = await response.text();
      if (response.status >= 200 && response.status < 300)
        return text;
      const wait = delay(response, attempt);
      if (attempt < 3 && retryable(response.status) && wait !== null) {
        await pause(wait);
        continue;
      }
      throw new CliExit(22, text);
    } catch (error) {
      if (error instanceof CliExit)
        throw error;
      if (attempt < 3) {
        await pause(2 ** (attempt - 1) * 1000);
        continue;
      }
      throw new CliExit(transportCode(error), error instanceof Error ? error.message : String(error));
    }
  }
  throw new CliExit(1, "jev: request failed");
}
async function main() {
  const key = process.env.TYPESAFE_API_KEY ?? "";
  if (!key)
    throw new CliExit(1, "jev: TYPESAFE_API_KEY unset");
  if (/[\r\n]/.test(key))
    throw new CliExit(1, "jev: TYPESAFE_API_KEY must not contain line breaks");
  const call = await parse(process.argv.slice(2));
  const body = await post(key, JSON.stringify({ state: call.state, model: call.model, questions: call.questions }));
  await writeStdout(project(call, body));
  return 0;
}
await runCli(main);
