import { afterAll, beforeEach, expect, test } from "bun:test";
import { startHttpsFixture } from "@toolu/conformance/https-fixture";
import { join } from "node:path";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";

const bundle = join(import.meta.dir, "../../dist/jev.js");
const fixture = await startHttpsFixture(["api.typesafe.ai"]);
afterAll(() => fixture.stop());
beforeEach(() => fixture.plan([]));

const answer = {
  model: "jev-1.13.0",
  answers: { q: { type: "noul", noul: 0.92 } },
  usage: { input_tokens: 3, output_tokens: 2 },
};

async function jev(
  args: string[],
  key: string | null = "fixture-key",
  stdin = "",
  extraEnv: Record<string, string> = {},
) {
  const env: Record<string, string | undefined> = { ...process.env, ...fixture.env, ...extraEnv };
  delete env.TYPESAFE_API_KEY;
  if (!("JEV_TIMEOUT" in extraEnv)) delete env.JEV_TIMEOUT;
  if (key !== null) env.TYPESAFE_API_KEY = key;
  const child = Bun.spawn([bundle, ...args], {
    env,
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
  });
  child.stdin.write(stdin);
  child.stdin.end();
  const [stdout, stderr, status] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { stdout, stderr, status };
}

test("noul sends one typed question and prints compact answers", async () => {
  fixture.plan([{ body: JSON.stringify(answer) }]);
  const run = await jev(["noul", "-s", "Payouts failing for 3 days", "Urgent?"]);
  expect(run).toEqual({ status: 0, stdout: '{"q":{"type":"noul","noul":0.92}}\n', stderr: "" });
  expect(fixture.connects).toEqual(["api.typesafe.ai:443"]);
  expect(fixture.requests[0]).toMatchObject({
    method: "POST",
    path: "/v1/systemone",
    headers: { authorization: "Bearer fixture-key" },
  });
  expect(JSON.parse(fixture.requests[0]?.body ?? "")).toEqual({
    state: "Payouts failing for 3 days",
    model: "jev-latest",
    questions: { q: { type: "noul", instructions: "Urgent?" } },
  });
});

test("missing key fails before any request", async () => {
  const run = await jev(["noul", "-s", "x", "Urgent?"], null);
  expect(run).toEqual({ status: 1, stdout: "", stderr: "jev: TYPESAFE_API_KEY unset\n" });
  expect(fixture.requests).toHaveLength(0);
});

test("credential line breaks fail before any request", async () => {
  const run = await jev(["noul", "-s", "x", "Urgent?"], "bad\nkey");
  expect(run).toEqual({
    status: 1,
    stdout: "",
    stderr: "jev: TYPESAFE_API_KEY must not contain line breaks\n",
  });
  expect(fixture.requests).toHaveLength(0);
});

test("choice, score and ask keep their criteria shapes and answer projections", async () => {
  const choice = {
    model: "jev-1.13.0",
    answers: {
      q: {
        type: "choice",
        choice: "billing",
        probabilities: { billing: 0.8, tech: 0.2 },
        confidence: 0.8,
      },
    },
    usage: { input_tokens: 3, output_tokens: 2 },
  };
  fixture.plan([{ body: JSON.stringify(choice) }]);
  expect(
    (await jev(["choice", "-s", "ticket", "Which team?", "-o", "billing=Payments", "-o", "tech"]))
      .status,
  ).toBe(0);
  expect(JSON.parse(fixture.requests[0]?.body ?? "").questions.q.criteria).toEqual({
    billing: "Payments",
    tech: null,
  });
  const score = {
    ...choice,
    answers: {
      q: {
        type: "score",
        score: 1.5,
        legend: { "0": "Calm", "1": "Angry", "2": "Furious" },
        probabilities: { "0": 0.1, "1": 0.3, "2": 0.6 },
        confidence: 0.8,
      },
    },
  };
  fixture.plan([{ body: JSON.stringify(score) }]);
  expect(
    (
      await jev([
        "score",
        "-s",
        "ticket",
        "Severity?",
        "-l",
        "Calm",
        "-l",
        "Angry",
        "-l",
        "Furious",
      ])
    ).status,
  ).toBe(0);
  expect(JSON.parse(fixture.requests[0]?.body ?? "").questions.q.criteria).toEqual([
    "Calm",
    "Angry",
    "Furious",
  ]);
  const root = mkdtempSync(join(tmpdir(), "jev-ask-"));
  try {
    const file = join(root, "questions.json");
    writeFileSync(file, JSON.stringify({ urgent: { type: "noul", instructions: "Urgent?" } }));
    fixture.plan([{ body: JSON.stringify({ ...answer, answers: { urgent: answer.answers.q } }) }]);
    const run = await jev(["ask", file, "-s", "ticket", "--raw"]);
    expect(run.status).toBe(0);
    expect(JSON.parse(run.stdout)).toEqual({ ...answer, answers: { urgent: answer.answers.q } });
    expect(JSON.parse(fixture.requests[0]?.body ?? "").questions).toEqual({
      urgent: { type: "noul", instructions: "Urgent?" },
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a Choice option named __proto__ remains a data key", async () => {
  const response = {
    ...answer,
    answers: {
      q: {
        type: "choice",
        choice: "__proto__",
        probabilities: { ["__proto__"]: 0.6, other: 0.4 },
        confidence: 0.6,
      },
    },
  };
  fixture.plan([{ body: JSON.stringify(response) }]);
  const run = await jev([
    "choice",
    "-s",
    "ticket",
    "Which?",
    "-o",
    "__proto__=prototype",
    "-o",
    "other",
  ]);
  expect(run.status).toBe(0);
  const body = JSON.parse(fixture.requests[0]?.body ?? "");
  expect(Object.keys(body.questions.q.criteria)).toEqual(["__proto__", "other"]);
  expect(body.questions.q.criteria["__proto__"]).toBe("prototype");
});

test("ask reads a question map from stdin and pins the model", async () => {
  fixture.plan([{ body: JSON.stringify({ ...answer, answers: { urgent: answer.answers.q } }) }]);
  const run = await jev(
    ["ask", "-", "-s", "ticket", "-m", "jev-1.13"],
    "fixture-key",
    '{"urgent":{"type":"noul","instructions":"Urgent?"}}',
  );
  expect(run.status).toBe(0);
  expect(JSON.parse(fixture.requests[0]?.body ?? "")).toEqual({
    state: "ticket",
    model: "jev-1.13",
    questions: { urgent: { type: "noul", instructions: "Urgent?" } },
  });
});

test("structured file state stays structured; scalar file and stdin stay strings", async () => {
  const root = mkdtempSync(join(tmpdir(), "jev-state-"));
  try {
    const file = join(root, "state.json");
    writeFileSync(file, '{"ticket":"Payouts failing"}\n');
    fixture.plan([{ body: JSON.stringify(answer) }]);
    expect((await jev(["noul", "-s", `@${file}`, "Urgent?"])).status).toBe(0);
    expect(JSON.parse(fixture.requests[0]?.body ?? "").state).toEqual({
      ticket: "Payouts failing",
    });
    writeFileSync(file, "123\n");
    fixture.plan([{ body: JSON.stringify(answer) }]);
    expect((await jev(["noul", "-s", `@${file}`, "Urgent?"])).status).toBe(0);
    expect(JSON.parse(fixture.requests[0]?.body ?? "").state).toBe("123");
    fixture.plan([{ body: JSON.stringify(answer) }]);
    expect(
      (await jev(["noul", "-s", "-", "Urgent?"], "fixture-key", "Payouts failing\n")).status,
    ).toBe(0);
    expect(JSON.parse(fixture.requests[0]?.body ?? "").state).toBe("Payouts failing");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("invalid arguments send no request", async () => {
  for (const args of [
    ["noul", "Urgent?"],
    ["choice", "-s", "x", "Which?", "-o", "one"],
    ["score", "-s", "x", "How?", "-l", "low"],
    ["ask", "-", "-s", "-"],
    ["noul", "-s", "x", "Urgent?", "--weight", "3"],
  ]) {
    const run = await jev(args);
    expect(run.status).toBe(1);
  }
  expect(fixture.requests).toHaveLength(0);
});

test("choice and score enforce upper bounds before network", async () => {
  const choice = ["choice", "-s", "x", "Which?"];
  for (let i = 0; i < 256; i++) choice.push("-o", `key${i}`);
  expect((await jev(choice)).stderr).toBe("jev: choice accepts at most 255 options\n");
  const score = ["score", "-s", "x", "How?"];
  for (let i = 0; i < 11; i++) score.push("-l", String(i));
  expect((await jev(score)).stderr).toBe("jev: score accepts at most 10 levels\n");
  expect(fixture.requests).toHaveLength(0);
});

test("malformed success cannot become a judgment", async () => {
  for (const body of [
    "not json",
    '{"model":"x"}',
    JSON.stringify({ ...answer, answers: { q: { type: "noul", noul: 1.2 } } }),
  ]) {
    fixture.plan([{ body }]);
    const run = await jev(["noul", "-s", "x", "Urgent?"]);
    expect(run.status).toBe(1);
    expect(run.stdout).toBe("");
    expect(run.stderr).toContain("invalid response");
  }
});

test("missing answer, wrong type and invalid distribution fail closed", async () => {
  const choice = {
    model: "jev-1.13.0",
    usage: answer.usage,
    answers: {
      q: { type: "choice", choice: "a", probabilities: { a: 0.8, b: 0.1 }, confidence: 0.8 },
    },
  };
  const noulArgs = ["noul", "-s", "x", "Urgent?"];
  const cases: Array<{ response: unknown; args: string[] }> = [
    { response: { ...answer, answers: {} }, args: noulArgs },
    { response: { ...answer, answers: { q: { type: "choice", choice: "a" } } }, args: noulArgs },
    { response: choice, args: ["choice", "-s", "x", "Which?", "-o", "a", "-o", "b"] },
    { response: { ...answer, usage: { input_tokens: -1, output_tokens: 2 } }, args: noulArgs },
  ];
  for (const { response, args } of cases) {
    fixture.plan([{ body: JSON.stringify(response) }]);
    const run = await jev(args);
    expect(run.status).toBe(1);
    expect(run.stderr).toContain("invalid response");
  }
});

test("a terminal HTTP error prints its body on stderr and exits 22", async () => {
  fixture.plan([{ status: 401, body: '{"error":"bad key"}' }]);
  const run = await jev(["noul", "-s", "x", "Urgent?"]);
  expect(run).toEqual({ status: 22, stdout: "", stderr: '{"error":"bad key"}\n' });
  expect(fixture.requests).toHaveLength(1);
});

test("408 and 529 retry, then a typed answer succeeds", async () => {
  fixture.plan([
    { status: 408, body: "transient" },
    { status: 529, body: "overloaded" },
    { body: JSON.stringify(answer) },
  ]);
  const run = await jev(["noul", "-s", "x", "Urgent?"]);
  expect(run.status).toBe(0);
  expect(run.stderr).toBe("");
  expect(fixture.requests).toHaveLength(3);
});

test("a retry-after value over 60 seconds surfaces HTTP error without waiting", async () => {
  fixture.plan([{ status: 429, body: "busy", headers: { "Retry-After": "61" } }]);
  const run = await jev(["noul", "-s", "x", "Urgent?"]);
  expect(run).toEqual({ status: 22, stdout: "", stderr: "busy\n" });
  expect(fixture.requests).toHaveLength(1);
});

test("a dropped connection is retried to a valid answer", async () => {
  fixture.plan([{ close: true }, { body: JSON.stringify(answer) }]);
  const run = await jev(["noul", "-s", "x", "Urgent?"]);
  expect(run.status).toBe(0);
  expect(fixture.connects.length).toBeGreaterThanOrEqual(2);
  expect(fixture.requests).toHaveLength(1);
});

test("a zero-second per-attempt timeout exits 28 after retries", async () => {
  fixture.plan([{ body: JSON.stringify(answer) }]);
  const run = await jev(["noul", "-s", "x", "Urgent?"], "fixture-key", "", { JEV_TIMEOUT: "0" });
  expect(run.status).toBe(28);
  expect(run.stdout).toBe("");
});
