/** Bounded gh subprocess transport shared by collector and thread writes. */
export type GhRunOptions = {
  timeoutSeconds?: number;
  attempts?: number;
  backoffSeconds?: number[];
};

export class GhError extends Error {
  constructor(
    readonly attempts: number,
    readonly classification: "transient" | "permanent" | "invalid_json",
    readonly lastMessage: string,
    readonly lastRc: number,
  ) {
    super(lastMessage || `rc ${lastRc}`);
  }
}

export function ghClassify(
  rc: number,
  stderr: string,
  stdout: string,
): "ok" | "transient" | "permanent" {
  if (rc === 0) return "ok";
  if (rc === 124) return "transient";
  const combined = `${stderr}\n${stdout}`;
  const fromStderr = /\(HTTP (\d{3})\)/.exec(combined)?.[1];
  let fromBody: string | undefined;
  try {
    const body: unknown = JSON.parse(stdout);
    if (body && typeof body === "object" && !Array.isArray(body)) {
      const status = (body as Record<string, unknown>).status;
      if (typeof status === "string" || typeof status === "number") fromBody = String(status);
    }
  } catch {
    /* the stderr/status classification remains authoritative */
  }
  const status = fromStderr ?? fromBody;
  if (status?.startsWith("5") || status === "429") return "transient";
  if (status === "403") return /rate limit/i.test(combined) ? "transient" : "permanent";
  if (status?.startsWith("4")) return "permanent";
  if (
    /connection refused|connection reset|no such host|i\/o timeout|TLS handshake|unexpected EOF|EOF$|network is unreachable|temporary failure|timed out/i.test(
      combined,
    )
  )
    return "transient";
  return "permanent";
}

export async function ghRun(args: string[], options: GhRunOptions = {}): Promise<string> {
  const timeoutSeconds = options.timeoutSeconds ?? Number(process.env.PB_GH_TIMEOUT ?? 60);
  const attempts = options.attempts ?? Number(process.env.PB_GH_ATTEMPTS ?? 3);
  const backoffSeconds =
    options.backoffSeconds ?? (process.env.PB_GH_BACKOFF ?? "2 4 8").split(/\s+/).map(Number);
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe", env: process.env });
    let timer: ReturnType<typeof setTimeout> | undefined;
    const timedOut = await Promise.race([
      proc.exited.then(() => false),
      new Promise<boolean>((resolve) => {
        timer = setTimeout(() => {
          proc.kill();
          resolve(true);
        }, timeoutSeconds * 1000);
      }),
    ]);
    if (timer) clearTimeout(timer);
    const [stdout, stderr] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
    ]);
    const rc = timedOut ? 124 : await proc.exited;
    const classification = ghClassify(rc, stderr, stdout);
    if (classification === "ok") return stdout;
    const lastMessage = stderr.split(/\r?\n/)[0] ?? "";
    if (classification === "permanent" || attempt === attempts)
      throw new GhError(attempt, classification, lastMessage, rc);
    const delay = backoffSeconds[attempt - 1] ?? 2;
    process.stderr.write(
      `pr-babysit: gh ${args[0]} attempt ${attempt} failed (${classification}: ${lastMessage || `rc ${rc}`}); retrying in ${delay}s\n`,
    );
    if (delay > 0) await Bun.sleep(delay * 1000);
  }
  throw new GhError(0, "permanent", "gh was not attempted", 1);
}

/** Match pb_gh_json_ok: a JSON object carrying GraphQL errors[] is invalid. */
export function ghJson(stdout: string): unknown {
  const parsed: unknown = JSON.parse(stdout);
  if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
    const errors = (parsed as Record<string, unknown>).errors;
    if (Array.isArray(errors) && errors.length > 0)
      throw new Error("response is not valid JSON or carries GraphQL errors[]");
  }
  return parsed;
}
