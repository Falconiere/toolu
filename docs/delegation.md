# Delegation across machines

Toolu can queue committed work on registered Herdr machines. An always-on
relay owns the queue and continues scheduling while the submitting controller is
offline. The controller and relay need `hc` connectivity to the configured
machines. Install Toolu on the relay and the selected agent CLI on each worker.
A worker can use its installed matching Toolu; otherwise the relay stages its
own executable privately for the attempt when OS and architecture match. For
different platforms, install a matching Toolu on the worker. Workers need Git
and their own credentials for the repository origin; credentials are not sent
with a task.

## Configure workers and relay

Use existing `hc` machine names. A worker root is an absolute, normalized path
on that machine. It holds repository clones under `repos/` and isolated attempt
worktrees under `tasks/`.

```sh
toolu setup machines register worker-a --root /srv/toolu --agent codex --agent claude --capacity 2
toolu setup machines list
toolu setup relay --machine relay-a
toolu machines status
```

Registration is global to the user's configuration, not the current repository.
The directory is `TOOLU_CONFIG_DIR/delegation` when that variable is set;
otherwise it is `$XDG_CONFIG_HOME/toolu/delegation`, or
`$HOME/.config/toolu/delegation` when `XDG_CONFIG_HOME` is unset. The relay
keeps its own authoritative state. Configuration changes made through these
commands are published to the configured relay.

Each enrolled relay has a unique authority generation. Machine and recipe
changes read the current relay configuration and apply only if it still matches
that snapshot; a concurrent change causes a conflict instead of silently
overwriting it. Retry the intended edit after inspecting the current registry.
Re-selecting the same active relay refreshes this controller from its
authoritative registry. On a controller's first attachment, existing local
registrations and recipes are merged into the relay's registry; a local entry
with the same name or origin takes precedence in that initial merge.

Optional setup recipes are JSON arrays of argument arrays, keyed by the exact
Git origin URL. For example, a file named `recipe.json` containing
`[["cargo", "fetch"], ["cargo", "build"]]` can be installed with:

```sh
toolu setup machines recipe --origin https://example.com/team/project.git --file recipe.json
```

Commands run directly in the attempt worktree, without shell expansion. Use an
empty array to remove a recipe. Write recipes so that repeating them is safe:
an interrupted step with an uncertain outcome is held for inspection instead
of being run again automatically. Existing attempts retain the recipe selected
at reservation time.

Drain a worker before maintenance or removing its registration. Drain stops new
assignments; existing work keeps its slot until it settles. Resume permits new
assignments again.

```sh
toolu machines drain worker-a
toolu machines status
toolu machines resume worker-a
toolu setup machines remove worker-a
```

An occupied worker cannot be removed or have its root, agents, or capacity
changed. To change relay authority, drain and settle queued and active work on
the old relay first, then run `toolu setup relay --machine <new-machine>`.
Done, cancelled, and failed receipts are terminal outcomes; they do not need
to be removed to retire the old authority.

Relay selection records its destination and new generation before retiring the
old authority or enrolling the new one. If selection is interrupted or returns
an uncertain error, run `toolu setup relay --machine <new-machine>` again with
the same destination to resume it; a different destination is refused while the
selection is pending. Retired generations stay fenced even if an old service or
controller returns. If the enrollment succeeded and another controller has
already moved authority again, retry settles the pending receipt and names the
successor to select. A controller that discovers its relay has retired must
select the successor named by the error before editing machines. Selection
refresh and `toolu relay recover` are separate: recovery restarts the selected
service, while setup completes or refreshes the authority selection.

## Submit and collect work

Commit all intended input first. The repository must have an `origin` remote;
the brief is a local text file. Choose a unique durable key before submission
and keep it for retries. Reusing a key with different input conflicts with its
existing receipt. The caller saves the submission and its relay destination
before delivery, so retry the same request with the same key after an uncertain
response.

```sh
toolu delegate submit --key task-001 --repo /path/to/repo --brief /path/to/brief.txt --agent codex
toolu delegate status task-001
toolu delegate results task-001 --repo /path/to/repo
toolu delegate cancel task-001
```

`submit` also accepts `--machine <name>` to pin a worker, `--group <name>` to
serialize tasks in a group, and agent flags after `--`. Eligible workers must
support the requested agent, have free capacity, and not be draining. The relay
selects the lowest occupied-to-capacity ratio, with machine name as the tie
breaker. A task stays queued when no worker is eligible.

A failed preparation puts that worker on a persisted cooldown. Unpinned retries
can use another eligible worker; pinned work waits for its worker's cooldown.
`machines status` shows `preparation_cooldown_until` and excludes those slots
from availability until the deadline passes.

With a relay configured, `toolu babysit dispatch-fix start --machine <name>`
also pins a fixer to a registered worker; its root comes from the registration.
Without a relay, the legacy `--machine` and `--remote-repo` flags remain paired.

Results imports a verified commit into a private `refs/toolu-transfer/.../snapshot`
ref in the specified repository; it does not merge or push it. The repository
origin must match the submitted origin. Cancellation of active work holds its
capacity until the owned terminal is confirmed stopped.

The worker receives instructions to commit, report, and exit. For manual worker
reporting, the attempt, base, and output file come from the worker's assigned
task; the commit must be the current HEAD. A successful report also requires a
clean worktree and a commit descended from the base.

```sh
toolu delegate report --file /srv/toolu/tasks/attempt-report.json --attempt attempt --base BASE_COMMIT --status done --commit HEAD_COMMIT
toolu delegate report --file /srv/toolu/tasks/attempt-report.json --attempt attempt --base BASE_COMMIT --status failed --commit HEAD_COMMIT --note "reason"
```

The relay accepts only a report bound to the attempt and input commit. A
finished terminal without a valid report remains held for inspection; uncertain
launches are not automatically repeated.

## Operate and recover

```sh
toolu relay status
toolu relay recover
```

`status` includes the relay service and its recorded scheduler health.
The worker list shows HC's observed connection state, occupied and available
slots, drain state, registered agents, and persisted preparation errors. A
connection alone does not prove readiness: Toolu, the agent and applicable build
prerequisites are checked again before launch. If HC cannot be queried, the
registry and assignments remain visible with unknown connections and an error.
`recover` reconciles the owned service terminal and starts it if needed.
`toolu relay serve` runs the scheduler on the designated relay and is normally
started by setup or recovery. Keep the relay machine available for new work;
already launched workers remain independent of the controller.

Receipts, transfer data, repository clones, and attempt worktrees are retained;
plan disk space and back up the relay state and worker data as appropriate.
They are not an automatic backup or a complete disaster-recovery mechanism.

## Two-physical-machine verification

This procedure requires two distinct connected Herdr machines, here called
`macmini` and `macbook`, and an existing relay alias `relay-a`. The aliases must
be configured in a dedicated controller HC session; the controller must reach
the relay, and the relay must reach each worker. Use a disposable repository whose
committed input has the same authenticated `origin` on both machines. Both
workers need Git, the selected agent CLI and their own credentials. Install a
matching Toolu on the relay; worker reporting uses an installed matching Toolu
or stages it privately on a compatible platform. Use isolated configuration and
absolute scratch roots that do not contain existing work. Do not run this against
a production relay or worktree.

On the controller, set `HC_SESSION` to the dedicated session, choose an empty
`TOOLU_CONFIG_DIR`, and put the native `toolu`, `hc`, Git and agent CLIs on
`PATH`. Set `REPO` to the disposable controller checkout, `BRIEF` to a local
brief file, `MINI_ROOT` and `BOOK_ROOT` to absolute scratch paths on the two
workers, and `RELAY` to the configured relay alias. The brief should direct the
agent to make and commit a small unique change, run its check, and use the
success or failure reporting command provided in its prompt. Give the work
enough time to observe an active terminal before disconnecting.

```sh
toolu setup machines register macmini --root "$MINI_ROOT" --agent codex --capacity 1
toolu setup machines register macbook --root "$BOOK_ROOT" --agent codex --capacity 1
toolu setup relay --machine "$RELAY"
toolu --json machines status
toolu --json delegate submit --key physical-mini --repo "$REPO" --brief "$BRIEF" --agent codex --machine macmini
toolu --json delegate submit --key physical-book --repo "$REPO" --brief "$BRIEF" --agent codex --machine macbook
toolu --json delegate status physical-mini
toolu --json delegate status physical-book
```

Record both task receipts, distinct assigned machines, each physical machine's
actual hostname, attempt IDs, base commits, and running terminal identities.
Record complete task-attempt and owned-terminal inventories before and after
recovery so a second launch cannot be hidden by matching only the first receipt. Verify that the selected machine in
each receipt matches its pin. After observing both tasks in progress, stop only
the dedicated controller HC session with `hc server stop`; the isolated worker
and relay sessions must remain available. From a separate operator session,
observe the relay and workers while the controller is disconnected. Restore the
controller session and inspect both receipts again. Confirm that the same attempt
and terminal identities were retained, no second worker started, and work made
progress while the controller was unavailable.

Next, after recording the relay's owned terminal from `toolu --json relay status`,
stop that dedicated test service with `hc kill "$RELAY/toolu-relay"`. Confirm
that dispatch pauses while it is stopped; then run:

```sh
toolu --json relay recover
toolu --json relay status
toolu --json delegate status physical-mini
toolu --json delegate status physical-book
```

Confirm that recovery restarts the owned relay, preserves the original task and
attempt identities, and resumes reconciliation without a duplicate terminal.
Once each task reports `done`, import its result and inspect the receipt's
attempt ID and result commit:

```sh
toolu --json delegate results physical-mini --repo "$REPO"
toolu --json delegate results physical-book --repo "$REPO"
toolu --json delegate results physical-mini --repo "$REPO"
```

Use each recorded attempt ID with `git -C "$REPO" show
refs/toolu-transfer/<attempt>/snapshot` and verify that the imported commit and
change match that task's receipt; the repeated import must resolve to the same
commit. Record command outputs, host identities, tool versions and observations,
including any failure or held state. Separately submit a task with an unavailable
or draining pin and verify it remains queued, and exercise cancellation of an
active task to verify capacity remains reserved until the owned terminal stops.
Retain the scratch state until results are inspected. The loopback tests exercise
real HC and Git but do not establish that these physical-machine checks passed.

### Observed verification

The recorded [two-machine evidence](delegation-evidence.json) covers real
Codex workers on macmini and macbook, controller disconnection, relay recovery,
and repeated verified imports. Trust dialogs were accepted only for the
synthetic one-file fixtures. The first relay recovery exposed a dead-owner lock
issue, which was fixed and regression-tested before recovery succeeded. The
first macbook worker needed one corrective report command using the verified
absolute reporter path; it did not start another attempt. A later macbook
regression task completed with the persisted absolute reporter path without
that manual correction. These observations do not assert the full gate passed.
