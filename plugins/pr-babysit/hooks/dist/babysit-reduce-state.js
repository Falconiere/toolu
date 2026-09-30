#!/usr/bin/env bun
// @bun

// plugins/pr-babysit/hooks/src/babysit-reduce-state.ts
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync
} from "fs";
import { basename, dirname, join } from "path";

// plugins/pr-babysit/hooks/src/babysit/reduce.ts
var ciReviewers = new Set(["github-actions", "github-actions[bot]", "claude", "claude[bot]"]);
var injectionPatterns = [
  "ignore (all |any |the )?(previous|prior|above|earlier) (instructions|prompts?|rules)",
  "disregard (all |any |the |your )?(previous|prior|system|above) ",
  "you are (now )?(an? )?(ai|assistant|llm|language model|claude|codex|copilot)",
  "(^|\\n)\\s*(system|assistant)\\s*:",
  "<(system|instructions?)>",
  "(run|execute) (the following|this|these) (command|shell|script)"
].map((pattern) => new RegExp(pattern, "i"));
var nil = (value, fallback) => value === null || value === undefined || value === false ? fallback : value;
var asArray = (value) => Array.isArray(value) ? value : [];
var jqString = (value) => typeof value === "string" ? value : JSON.stringify(value);
var reason = (code, detail) => ({ code, detail });
function checkState(check) {
  if (check.__typename === "StatusContext") {
    return check.state === "SUCCESS" ? "pass" : check.state === "PENDING" || check.state === "EXPECTED" ? "pending" : "fail";
  }
  return check.status !== "COMPLETED" ? "pending" : ["SUCCESS", "NEUTRAL", "SKIPPED"].includes(check.conclusion) ? "pass" : "fail";
}
function ciStatus(checks) {
  if (checks.length === 0)
    return "pending";
  const states = checks.map((check) => check.status);
  return states.includes("fail") ? "fail" : states.includes("pending") ? "pending" : "pass";
}
function classifiedThread(thread, author, actions) {
  const comments = asArray(thread.comments);
  const lastComment = comments.at(-1) ?? null;
  const nonAuthorComments = comments.filter((comment) => comment.author !== author);
  const lastNonAuthor = nonAuthorComments.at(-1) ?? null;
  const flagged = Object.keys(actions.flagged).includes(thread.id);
  const authorClass = lastNonAuthor === null ? "none" : ciReviewers.has(lastNonAuthor.author) ? "ci_reviewer" : lastNonAuthor.authorType === "Bot" ? "bot" : "human";
  const open = thread.isResolved === false;
  const answerable = open && !flagged && lastComment !== null && lastComment.author !== author && (authorClass === "human" || authorClass === "ci_reviewer");
  const actionable = thread.isOutdated ? answerable && authorClass === "human" : answerable;
  const audited = open && !thread.isOutdated && !flagged;
  const injectionPattern = injectionPatterns.find((pattern) => nonAuthorComments.some((comment) => pattern.test(nil(comment.body, ""))))?.source ?? null;
  return {
    id: thread.id,
    path: thread.path,
    line: thread.line,
    isOutdated: thread.isOutdated,
    isResolved: thread.isResolved,
    rootCommentId: nil(comments[0]?.databaseId, null),
    inReplyTo: nil(lastNonAuthor?.databaseId, null),
    authorClass,
    lastCommentAuthor: nil(lastComment?.author, null),
    lastCommentAt: nil(lastComment?.createdAt, null),
    injectionSuspect: injectionPattern !== null,
    injectionPattern,
    comments,
    flags: {
      actionable,
      audited,
      flagged,
      skippedOutdated: open && thread.isOutdated && authorClass === "ci_reviewer",
      replied: Object.keys(actions.replied).includes(`thread:${thread.id}@${jqString(nil(lastNonAuthor?.databaseId, 0))}`)
    }
  };
}
function reduceState(snapshot, previous, now, statePath, snapshotPath) {
  const snap = snapshot;
  const prev = previous;
  const pr = snap.pr;
  const head = snap.head.sha;
  const author = pr.author;
  const slot = `${snap.repo.toLowerCase().replaceAll("/", "-")}-${snap.number}`;
  const key = `${snap.repo}#${snap.number}`;
  const actions = prev === null ? { replied: {}, resolved: {}, flagged: {} } : nil(prev.actions, { replied: {}, resolved: {}, flagged: {} });
  const rollup = asArray(pr.statusCheckRollup);
  const checks = rollup.map((check) => ({
    name: nil(check.name, nil(check.context, "unknown")),
    status: checkState(check),
    url: nil(check.detailsUrl, nil(check.targetUrl, null))
  }));
  const ci = ciStatus(checks);
  const verdict = nil(snap.bot?.verdict, {});
  const botComment = snap.bot?.comment ?? null;
  const botState = nil(verdict.state, "absent");
  const botVerdict = nil(verdict.verdict, "none");
  const findings = asArray(verdict.findings);
  const keys = findings.map((finding) => finding.key);
  const findingsCount = findings.length;
  const degraded = botState === "absent" || botState === "unknown" || verdict.is_review_comment === false;
  const degradedReason = botState === "absent" ? "review_absent" : degraded ? "review_unknown_format" : null;
  const sameRun = prev !== null && botComment !== null && nil(prev.pr?.botCommentId, null) === botComment.id && nil(prev.pr?.botCommentUpdatedAt, null) === botComment.updatedAt;
  const threads = asArray(snap.threads).map((thread) => classifiedThread(thread, author, actions));
  const fixer = prev === null ? null : nil(prev.fixer, null);
  const fixerActive = fixer !== null && (fixer.status === "running" || fixer.status === "blocked");
  const fixingIds = fixerActive ? asArray(fixer.items).map(jqString) : [];
  const owned = (item) => fixingIds.includes(jqString(item.id));
  const allActionable = threads.filter((thread) => thread.flags.actionable).map(({ flags: _flags, isResolved: _resolved, ...rest }) => rest);
  const actionable = allActionable.filter((item) => !owned(item));
  const fixing = allActionable.filter(owned);
  const audited = threads.filter((thread) => thread.flags.audited);
  const staleUnresolved = audited.filter((thread) => !thread.flags.actionable).map((thread) => ({
    id: thread.id,
    path: thread.path,
    line: thread.line,
    repliedAt: thread.lastCommentAt,
    lastCommentAuthor: thread.lastCommentAuthor
  }));
  const skippedOutdated = threads.filter((thread) => thread.flags.skippedOutdated).map((thread) => thread.id);
  const flaggedInjection = threads.filter((thread) => thread.flags.flagged).map((thread) => thread.id);
  const unresolved = audited.length;
  const comments = asArray(snap.comments);
  const repliedKeys = Object.keys(actions.replied);
  const allConvActionable = comments.filter((comment) => comment.author !== author && comment.authorType !== "Bot").filter((comment) => !comments.some((later) => later.author === author && later.createdAt > comment.createdAt)).filter((comment) => !repliedKeys.includes(`conversation:${jqString(comment.id)}`)).map(({ id, author, body, createdAt, url }) => ({ id, author, body, createdAt, url }));
  const convActionable = allConvActionable.filter((item) => !owned(item));
  const convFixing = allConvActionable.filter(owned);
  const allReviewActionable = asArray(snap.reviews).filter((review) => review.author !== author && review.authorType !== "Bot" && review.state !== "APPROVED" && nil(review.body, "").length > 0).filter((review) => !repliedKeys.includes(`review:${jqString(review.id)}`)).map(({ id, author, state, body, submittedAt, url }) => ({
    id,
    author,
    state,
    body,
    submittedAt,
    url
  }));
  const reviewActionable = allReviewActionable.filter((item) => !owned(item));
  const reviewFixing = allReviewActionable.filter(owned);
  const lastRoundKeys = prev === null ? [] : asArray(prev.pr?.lastRoundFindingKeys);
  const lastRoundHadRejection = prev === null ? false : nil(prev.pr?.lastRoundHadRejection, false);
  const prevStreak = prev === null ? 0 : nil(prev.pr?.recurrenceStreak, 0);
  const fixAttempts = prev === null ? 0 : nil(prev.pr?.fixAttempts, 0);
  const recurringKeys = sameRun || prev === null ? [] : keys.filter((findingKey) => lastRoundKeys.includes(findingKey));
  const streak = sameRun ? prevStreak : recurringKeys.length > 0 ? prevStreak + 1 : 0;
  const comparison = {
    ciStatus: ci,
    reviewDecision: pr.reviewDecision,
    mergeable: pr.mergeable,
    unresolvedThreads: unresolved,
    headSha: head,
    botVerdict,
    botState,
    botFindingKeys: keys
  };
  const previousComparison = prev?.pr && {
    ciStatus: prev.pr.ciStatus,
    reviewDecision: prev.pr.reviewDecision,
    mergeable: prev.pr.mergeable,
    unresolvedThreads: prev.pr.unresolvedThreads,
    headSha: prev.pr.headSha,
    botVerdict: prev.pr.botVerdict,
    botState: prev.pr.botState,
    botFindingKeys: prev.pr.botFindingKeys
  };
  const changed = prev === null || JSON.stringify(previousComparison) !== JSON.stringify(comparison);
  const idleStreak = changed || fixerActive && fixer.status === "running" ? 0 : nil(prev?.idleStreak, 0) + 1;
  const intervalMinutes = idleStreak >= 9 ? 15 : idleStreak >= 6 ? 12 : idleStreak >= 3 ? 6 : ci === "fail" ? 1 : 3;
  const waitSeconds = idleStreak >= 6 ? 60 : idleStreak >= 3 ? 30 : 15;
  const providerErrorRepeated = prev !== null && botState === "provider_error" && nil(prev.pr?.botState, "") === "provider_error" && nil(prev.pr?.headSha, "") === head;
  const escalations = [];
  if (pr.state === "MERGED")
    escalations.push(reason("pr_merged", "PR is merged"));
  if (pr.state === "CLOSED")
    escalations.push(reason("pr_closed", "PR is closed"));
  if (pr.mergeable === "CONFLICTING")
    escalations.push(reason("merge_conflict", "mergeable is CONFLICTING"));
  if (fixAttempts >= 5)
    escalations.push(reason("fix_attempts_exhausted", `${fixAttempts} fix attempts recorded`));
  if (recurringKeys.length > 0 && lastRoundHadRejection)
    escalations.push(reason("recurrence_after_rejection", `${recurringKeys.length} finding key(s) recurred after a Won't-fix round`));
  if (recurringKeys.length > 0 && streak >= 2)
    escalations.push(reason("recurrence_streak", `finding keys recurred on ${streak} consecutive rounds`));
  if (providerErrorRepeated)
    escalations.push(reason("provider_error_repeated", `review provider error twice on head ${head.slice(0, 8)}`));
  const signals = [];
  if (ci === "pass")
    signals.push(reason("ci_pass", `${checks.length} check(s) passed`));
  else if (ci === "fail")
    signals.push(reason("ci_failed", checks.filter((check) => check.status === "fail").map((check) => check.name).join(", ")));
  else
    signals.push(reason("ci_pending", checks.length === 0 ? "no checks reported yet" : checks.filter((check) => check.status === "pending").map((check) => check.name).join(", ")));
  if (unresolved === 0)
    signals.push(reason("threads_clear", "no unresolved review threads"));
  if (actionable.length > 0)
    signals.push(reason("threads_unresolved", `${actionable.length} actionable thread(s)`));
  if (staleUnresolved.length > 0)
    signals.push(reason("threads_stale_unresolved", `${staleUnresolved.length} replied-but-unresolved thread(s)`));
  if (botState === "in_progress")
    signals.push(reason("review_in_progress", "review bot still running"));
  else if (botState === "provider_error" && !providerErrorRepeated)
    signals.push(reason("provider_error", "review bot reported a provider error; rerun the review job once"));
  else if (botState === "complete" && botVerdict === "approved" && findingsCount === 0)
    signals.push(reason("review_approved", "bot verdict approved with zero findings"));
  else if (botState === "complete")
    signals.push(reason("review_changes", `bot verdict ${botVerdict} with ${findingsCount} finding(s)`));
  else if (degraded)
    signals.push(reason(degradedReason, "bot verdict cannot be read"));
  if (degraded)
    signals.push(reason("manual_verify", `verify review findings manually: ${nil(botComment?.url, "no bot comment")}`));
  if (pr.mergeable === "UNKNOWN" && pr.state === "OPEN")
    signals.push(reason("mergeable_unknown", "GitHub has not computed mergeability yet"));
  if (fixerActive)
    signals.push(reason("fixer_running", `fixer ${fixer.status}: group ${nil(fixer.current, 1)} of ${asArray(fixer.groups).length}; ${fixingIds.length} item(s) in flight`));
  if (!changed)
    signals.push(reason("unchanged", "nothing changed since the last tick"));
  const successReady = pr.state === "OPEN" && ci === "pass" && unresolved === 0 && pr.mergeable !== "UNKNOWN" && !fixerActive && (botState === "complete" && botVerdict === "approved" && findingsCount === 0 || degraded);
  const decision = escalations.length > 0 ? "escalate" : successReady ? "success" : "keep_going";
  return {
    state: {
      version: 2,
      slot,
      repo: snap.repo,
      number: snap.number,
      cronName: `pr-babysit:${slot}`,
      lastUpdate: now,
      totalTicks: nil(prev?.totalTicks, 0) + 1,
      idleStreak,
      currentInterval: intervalMinutes,
      waitSeconds,
      status: nil(prev?.status, "active"),
      worktree: nil(prev?.worktree, null),
      fixer,
      herdrWorktree: nil(prev?.herdrWorktree, null),
      hostCooldowns: nil(prev?.hostCooldowns, {}),
      pr: {
        key,
        ciStatus: ci,
        reviewDecision: pr.reviewDecision,
        mergeable: pr.mergeable,
        unresolvedThreads: unresolved,
        headSha: head,
        fixAttempts,
        botVerdict,
        botState,
        botCommentId: nil(botComment?.id, null),
        botCommentUpdatedAt: nil(botComment?.updatedAt, null),
        botFindingKeys: keys,
        lastRoundFindingKeys: lastRoundKeys,
        lastRoundHadRejection,
        recurrenceStreak: streak,
        unresolvedAfterClearance: unresolved,
        lastError: null
      },
      actions,
      lastGoodSnapshot: snapshotPath
    },
    result: {
      version: 1,
      slot,
      changed,
      decision,
      reasons: [...escalations, ...signals],
      pr: {
        number: pr.number,
        url: pr.url,
        head,
        branch: pr.headRefName,
        base: pr.baseRefName,
        author,
        state: pr.state,
        mergeable: pr.mergeable,
        reviewDecision: pr.reviewDecision
      },
      ci: { status: ci, checks },
      verdict: {
        state: botState,
        verdict: botVerdict,
        findingsCount,
        findingKeys: keys,
        mustFix: nil(verdict.must_fix, []),
        commentUrl: nil(botComment?.url, null),
        commentId: nil(botComment?.id, null),
        degraded,
        degradedReason,
        sameRunAsLastTick: sameRun
      },
      threads: {
        total: snap.threads.length,
        unresolved,
        actionable,
        fixing,
        staleUnresolved,
        skippedOutdated,
        flaggedInjection
      },
      conversation: { actionable: convActionable, fixing: convFixing },
      reviews: { actionable: reviewActionable, fixing: reviewFixing },
      fixer,
      recurrence: { streak, lastRoundHadRejection, recurringKeys, fixAttempts },
      backoff: { idleStreak, intervalMinutes, waitSeconds },
      errors: [],
      snapshotPath,
      statePath
    }
  };
}

// plugins/pr-babysit/hooks/src/babysit-reduce-state.ts
function failure(code, message, extra = {}) {
  process.stdout.write(`${JSON.stringify({ version: 1, errors: [{ code, message, ...extra }] })}
`);
  process.exit(code === "usage" ? 2 : 3);
}
function atomicJson(path, value) {
  const dir = dirname(path);
  mkdirSync(dir, { recursive: true });
  const temp = join(dir, `.${basename(path)}.tmp.${process.pid}.${crypto.randomUUID()}`);
  try {
    writeFileSync(temp, `${JSON.stringify(value)}
`);
    renameSync(temp, path);
  } catch (error) {
    try {
      unlinkSync(temp);
    } catch {}
    throw error;
  }
}
var args = process.argv.slice(2);
var options = {};
var recognized = new Set([
  "--snapshot",
  "--state",
  "--now",
  "--state-out",
  "--result-out",
  "--state-path",
  "--snapshot-path"
]);
for (let index = 0;index < args.length; index += 2) {
  const flag = args[index];
  if (!recognized.has(flag))
    failure("usage", `reduce-state.sh: unknown argument: ${flag}`);
  const value = args[index + 1];
  if (!value || recognized.has(value))
    failure("usage", `reduce-state.sh: ${flag} requires a value`);
  options[flag] = value;
}
var snapshotPath = options["--snapshot"] ?? "";
var statePath = options["--state"] ?? "";
var now = options["--now"] ?? "";
if (!snapshotPath)
  failure("usage", "reduce-state.sh: --snapshot <path> required");
if (!statePath)
  failure("usage", "reduce-state.sh: --state <path> required");
if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(now ?? ""))
  failure("usage", "reduce-state.sh: --now must be an ISO-8601 UTC timestamp (YYYY-MM-DDTHH:MM:SSZ)");
if (!existsSync(snapshotPath))
  failure("invalid_json", `reduce-state.sh: snapshot not found: ${snapshotPath}`, {
    source: "snapshot"
  });
var snapshot;
try {
  snapshot = JSON.parse(readFileSync(snapshotPath, "utf8"));
} catch {
  failure("invalid_json", `reduce-state.sh: snapshot is not valid JSON: ${snapshotPath}`, {
    source: "snapshot"
  });
}
if (snapshot?.version !== 1 || typeof snapshot.repo !== "string" || typeof snapshot.number !== "number" || typeof snapshot.head?.sha !== "string" || !snapshot.pr || typeof snapshot.pr !== "object" || Array.isArray(snapshot.pr) || !Array.isArray(snapshot.threads)) {
  failure("invalid_json", "reduce-state.sh: snapshot is not a version-1 pr-babysit snapshot", {
    source: "snapshot"
  });
}
var previous = null;
if (existsSync(statePath)) {
  try {
    previous = JSON.parse(readFileSync(statePath, "utf8"));
  } catch {
    failure("state_malformed", `reduce-state.sh: state file is not valid JSON: ${statePath}`, {
      source: "state"
    });
  }
  if (previous?.version !== 2)
    failure("state_malformed", `reduce-state.sh: state file is not version 2: ${statePath}`, {
      source: "state",
      version: previous?.version ?? null
    });
  if (previous.repo !== snapshot.repo || previous.number !== snapshot.number)
    failure("slot_mismatch", `reduce-state.sh: state belongs to ${previous.repo}#${previous.number}, snapshot is ${snapshot.repo}#${snapshot.number}`, {
      source: "state",
      state: { repo: previous.repo, number: previous.number },
      snapshot: { repo: snapshot.repo, number: snapshot.number }
    });
}
var output;
try {
  output = reduceState(snapshot, previous, now, options["--state-path"] || statePath, options["--snapshot-path"] || snapshotPath);
} catch {
  failure("invalid_json", `reduce-state.sh: reducer failed on ${snapshotPath}`, {
    source: "reduce"
  });
}
if (options["--state-out"]) {
  try {
    atomicJson(options["--state-out"], output.state);
  } catch {
    failure("invalid_json", `reduce-state.sh: could not write ${options["--state-out"]}`, {
      source: "state_out"
    });
  }
}
if (options["--result-out"]) {
  try {
    atomicJson(options["--result-out"], output.result);
  } catch {
    failure("invalid_json", `reduce-state.sh: could not write ${options["--result-out"]}`, {
      source: "result_out"
    });
  }
} else
  process.stdout.write(`${JSON.stringify(output.result)}
`);
