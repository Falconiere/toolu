// The OpenCode plugin shim: `toolu plugins install --host opencode` fills in its quoted
// placeholders and writes it as each plugin's server.js, under a "do not edit" line.
// It forwards OpenCode's hooks to `toolu opencode bridge`, which runs the plugin's hooks/hooks.json.
import { spawn, spawnSync } from "node:child_process";
import { accessSync, constants, statSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const PLUGIN = "__PLUGIN__";
const HOOKS = "__HOOKS__";
const CATALOG = "__CATALOG__";
const MISSING = "__MISSING__";
const ROOT = dirname(fileURLToPath(import.meta.url));
const ENFORCING = "tool.execute.before";

const executable = (path) => {
  try {
    accessSync(path, constants.X_OK);
    return statSync(path).isFile();
  } catch {
    return false;
  }
};

// The launcher's order: TOOLU_BIN alone when set, else the install directories, then PATH.
const candidates = () => {
  if (process.env.TOOLU_BIN) return [process.env.TOOLU_BIN];
  const path = (process.env.PATH ?? "")
    .split(delimiter)
    .filter(Boolean)
    .map((dir) => join(dir, "toolu"));
  const installed = ["/opt/homebrew/bin", "/usr/local/bin", "/home/linuxbrew/.linuxbrew/bin"];
  return [...installed.map((dir) => join(dir, "toolu")), join(homedir(), ".local/bin/toolu"), ...path];
};

// The first candidate whose `--hook-protocol` prints an integer.
const findToolu = () =>
  candidates().find((path) => {
    if (!executable(path)) return false;
    const probe = spawnSync(path, ["--hook-protocol"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      timeout: 10000,
    });
    return /^\d+\s*$/.test(probe.stdout ?? "");
  });

const run = (toolu, args, input) =>
  new Promise((resolve) => {
    const child = spawn(toolu, args, { stdio: ["pipe", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", (error) => resolve({ error: error.message, stdout, stderr }));
    child.on("close", (status) => resolve({ status, stdout, stderr }));
    child.stdin.on("error", () => {});
    child.stdin.end(input);
  });

// A failure refuses a tool call and is only reported anywhere else.
const failed = (hook, text) =>
  hook === ENFORCING ? { action: "deny", reason: text } : { action: "continue", message: text };

const bridge = async (toolu, hook, directory, event) => {
  if (!toolu) return failed(hook, MISSING);
  let input;
  try {
    input = JSON.stringify({ directory, event });
  } catch (error) {
    return failed(hook, `${PLUGIN} plugin: the ${hook} event is not JSON: ${error?.message ?? error}`);
  }
  const args = ["opencode", "bridge", "--event", hook, "--plugin-root", ROOT];
  const ran = await run(toolu, args, input);
  if (ran.status === 0) {
    try {
      const reply = JSON.parse(ran.stdout);
      if (reply && typeof reply.action === "string") return reply;
    } catch {
      // Reported below with the bridge's stderr.
    }
  }
  const status = ran.status === undefined ? "" : ` with status ${ran.status}`;
  const detail = (ran.error ?? ran.stderr ?? "").trim();
  return failed(hook, `${PLUGIN} plugin: toolu opencode bridge on ${hook} failed${status}${detail ? `: ${detail}` : ""}`);
};

const note = (reply) => {
  if (reply.message) console.warn(`[toolu ${PLUGIN}] ${reply.message}`);
};

// This plugin's skills and commands, read from its directory by `toolu opencode catalog`.
const register = async (ctx, toolu) => {
  if (!CATALOG || !toolu) return;
  const ran = await run(toolu, ["opencode", "catalog", "--plugin-root", ROOT], "");
  let catalog;
  try {
    catalog = JSON.parse(ran.stdout);
  } catch {
    console.warn(`[toolu ${PLUGIN}] toolu opencode catalog failed: ${(ran.error ?? ran.stderr ?? "").trim()}`);
    return;
  }
  const skills = catalog.skills ?? [];
  const commands = catalog.commands ?? [];
  if (skills.length) await ctx.skill.transform((editor) => skills.forEach((skill) => editor.add(skill)));
  if (!commands.length) return;
  await ctx.command.transform((editor) => {
    for (const command of commands) {
      editor.add({
        name: command.name,
        description: command.description,
        execute: async ({ sessionID, prompt, delivery }) => {
          const text = command.template.replaceAll("$ARGUMENTS", prompt.text ?? "");
          await ctx.session.prompt({ ...prompt, sessionID, text, delivery });
        },
      });
    }
  });
};

const hooks = async (ctx, toolu) => {
  const directory = ctx.location?.directory;
  const has = (hook) => HOOKS.includes(hook);
  const entries = new Set();
  // Session context lives in the session's instructions, replaced on every prompt.
  const keep = async (sessionID, key, text) => {
    const entry = { sessionID, key: `toolu.${PLUGIN}.${key}` };
    try {
      if (text) {
        await ctx.session.instructions.entry.put({ ...entry, value: text });
        entries.add(`${sessionID}/${key}`);
      } else if (entries.delete(`${sessionID}/${key}`)) {
        await ctx.session.instructions.entry.remove(entry);
      }
    } catch (error) {
      console.warn(`[toolu ${PLUGIN}] session context: ${error?.message ?? error}`);
    }
  };
  const started = new Map();
  const start = (sessionID) => {
    if (!has("session.start") || !sessionID) return Promise.resolve();
    if (!started.has(sessionID)) {
      const reply = bridge(toolu, "session.start", directory, { sessionID });
      started.set(
        sessionID,
        reply.then(async (answer) => {
          note(answer);
          await keep(sessionID, "session", answer.context);
        }),
      );
    }
    return started.get(sessionID);
  };
  if (has("session.start") || has("session.prompt")) {
    await ctx.session.hook("prompt", async (event) => {
      await start(event.sessionID);
      if (!has("session.prompt")) return;
      const reply = await bridge(toolu, "session.prompt", directory, event);
      note(reply);
      await keep(event.sessionID, "prompt", reply.context);
    });
  }
  if (has("session.compaction")) {
    await ctx.session.hook("compaction", async (event) => {
      note(await bridge(toolu, "session.compaction", directory, { sessionID: event.sessionID }));
    });
  }
  if (has("session.start") || has(ENFORCING)) {
    await ctx.tool.hook("execute.before", async (event) => {
      await start(event.sessionID);
      if (!has(ENFORCING)) return;
      const reply = await bridge(toolu, ENFORCING, directory, event);
      note(reply);
      if (reply.action === "deny") throw new Error(reply.reason ?? `${PLUGIN} plugin: denied`);
    });
  }
  if (has("tool.execute.after")) {
    await ctx.tool.hook("execute.after", async (event) => {
      const reply = await bridge(toolu, "tool.execute.after", directory, event);
      note(reply);
      const text = [reply.reason, reply.context].filter(Boolean).join("\n\n");
      if (!text || event.status !== "completed" || !event.result) return;
      const content = [...(event.result.content ?? []), { type: "text", text }];
      event.result = { ...event.result, content };
    });
  }
};

export default {
  id: `toolu.${PLUGIN}`,
  async setup(ctx) {
    let toolu;
    try {
      toolu = findToolu();
      await hooks(ctx, toolu);
    } catch (error) {
      // OpenCode runs tools without a plugin whose setup throws, so refuse them instead.
      const reason = `${PLUGIN} plugin: setup failed: ${error?.message ?? error}`;
      console.error(reason);
      if (HOOKS.includes(ENFORCING)) {
        await ctx.tool.hook("execute.before", () => {
          throw new Error(reason);
        });
      }
    }
    try {
      await register(ctx, toolu);
    } catch (error) {
      console.warn(`[toolu ${PLUGIN}] skills and commands: ${error?.message ?? error}`);
    }
  },
};
