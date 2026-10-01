import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join } from "node:path";

export type Json = Record<string, unknown>;

export class BabysitError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly extra: Json = {},
  ) {
    super(message);
  }
}

export function exitCode(code: string): number {
  if (code === "usage") return 2;
  if (code === "duplicate_reply") return 4;
  if (code === "resolve_unconfirmed") return 5;
  if (code === "locked") return 75;
  return 3;
}

export function errorDocument(error: BabysitError): Json {
  return { version: 1, errors: [{ code: error.code, message: error.message, ...error.extra }] };
}

export function fail(code: string, message: string, extra: Json = {}): never {
  throw new BabysitError(code, message, extra);
}

export function runCli(action: () => Promise<unknown> | unknown): void {
  Promise.resolve()
    .then(action)
    .then((result) => {
      if (result !== undefined) process.stdout.write(`${JSON.stringify(result)}\n`);
    })
    .catch((error: unknown) => {
      if (error instanceof BabysitError) {
        process.stdout.write(`${JSON.stringify(errorDocument(error))}\n`);
        process.exitCode = exitCode(error.code);
      } else {
        const message = error instanceof Error ? error.message : String(error);
        process.stdout.write(
          `${JSON.stringify(errorDocument(new BabysitError("api_error", message)))}\n`,
        );
        process.exitCode = 3;
      }
    });
}

export function parseFlags(
  argv: string[],
  command: string,
  valued: readonly string[],
  bare: readonly string[] = [],
): Record<string, string | boolean> {
  const flags: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i += 1) {
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

export function flag(flags: Record<string, string | boolean>, key: string): string {
  const value = flags[key];
  return typeof value === "string" ? value : "";
}

export function utcNow(): string {
  return new Date().toISOString().slice(0, 19) + "Z";
}

export function readJson(path: string): unknown {
  return JSON.parse(readFileSync(path, "utf8"));
}

/** Write a complete JSON document through a sibling temp, leaving the old file intact on failure. */
export function atomicWriteJson(path: string, value: unknown, raw?: string): void {
  const content = raw ?? `${JSON.stringify(value)}\n`;
  JSON.parse(content);
  mkdirSync(dirname(path), { recursive: true });
  const dir = mkdtempSync(join(dirname(path), `.${basename(path)}.tmp.${process.pid}.`));
  const temp = join(dir, "value");
  try {
    writeFileSync(temp, content);
    renameSync(temp, path);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function readTextOrEmpty(path: string): string {
  try {
    return readFileSync(path, "utf8").trim();
  } catch {
    return "";
  }
}

function pidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function errorCode(error: unknown): string | undefined {
  return error && typeof error === "object" && "code" in error && typeof error.code === "string"
    ? error.code
    : undefined;
}

export class SlotLock {
  readonly path: string;
  private held = false;
  private identity: { dev: number; ino: number } | null = null;
  private onTerm = (): void => {
    this.release();
    process.exit(143);
  };
  private onInt = (): void => {
    this.release();
    process.exit(130);
  };

  constructor(statePath: string) {
    this.path = `${statePath}.lock`;
  }

  acquire(): void {
    process.on("SIGTERM", this.onTerm);
    process.on("SIGINT", this.onInt);
    try {
      mkdirSync(dirname(this.path), { recursive: true });
      for (let attempt = 0; attempt < 2; attempt += 1) {
        let created = false;
        try {
          mkdirSync(this.path);
          created = true;
        } catch (error) {
          if (errorCode(error) !== "EEXIST") throw error;
          // The holder can release between EEXIST and inspection.
          if (!existsSync(this.path)) {
            if (attempt === 0) continue;
            fail("locked", "slot is held by another controller", { pid: null, since: null });
          }
        }
        if (created) {
          this.held = true;
          try {
            const { dev, ino } = statSync(this.path);
            this.identity = { dev, ino };
            writeFileSync(join(this.path, "pid"), `${process.pid}\n`);
            writeFileSync(join(this.path, "since"), `${Math.floor(Date.now() / 1000)}\n`);
            return;
          } catch (error) {
            // A removed or replaced directory is contention, never a lock we can delete blindly.
            if (errorCode(error) === "ENOENT")
              fail("locked", "slot is held by another controller", { pid: null, since: null });
            throw error;
          }
        }
        const pidText = readTextOrEmpty(join(this.path, "pid"));
        const sinceText = readTextOrEmpty(join(this.path, "since"));
        const pid = /^\d+$/.test(pidText) ? Number(pidText) : null;
        const since = /^\d+$/.test(sinceText) ? Number(sinceText) : null;
        const staleAfter = Number(process.env.PB_LOCK_STALE_SECONDS ?? "600");
        let incompleteAgeMs = 0;
        if (pid === null || since === null) {
          try {
            incompleteAgeMs = Date.now() - statSync(this.path).mtimeMs;
          } catch {
            if (attempt === 0) continue;
          }
        }
        const stale =
          pid === null || since === null
            ? // A new holder needs a moment to write pid and since after atomic mkdir.
              incompleteAgeMs >= 1000
            : !pidAlive(pid) || Math.floor(Date.now() / 1000) - since > staleAfter;
        if (attempt === 0 && stale) {
          process.stderr.write(
            `pr-babysit: reclaiming stale lock ${this.path} (pid ${pidText || "?"}, since ${sinceText || "?"})\n`,
          );
          rmSync(this.path, { recursive: true, force: true });
          continue;
        }
        fail("locked", "slot is held by another controller", { pid, since });
      }
    } catch (error) {
      this.release();
      if (error instanceof BabysitError) throw error;
      const message = error instanceof Error ? error.message : String(error);
      fail("api_error", `slot lock failed: ${message}`, { source: "lock" });
    }
  }

  release(): void {
    let owned = false;
    if (this.held && this.identity) {
      try {
        const { dev, ino } = statSync(this.path);
        owned = dev === this.identity.dev && ino === this.identity.ino;
      } catch {
        // The directory was already removed; never delete a replacement.
      }
    }
    if (owned) {
      rmSync(this.path, { recursive: true, force: true });
    }
    this.held = false;
    this.identity = null;
    process.off("SIGTERM", this.onTerm);
    process.off("SIGINT", this.onInt);
  }
}

export function loadState(path: string): Json {
  if (!existsSync(path))
    fail("state_malformed", `state file not found: ${path} (run babysit-tick.js first)`, {
      source: "state",
    });
  let value: unknown;
  try {
    value = readJson(path);
  } catch {
    fail("state_malformed", `state file is not valid JSON: ${path}`, { source: "state" });
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    fail("state_malformed", `state file is not a version-2 pr-babysit state: ${path}`, {
      source: "state",
      version: null,
    });
  }
  const state = value as Json;
  if (state.version !== 2 || typeof state.repo !== "string" || typeof state.number !== "number") {
    fail("state_malformed", `state file is not a version-2 pr-babysit state: ${path}`, {
      source: "state",
      version: state.version ?? null,
    });
  }
  if (!/^[A-Za-z0-9][A-Za-z0-9_-]*\/[A-Za-z0-9_][A-Za-z0-9._-]*$/.test(state.repo as string)) {
    fail("state_malformed", `state file repo is not owner/name: ${state.repo}`, {
      source: "state",
    });
  }
  if (!/^\d+$/.test(String(state.number))) {
    fail("state_malformed", `state file number is not an integer: ${state.number}`, {
      source: "state",
    });
  }
  return state;
}
