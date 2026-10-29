---
name: herdr-app
description: Inspect or change Herdr App spaces, tabs, panes, and worker visibility when the user asks.
---

# Herdr App

`hc` runs and controls programs and agents; `herdr-app` controls the visible
window. Use `hc --skill` for program lifecycle and `herdr-app --skill` for the
app's current command and customization guidance.

Inspect the running window and available commands before acting:

```bash
herdr-app ls
herdr-app status
herdr-app commands
```

These commands do not open a window. Do not run bare `herdr-app` merely to
inspect it. Take explicit IDs from the inspection output before acting.

When the user asks to see an existing worker, use its hc address, including the
machine prefix when present:

```bash
toolu herdr-app show <terminal-address>
```

This resolves the terminal through hc and asks the running app to show
it. Do not focus or rearrange the window just because a worker starts or needs
attention.

For a requested space, tab, or pane action, use the matching `herdr-app do`
command and the explicit ID of its target. For example, after inspecting IDs:

```bash
herdr-app do space.new --name <space-name> --folder <absolute-path>
herdr-app do tab.new --space <space-id> --name <tab-name>
herdr-app do pane.split --pane <pane-id> --direction right
```

Supply names for new spaces and tabs, check the response, and inspect the window
again to confirm the result. Closing spaces, tabs, or panes can end their
terminals; inspect the target and preserve other work. Never stop or restart a
core or app you did not start.
