import { CliExit } from "@toolu/core/cli";
import type { Call } from "./parse.ts";

const record = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);
const probability = (value: unknown): value is number =>
  typeof value === "number" && value >= 0 && value <= 1;
const sameKeys = (a: Record<string, unknown>, b: Record<string, unknown>) =>
  JSON.stringify(Object.keys(a).sort()) === JSON.stringify(Object.keys(b).sort());
function distribution(value: unknown, keys: string[]): boolean {
  if (
    !record(value) ||
    JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...keys].sort())
  )
    return false;
  const numbers = Object.values(value);
  return (
    numbers.every(probability) &&
    Math.abs(numbers.reduce<number>((sum, n) => sum + (n as number), 0) - 1) < 0.000001
  );
}
export function project(call: Call, body: string): string {
  let response: unknown;
  try {
    response = JSON.parse(body);
  } catch {
    throw new CliExit(1, "jev: invalid response: expected a typed answer for every question");
  }
  const invalid = (): never => {
    throw new CliExit(1, "jev: invalid response: expected a typed answer for every question");
  };
  const envelope = record(response) ? response : invalid();
  if (
    typeof envelope.model !== "string" ||
    !envelope.model ||
    !record(envelope.usage) ||
    !Number.isInteger(envelope.usage.input_tokens) ||
    (envelope.usage.input_tokens as number) < 0 ||
    !Number.isInteger(envelope.usage.output_tokens) ||
    (envelope.usage.output_tokens as number) < 0 ||
    !record(envelope.answers) ||
    !sameKeys(envelope.answers, call.questions)
  )
    invalid();
  const answers = envelope.answers as Record<string, unknown>;
  for (const [id, question] of Object.entries(call.questions)) {
    if (!record(question)) invalid();
    if (question.type !== "noul" && question.type !== "choice" && question.type !== "score")
      invalid();
    const answer = record(answers[id]) ? (answers[id] as Record<string, unknown>) : invalid();
    if (answer.type !== question.type) invalid();
    if (question.type === "noul") {
      if (!probability(answer.noul)) invalid();
    } else if (question.type === "choice") {
      if (
        !record(question.criteria) ||
        !probability(answer.confidence) ||
        !distribution(answer.probabilities, Object.keys(question.criteria)) ||
        typeof answer.choice !== "string" ||
        !Object.hasOwn(question.criteria, answer.choice)
      )
        invalid();
    } else if (question.type === "score") {
      if (!Array.isArray(question.criteria)) invalid();
      const levels = (question.criteria as string[]).map((_, i) => String(i));
      if (
        !probability(answer.confidence) ||
        typeof answer.score !== "number" ||
        answer.score < 0 ||
        answer.score > levels.length - 1 ||
        !record(answer.legend) ||
        JSON.stringify(Object.keys(answer.legend).sort()) !== JSON.stringify([...levels].sort()) ||
        !distribution(answer.probabilities, levels)
      )
        invalid();
    }
  }
  return call.raw ? `${JSON.stringify(response, null, 2)}\n` : `${JSON.stringify(answers)}\n`;
}
