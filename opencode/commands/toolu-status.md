---
description: Show the current toolu quality-gate status (statusline equivalent for opencode).
agent: build
---

# toolu status

opencode has no statusline bar the way Claude Code does. This command is the
closest equivalent: it reads the project's quality-gate status file and prints
it inline.

Read the gate file and print its contents verbatim:

```sh
GATE_FILE="${OPENCODE_PROJECT_DIR:-$(pwd)}/.opencode/tmp/quality-gate-status.json"
if [ -f "$GATE_FILE" ]; then
  cat "$GATE_FILE"
else
  echo '{"status":"clear"}'
fi
```

Interpret the JSON for the user:

- `{"status":"clear"}` — gate is green; no recent quality failures.
- `{"status":"failing","reason":"..."}` — gate is red; the `reason` field names
  the most recent failing concern. Tell the user the failing reason verbatim
  and recommend running the project's check/lint/typecheck command to confirm
  it's still failing before fixing.
- Missing file or unparseable JSON — the post-tools gate has not run yet this
  session; treat as `clear` and note that gate feedback will appear on the
  next edit or bash command.

Do not summarize or reformat. Print the JSON line and the interpretation. Do
not mark the task complete with a `###TASK_COMPLETED###` marker — this is a
read-only command.
