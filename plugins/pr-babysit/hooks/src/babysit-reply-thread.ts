#!/usr/bin/env bun
import { existsSync, statSync } from "node:fs";
import { fail, flag, parseFlags, runCli } from "./babysit/common";
import { replyThread } from "./babysit/writes";

runCli(async () => {
  const flags = parseFlags(process.argv.slice(2), "reply-thread.sh", [
    "--state-file",
    "--kind",
    "--thread",
    "--root-comment",
    "--in-reply-to",
    "--comment-id",
    "--review-id",
    "--body-file",
    "--timeout",
  ]);
  const statePath = flag(flags, "--state-file");
  const kind = flag(flags, "--kind");
  const bodyPath = flag(flags, "--body-file");
  const thread = flag(flags, "--thread");
  const root = flag(flags, "--root-comment");
  const inReplyTo = flag(flags, "--in-reply-to");
  const commentId = flag(flags, "--comment-id");
  const reviewId = flag(flags, "--review-id");
  if (!statePath) fail("usage", "reply-thread.sh: --state-file required");
  if (!bodyPath || !existsSync(bodyPath))
    fail("usage", "reply-thread.sh: --body-file <existing file> required");
  const bodySize = statSync(bodyPath).size;
  if (bodySize === 0) fail("usage", "reply-thread.sh: reply body is empty");
  if (bodySize > 65536)
    fail("usage", "reply-thread.sh: reply body exceeds GitHub's 65536-character limit");
  if (kind === "thread") {
    if (!/^[A-Za-z0-9_=-]+$/.test(thread))
      fail("usage", "reply-thread.sh: --thread <graphqlId> required for --kind thread");
    if (!/^\d+$/.test(root))
      fail("usage", "reply-thread.sh: --root-comment <databaseId> required for --kind thread");
    if (!/^\d+$/.test(inReplyTo))
      fail("usage", "reply-thread.sh: --in-reply-to <databaseId> required for --kind thread");
  } else if (kind === "conversation") {
    if (!/^\d+$/.test(commentId))
      fail("usage", "reply-thread.sh: --comment-id <id> required for --kind conversation");
  } else if (kind === "review") {
    if (!/^\d+$/.test(reviewId))
      fail("usage", "reply-thread.sh: --review-id <id> required for --kind review");
  } else {
    fail("usage", "reply-thread.sh: --kind must be thread, conversation or review");
  }
  if (!Bun.which("gh")) fail("gh_unavailable", "gh is required");
  return replyThread({
    statePath,
    kind,
    thread,
    root,
    inReplyTo,
    commentId,
    reviewId,
    bodyPath,
    ...(flag(flags, "--timeout") ? { timeoutSeconds: Number(flag(flags, "--timeout")) } : {}),
  });
});
