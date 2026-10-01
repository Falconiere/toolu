/** Bootstrap selected plugins from committed Bun bundles. */
import { mkdirSync } from "node:fs";
import { opencodeDataRoot } from "../host/roots.ts";
import type { PluginManifest } from "../inventory/types.ts";
import { resolveBunExecutable } from "../preflight/check.ts";
import { notReady, ready, type BootstrapResult } from "./result.ts";
import { pluginBootstrapScript, requiresNativeRegistration } from "./entrypoint.ts";
import { evaluateBootstrapReadiness } from "./readiness.ts";

export type BootstrapRuntimeOptions = {
  repoRoot: string;
  dataRoot?: string;
  projectRoot: string;
  plugins: PluginManifest[];
  env?: Record<string, string>;
  isolatedHome?: string;
  deadlineMs?: number;
};

const MAX_OUTPUT_BYTES = 512_000;

async function readBoundedOutput(stream: ReadableStream<Uint8Array>): Promise<string> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let bytes = 0;
  let output = "";
  async function readNext(): Promise<string> {
    const { done, value } = await reader.read();
    if (done) return output + decoder.decode();
    bytes += value.byteLength;
    if (bytes > MAX_OUTPUT_BYTES) throw new Error("startup output exceeded 512000 bytes");
    output += decoder.decode(value, { stream: true });
    return readNext();
  }
  try {
    return await readNext();
  } finally {
    reader.releaseLock();
  }
}

function bootstrapEnv(options: BootstrapRuntimeOptions, dataRoot: string): Record<string, string> {
  const inherited: Record<string, string> = {};
  for (const [key, value] of Object.entries(process.env)) {
    if (value !== undefined) inherited[key] = value;
  }
  return {
    ...inherited,
    ...options.env,
    TOOLU_CONFIG_DIR: dataRoot,
    TOOLU_PROJECT_DIR: options.projectRoot,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    TOOLU_HOST_OVERRIDE: "opencode",
    HOME: options.isolatedHome ?? dataRoot,
  };
}

async function runEntrypoint(
  bun: string,
  script: string,
  pluginDir: string,
  cwd: string,
  env: Record<string, string>,
  deadlineMs: number,
): Promise<BootstrapResult | null> {
  try {
    const proc = Bun.spawn([bun, script], {
      cwd,
      env: { ...env, CLAUDE_PLUGIN_ROOT: pluginDir },
      stdin: new Blob(["{}"]),
      stdout: "pipe",
      stderr: "pipe",
    });
    const timeoutState = { hit: false };
    const timer = setTimeout(() => {
      timeoutState.hit = true;
      proc.kill();
    }, deadlineMs);
    try {
      const [stdout, stderr, exitCode] = await Promise.all([
        readBoundedOutput(proc.stdout),
        readBoundedOutput(proc.stderr),
        proc.exited,
      ]);
      if (timeoutState.hit) return notReady(`bootstrap bundle timed out (${script})`);
      if (exitCode !== 0) {
        return notReady(`bootstrap bundle ${script} exited ${exitCode}: ${stderr || stdout}`);
      }
      return null;
    } catch (error) {
      proc.kill();
      throw error;
    } finally {
      clearTimeout(timer);
    }
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return notReady(`bootstrap bundle failed (${script}): ${reason}`);
  }
}

/** Run each selected plugin's bundle and verify tangible registry/session output. */
async function bootstrapRuntimeInternal(
  options: BootstrapRuntimeOptions,
): Promise<BootstrapResult> {
  const dataRoot = options.dataRoot ?? opencodeDataRoot({ projectRoot: options.projectRoot });
  mkdirSync(dataRoot, { recursive: true });
  const env = bootstrapEnv(options, dataRoot);
  // The child HOME is isolated; the runtime must come from the original host HOME.
  const bun = resolveBunExecutable({ ...env, HOME: options.env?.HOME ?? process.env.HOME ?? "" });
  if (!bun) return notReady("Bun runtime not found, checked TOOLU_BUN, PATH and ~/.bun/bin/bun");
  const deadlineMs = options.deadlineMs ?? 120_000;
  const runFrom = async (index: number): Promise<BootstrapResult | null> => {
    const plugin = options.plugins[index];
    if (!plugin) return null;
    if (requiresNativeRegistration(plugin.pluginDir)) {
      return notReady(`selected plugin ${plugin.name} has no native startup bundle`);
    }
    const script = pluginBootstrapScript(plugin.pluginDir);
    if (!script) return runFrom(index + 1);
    const failure = await runEntrypoint(
      bun,
      script,
      plugin.pluginDir,
      options.projectRoot,
      env,
      deadlineMs,
    );
    if (failure) return failure;
    return runFrom(index + 1);
  };
  const failure = await runFrom(0);
  if (failure) return failure;
  const proof = evaluateBootstrapReadiness(dataRoot);
  return proof.ready
    ? ready(proof.artifacts)
    : notReady(proof.reason ?? "bootstrap artifacts missing");
}

/** All startup failures become NotReady so permission.evaluate can deny. */
export async function bootstrapRuntime(options: BootstrapRuntimeOptions): Promise<BootstrapResult> {
  try {
    return await bootstrapRuntimeInternal(options);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return notReady(`bootstrap failed: ${reason}`);
  }
}
