// @bun
// plugins/pr-babysit/hooks/src/babysit/common.ts
class BabysitError extends Error {
  code;
  extra;
  constructor(code, message, extra = {}) {
    super(message);
    this.code = code;
    this.extra = extra;
  }
}
function exitCode(code) {
  if (code === "usage")
    return 2;
  if (code === "duplicate_reply")
    return 4;
  if (code === "resolve_unconfirmed")
    return 5;
  if (code === "locked")
    return 75;
  return 3;
}
function errorDocument(error) {
  return { version: 1, errors: [{ code: error.code, message: error.message, ...error.extra }] };
}
function fail(code, message, extra = {}) {
  throw new BabysitError(code, message, extra);
}
function runCli(action) {
  Promise.resolve().then(action).then((result) => {
    if (result !== undefined)
      process.stdout.write(`${JSON.stringify(result)}
`);
  }).catch((error) => {
    if (error instanceof BabysitError) {
      process.stdout.write(`${JSON.stringify(errorDocument(error))}
`);
      process.exitCode = exitCode(error.code);
    } else {
      const message = error instanceof Error ? error.message : String(error);
      process.stdout.write(`${JSON.stringify(errorDocument(new BabysitError("api_error", message)))}
`);
      process.exitCode = 3;
    }
  });
}
function parseFlags(argv, command, valued, bare = []) {
  const flags = {};
  for (let i = 0;i < argv.length; i += 1) {
    const arg = argv[i] ?? "";
    if (bare.includes(arg)) {
      flags[arg] = true;
    } else if (valued.includes(arg)) {
      flags[arg] = argv[i + 1] ?? "";
      i += 1;
    } else {
      fail("usage", `${command}: unknown argument: ${arg}`);
    }
  }
  return flags;
}
function flag(flags, key) {
  const value = flags[key];
  return typeof value === "string" ? value : "";
}

// plugins/pr-babysit/hooks/src/babysit/fixer-route.ts
import { spawnSync } from "child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { homedir, tmpdir } from "os";
import { join } from "path";
var TIERS = ["trivial", "standard", "complex", "critical"];
var SAFE = /^[A-Za-z0-9_./:=,@%+#-]+$/;
var DEFAULTS = {
  claude: [
    { model: "sonnet", effort: "low" },
    { model: "sonnet", effort: "medium" },
    { model: "opus", effort: "high" },
    { model: "opus", effort: "xhigh" }
  ],
  codex: [
    { model: "gpt-6-sol", effort: "low" },
    { model: "gpt-6-sol", effort: "medium" },
    { model: "gpt-6-sol", effort: "high" },
    { model: "gpt-6-sol", effort: "xhigh" }
  ],
  cursor: [
    { model: "composer-2.5" },
    { model: "gpt-5.6-sol-high" },
    { model: "claude-opus-5-thinking-high" },
    { model: "gpt-5.6-sol-xhigh" }
  ],
  opencode: [{}, {}, {}, {}]
};
function hostKind(name) {
  const normalized = String(name).trim().toLowerCase();
  if (normalized === "claude" || normalized === "claude-code")
    return "claude";
  if (normalized === "codex")
    return "codex";
  if (normalized === "cursor" || normalized === "cursor-agent")
    return "cursor";
  if (normalized === "opencode")
    return "opencode";
  fail("config_invalid", `unknown host '${String(name)}'; use one of claude, codex, cursor, opencode`, {
    host: name
  });
}
function hostCli(host) {
  return host === "cursor" ? "cursor-agent" : host;
}
function hostList(value, where) {
  if (!Array.isArray(value))
    fail("config_invalid", `prBabysit: ${where} must be an array of hosts`);
  return [...new Set(value.map(hostKind))];
}
function block(path) {
  if (!existsSync(path))
    return {};
  let value;
  try {
    value = JSON.parse(readFileSync(path, "utf8"));
  } catch {
    process.stderr.write(`pr-babysit-config: malformed JSON in ${path}; ignoring
`);
    return {};
  }
  if (!value || typeof value !== "object" || Array.isArray(value) || !("prBabysit" in value))
    return {};
  const config = value.prBabysit;
  if (!config || typeof config !== "object" || Array.isArray(config))
    fail("config_invalid", `prBabysit must be an object (in ${path})`, { file: path });
  return config;
}
function merge(a, b) {
  const out = { ...a };
  for (const [key, value] of Object.entries(b)) {
    const old = out[key];
    out[key] = old && value && typeof old === "object" && typeof value === "object" && !Array.isArray(old) && !Array.isArray(value) ? merge(old, value) : value;
  }
  return out;
}
function opencodePaths(root) {
  const home = process.env.HOME || homedir();
  const user = process.env.TOOLU_USER_CONFIG_DIR || process.env.TOOLU_CONFIG_DIR || join(process.env.XDG_CONFIG_HOME || join(home, ".config"), "opencode");
  const dir = process.env.TOOLU_PROJECT_CONFIG_DIRNAME || ".opencode";
  return {
    user: join(user, "toolu.config.json"),
    project: root ? join(root, dir, "toolu.config.json") : ""
  };
}
function gitRoot() {
  return process.env.TOOLU_PROJECT_DIR || (spawnSync("git", ["rev-parse", "--show-toplevel"], { encoding: "utf8" }).stdout ?? "").trim();
}
function configPaths(host) {
  if (host === "opencode")
    return opencodePaths(gitRoot());
  const home = host === "codex" ? process.env.CODEX_HOME || join(process.env.HOME || homedir(), ".codex") : process.env.CLAUDE_CONFIG_DIR || join(process.env.HOME || homedir(), ".claude");
  const user = join(process.env.TOOLU_CONFIG_DIR || home, "toolu.config.json");
  const root = gitRoot();
  const dir = process.env.TOOLU_PROJECT_CONFIG_DIRNAME || (host === "codex" ? ".codex" : ".claude");
  return { user, project: root ? join(root, dir, "toolu.config.json") : "" };
}
function loadFixerConfig(host) {
  const paths = configPaths(host);
  const raw = merge(block(paths.user), paths.project ? block(paths.project) : {});
  const dispatch = raw.dispatch ?? "herdr";
  if (dispatch !== "herdr" && dispatch !== "inline")
    fail("config_invalid", "prBabysit: dispatch must be herdr or inline");
  const hosts = raw.hosts === undefined ? [host] : hostList(raw.hosts, "hosts");
  if (hosts.length === 0)
    fail("config_invalid", "prBabysit: hosts must name at least one host");
  const prefer = {};
  if (raw.prefer !== undefined) {
    if (!raw.prefer || typeof raw.prefer !== "object" || Array.isArray(raw.prefer))
      fail("config_invalid", "prBabysit: prefer must be an object");
    for (const [tier, list] of Object.entries(raw.prefer)) {
      if (!TIERS.includes(tier))
        fail("config_invalid", `prBabysit: prefer key ${tier} is not a tier`);
      prefer[tier] = hostList(list, `prefer.${tier}`);
    }
  }
  const routing = structuredClone(DEFAULTS);
  if (raw.routing !== undefined) {
    if (!raw.routing || typeof raw.routing !== "object" || Array.isArray(raw.routing))
      fail("config_invalid", "prBabysit: routing must be an object");
    for (const [name, row] of Object.entries(raw.routing)) {
      const kind = hostKind(name);
      if (!Array.isArray(row) || row.length !== 4)
        fail("config_invalid", `prBabysit: routing.${name} must list 4 tiers`);
      if (row.some((v) => !v || typeof v !== "object" || Array.isArray(v) || [v.model, v.effort].some((s) => s != null && (typeof s !== "string" || !SAFE.test(s)))))
        fail("config_invalid", `prBabysit: routing.${name} has a non-string or shell-unsafe model/effort`);
      routing[kind] = row;
    }
  }
  for (const key of ["unattended", "jev"])
    if (raw[key] !== undefined && typeof raw[key] !== "boolean")
      fail("config_invalid", `prBabysit: ${key} must be true or false`);
  return {
    dispatch,
    hosts,
    prefer,
    routing,
    unattended: raw.unattended !== false,
    jev: raw.jev !== false
  };
}
function commandAvailable(name) {
  return Bun.which(name, { PATH: process.env.PATH ?? "" }) !== null;
}
function readJsonIfValid(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}
function validateItems(value, raised) {
  const obj = value;
  const items = obj?.items;
  if (!Array.isArray(items) || items.length === 0)
    fail("plan_invalid", "items must be a non-empty array");
  if (items.some((i) => !i || typeof i !== "object" || Array.isArray(i) || typeof i.id !== "string" || !i.id))
    fail("plan_invalid", "every item needs an id");
  if (new Set(items.map((i) => i.id)).size !== items.length)
    fail("plan_invalid", "item ids must be unique");
  if (items.some((i) => typeof i.task !== "string" || !i.task))
    fail("plan_invalid", "every item needs a task");
  if (items.some((i) => !["thread", "conversation", "review", "ci"].includes(i.kind)))
    fail("plan_invalid", "item kind must be thread, conversation, review or ci");
  const round = obj?.round ?? 1;
  if (!Number.isInteger(round) || round < 1)
    fail("plan_invalid", "round must be a positive integer");
  if (raised.some((id) => !items.some((i) => i.id === id)))
    fail("plan_invalid", "--raise names an item that is not in the items file");
  return { round, items };
}
function heuristic(item) {
  const severity = (item.severity ?? "").toLowerCase();
  const task = item.task.toLowerCase();
  if (severity === "critical" || /\b(security|injection|race|concurrency|deadlock|data[- ]loss)\b/.test(task))
    return 3;
  if (severity === "high")
    return 2;
  if (["low", "nit", ""].includes(severity) && /\b(typos?|nits?|wording|renam(e|es|ed|ing)|spelling|whitespace|comments?|docs?|imports?)\b/.test(task))
    return 0;
  return 1;
}
function jevScript(host) {
  const home = process.env.HOME || homedir();
  const candidates = host === "opencode" ? [process.env.PB_JEV, process.env.TOOLU_CONFIG_DIR && join(process.env.TOOLU_CONFIG_DIR, "jev/jev.sh")] : [
    process.env.PB_JEV,
    join(process.env.CLAUDE_CONFIG_DIR || join(home, ".claude"), "jev/jev.sh"),
    join(process.env.CODEX_HOME || join(home, ".codex"), "jev/jev.sh")
  ];
  return candidates.find((path) => !!path && existsSync(path));
}
function jevAnswers(items, disabled, replay, host) {
  if (replay) {
    const value = readJsonIfValid(replay);
    if (value === null)
      fail("usage", `route-fix.js: --jev-answers-in is not JSON: ${replay}`);
    return { answers: value, note: "" };
  }
  if (disabled)
    return { answers: {}, note: "jev disabled; heuristic tiers used" };
  const script = jevScript(host);
  if (!script)
    return { answers: {}, note: "jev unavailable (jev.sh not installed); heuristic tiers used" };
  if (!process.env.TYPESAFE_API_KEY)
    return {
      answers: {},
      note: "jev unavailable (TYPESAFE_API_KEY not set); heuristic tiers used"
    };
  const state = {
    items: Object.fromEntries(items.map((i) => [
      i.id,
      { task: i.task, path: i.path ?? null, severity: i.severity ?? null, kind: i.kind }
    ]))
  };
  const criteria = [
    "Trivial: a typo, wording, rename, import, or one-line change with an obvious answer.",
    "Standard: a bounded edit in one function or file with a clear answer, possibly with its colocated test.",
    "Complex: spans several files, or needs new tests, concurrency handling, or behavior design.",
    "Critical: security or data-loss risk, cross-cutting architecture, or an ambiguous request that needs deep reasoning."
  ];
  const questions = Object.fromEntries(items.map((i) => [
    i.id,
    {
      type: "score",
      instructions: `How much implementation complexity does the review fix \`items["${i.id}"]\` carry for one coding agent? Judge from its task, path, severity and kind only.`,
      criteria
    }
  ]));
  const temp = mkdtempSync(join(tmpdir(), "pr-babysit-route-"));
  let run;
  try {
    const stateFile = join(temp, "state.json");
    writeFileSync(stateFile, JSON.stringify(state));
    const bunArgs = host === "opencode" ? ["--no-env-file"] : [];
    run = spawnSync(process.execPath, [...bunArgs, script, "ask", "-", "-s", `@${stateFile}`], {
      input: JSON.stringify(questions),
      encoding: "utf8"
    });
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
  if (run.status !== 0)
    return {
      answers: {},
      note: `jev failed (exit ${run.status ?? 1}): ${run.stderr.replace(/\s+$/, "").replace(/\n/g, " ").slice(0, 160)}; heuristic tiers used`
    };
  try {
    const value = JSON.parse(run.stdout);
    if (value && typeof value === "object" && !Array.isArray(value))
      return { answers: value, note: "" };
  } catch {}
  return { answers: {}, note: "jev returned an unparsable answer; heuristic tiers used" };
}
function routeFix(opts) {
  if (opts.host !== "claude" && opts.host !== "codex" && opts.host !== "opencode")
    fail("usage", "route-fix.js: --host claude|codex|opencode required");
  if (!opts.itemsFile)
    fail("usage", "route-fix.js: --items required");
  const raw = readJsonIfValid(opts.itemsFile);
  if (raw === null)
    fail("plan_invalid", `items file is missing or not JSON: ${opts.itemsFile}`);
  const items = validateItems(raw, opts.raise ?? []).items;
  const config = loadFixerConfig(opts.host);
  const now = Date.parse(opts.now || new Date().toISOString());
  if (!Number.isFinite(now))
    fail("plan_invalid", "--now must be ISO-8601");
  const state = opts.stateFile ? readJsonIfValid(opts.stateFile) : null;
  const cooldowns = state?.hostCooldowns ?? {};
  const missing = config.hosts.filter((h) => !commandAvailable(hostCli(h)));
  const cooling = Object.entries(cooldowns).filter(([, v]) => v?.until && Date.parse(v.until) > now).map(([h]) => h);
  const eligible = config.hosts.filter((h) => !missing.includes(h) && !cooling.includes(h));
  const { answers, note: jevNote } = jevAnswers(items, !!opts.noJev || !config.jev, opts.answersFile ?? "", opts.host);
  const scored = items.map((i) => {
    const answer = answers[i.id];
    const score = typeof answer?.score === "number" ? answer.score : null;
    const tier = score === null ? heuristic(i) : Math.max(0, Math.min(3, Math.round(score + 0.15)));
    const raised = (opts.raise ?? []).includes(i.id);
    return {
      id: i.id,
      tier: raised ? Math.min(3, tier + 1) : tier,
      score,
      confidence: score === null ? null : answer?.confidence ?? null,
      raised,
      source: score === null ? "heuristic" : "jev"
    };
  });
  const inlineWhy = config.dispatch === "inline" ? "prBabysit.dispatch is inline" : eligible.length ? null : `no fixer host available${cooling.length ? ` (cooling: ${cooling.join(", ")})` : ""}${missing.length ? ` (CLI not on PATH: ${missing.join(", ")})` : ""}`;
  const dispatch = inlineWhy ? "inline" : "herdr";
  const classes = ["mechanical", "implementation", "architecture", "architecture"];
  const groups = [3, 2, 1, 0].filter((tier) => scored.some((i) => i.tier === tier)).map((tier, index) => {
    const preferred = config.prefer[TIERS[tier]] ?? [];
    const host = dispatch === "inline" ? null : preferred.find((h) => eligible.includes(h)) ?? eligible[0] ?? null;
    const choice = host ? config.routing[host][tier] ?? {} : {};
    return {
      seq: index + 1,
      tier: TIERS[tier],
      class: classes[tier],
      host,
      model: choice.model ?? null,
      effort: choice.effort ?? null,
      items: scored.filter((i) => i.tier === tier).map((i) => i.id)
    };
  });
  const source = scored.every((i) => i.source === "jev") ? "jev" : scored.every((i) => i.source === "heuristic") ? "heuristic" : "mixed";
  const notes = [
    jevNote,
    inlineWhy,
    missing.length && eligible.length ? `dropped (CLI not on PATH): ${missing.join(", ")}` : "",
    cooling.length && eligible.length ? `cooling: ${cooling.join(", ")}` : ""
  ].filter(Boolean);
  return {
    version: 1,
    dispatch,
    unattended: config.unattended,
    source,
    note: notes.join("; ") || null,
    items: scored.map((i) => ({ ...i, tier: TIERS[i.tier] })),
    groups
  };
}

// plugins/pr-babysit/hooks/src/babysit-route-fix.ts
runCli(() => {
  const args = process.argv.slice(2);
  const raised = [];
  const withoutRaise = [];
  for (let i = 0;i < args.length; i += 1) {
    if (args[i] === "--raise")
      raised.push(args[++i] ?? "");
    else
      withoutRaise.push(args[i]);
  }
  const flags = parseFlags(withoutRaise, "route-fix.js", ["--items", "--host", "--state-file", "--jev-answers-in", "--now"], ["--no-jev"]);
  const host = flag(flags, "--host");
  const itemsFile = flag(flags, "--items");
  if (!host)
    fail("usage", "route-fix.js: --host claude|codex|opencode required");
  if (!itemsFile)
    fail("usage", "route-fix.js: --items required");
  return routeFix({
    itemsFile,
    host,
    stateFile: flag(flags, "--state-file"),
    raise: raised,
    noJev: flags["--no-jev"] === true,
    answersFile: flag(flags, "--jev-answers-in"),
    now: flag(flags, "--now")
  });
});
