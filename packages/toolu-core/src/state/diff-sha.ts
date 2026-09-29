/**
 * Content-addressed branch-diff hash (#255), a port of `toolu_diff_sha`: the
 * git blob id of `git diff --no-color BASE...HEAD`. It survives amend and
 * rebase because it hashes content, not commit ids. An empty diff yields the
 * empty-blob id; each caller decides what that means.
 */
import { childEnv, type HostEnv } from "../host/host-name.ts";

export type DiffShaOptions = { env?: HostEnv };

/** The hash, or undefined when either git step fails or prints nothing (bash: non-zero exit). */
export function diffSha(
  repoRoot: string,
  baseRef: string,
  options: DiffShaOptions = {},
): string | undefined {
  const env = childEnv(options.env ?? process.env);
  // `--end-of-options`: a ref starting with `-` stays a revision, never an option such as `--output=`.
  // Bun.spawnSync has no output cap, unlike node's spawnSync `maxBuffer`: a large diff must still hash.
  const diff = Bun.spawnSync(
    ["git", "-C", repoRoot, "diff", "--no-color", "--end-of-options", `${baseRef}...HEAD`],
    {
      env,
      stdout: "pipe",
      stderr: "ignore",
    },
  );
  if (!diff.success) return undefined;
  // Hash in the repo, so its object format (sha1 or sha256) decides the id.
  const hash = Bun.spawnSync(["git", "-C", repoRoot, "hash-object", "--stdin"], {
    env,
    stdin: diff.stdout,
    stdout: "pipe",
    stderr: "ignore",
  });
  if (!hash.success) return undefined;
  const sha = hash.stdout.toString("utf8").trim();
  return sha === "" ? undefined : sha;
}
