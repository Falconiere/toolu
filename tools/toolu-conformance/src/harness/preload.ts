/**
 * bun test preload (root bunfig.toml). Real-subprocess tests spawn git, bash
 * scripts and whole plugin-tree copies, and `test.concurrent` starts every test
 * in a file at once, so bun's 5 s default per-test timeout fails them on a
 * loaded 2-core CI runner. bunfig has no timeout key; this raises the default
 * for every suite run from the repo root. A test can still pass its own.
 */
import { setDefaultTimeout } from "bun:test";

const SUBPROCESS_TEST_TIMEOUT_MS = 60_000;

setDefaultTimeout(SUBPROCESS_TEST_TIMEOUT_MS);
