/** Pick the tracker for an epic reference. URLs and `jira:`/`linear:`
 * prefixes are unambiguous; a bare `ABC-12` key goes to whichever of Jira
 * or Linear is configured, and asks for a prefix when both are. */

import { GitHubTracker } from "./github.ts";
import { JiraTracker, jiraConfigured, parseJiraRef } from "./jira.ts";
import { LinearTracker, parseLinearRef } from "./linear.ts";
import type { Tracker, TrackerKind } from "./types.ts";

const GITHUB = /github\.com\/|^[\w.-]+\/[\w.-]+#\d+$|^#?\d+$/;

export function detectTracker(
  ref: string,
  hint?: string,
  env: NodeJS.ProcessEnv = process.env,
): TrackerKind {
  if (hint === "github" || hint === "jira" || hint === "linear") return hint;
  if (hint) throw new Error(`unknown tracker ${hint}; use github, jira, or linear`);
  const t = ref.trim();
  if (/^jira:/i.test(t) || /atlassian\.net\/browse\//i.test(t)) return "jira";
  if (/^linear:/i.test(t) || /linear\.app\//i.test(t)) return "linear";
  if (GITHUB.test(t)) return "github";
  if (parseJiraRef(t) || parseLinearRef(t)) {
    const jira = jiraConfigured(env);
    const linear = !!env.LINEAR_API_KEY;
    if (jira && !linear) return "jira";
    if (linear && !jira) return "linear";
    throw new Error(
      `${t} could be a Jira or Linear key; prefix it (jira:${t} or linear:${t}) or pass --tracker`,
    );
  }
  throw new Error(`unrecognized epic reference: ${JSON.stringify(ref)}`);
}

/** `defaultRepo` is the GitHub repo that holds the code for Jira/Linear
 * children without a `repo:` label or `Repo:` line. */
export function makeTracker(kind: TrackerKind, ref: string, defaultRepo: string): Tracker {
  if (kind === "jira") return new JiraTracker(ref, defaultRepo);
  if (kind === "linear") return new LinearTracker(ref, defaultRepo);
  return new GitHubTracker(ref);
}
