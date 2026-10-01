import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import {
  agentArgs,
  HOST_LIMIT,
  parseHostKind,
  type HostKind,
} from "../../../../epic-orchestrator/scripts/hosts";
import { DEFAULT_TABLE } from "../../../../epic-orchestrator/scripts/route";

const lib = resolve(import.meta.dir, "../../../scripts/lib");
const name = "pb-3fa2c1-r1g1";

function bash(script: string, args: string[] = []): { status: number | null; stdout: string } {
  const result = spawnSync(
    "bash",
    [
      "-c",
      `. "$1"; . "$2"; ${script}`,
      "babysit-hosts-test",
      `${lib}/fixer-compat.sh`,
      `${lib}/hosts.sh`,
      ...args,
    ],
    { encoding: "utf8" },
  );
  return { status: result.status, stdout: result.stdout.replace(/\n$/, "") };
}

test("24 real host argv combinations match epic agentArgs", () => {
  const models: Record<"claude" | "codex" | "cursor", string> = {
    claude: "opus",
    codex: "gpt-6-sol",
    cursor: "gpt-5.6-sol-high",
  };
  let count = 0;
  for (const kind of ["claude", "codex", "cursor"] as const) {
    for (const bypass of [true, false])
      for (const model of [undefined, models[kind]])
        for (const effort of [undefined, "high"]) {
          const old = bash('pb_agent_args "$3" "$4" "$5" "$6" "$7" auto', [
            kind,
            name,
            model ?? "",
            effort ?? "",
            String(bypass),
          ]);
          expect(old.status).toBe(0);
          expect(old.stdout.split("\n")).toEqual(
            agentArgs(kind, {
              key: name,
              model,
              effort,
              bypass,
              permissionMode: "auto",
              resume: false,
            }),
          );
          count += 1;
        }
  }
  expect(count).toBe(24);
});

test("routing defaults and usage-limit pattern match epic sources", () => {
  const routing = bash("pb_default_routing_json");
  expect(routing.status).toBe(0);
  expect(JSON.parse(routing.stdout)).toEqual({
    claude: DEFAULT_TABLE.hosts.claude,
    codex: DEFAULT_TABLE.hosts.codex,
    cursor: DEFAULT_TABLE.hosts.cursor,
  });
  const limit = bash("pb_host_limit_re");
  expect(limit.stdout).toBe(HOST_LIMIT.source);
});

test("host aliases and invalid names retain the fixer contract", () => {
  for (const alias of ["cursor-agent", "Claude-Code", " Cursor-Agent ", "codex"]) {
    const kind = bash('pb_host_kind "$3"', [alias]);
    expect(kind.status).toBe(0);
    expect(kind.stdout).toBe(parseHostKind(alias) as HostKind);
  }
  expect(bash('pb_host_cli "$3"', ["cursor"]).stdout).toBe("cursor-agent");
  const invalid = bash('pb_host_kind "$3"', ["gemini"]);
  expect(invalid.status).toBe(3);
  expect(JSON.parse(invalid.stdout).errors[0].code).toBe("config_invalid");
});

test("shell-unsafe model identifiers are rejected before launch", () => {
  for (const [kind, model] of [
    ["claude", "a b"],
    ["cursor", "claude[effort=high]"],
  ]) {
    const result = bash('pb_agent_args "$3" "$4" "$5" "" true auto', [
      kind ?? "",
      name,
      model ?? "",
    ]);
    expect(result.status).toBe(3);
    expect(JSON.parse(result.stdout).errors[0].code).toBe("config_invalid");
  }
});
