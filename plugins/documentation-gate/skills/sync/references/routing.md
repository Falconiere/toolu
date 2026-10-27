# Worker routing

Build a JSON array of 1–254 readiness-checked profiles. Each object has exactly the relevant fields `id`, `host`, `model`, `effort`, and `description`, all nonempty strings. IDs are unique ASCII letters, digits, `_` or `-`; `no-suitable-route` is reserved. Hosts are `claude`, `codex`, `cursor`, or `muse`. Cursor effort is `model` (encoded in its model ID); other hosts accept `low`, `medium`, `high`, `xhigh`, `max`, or `ultra` only when the selected host and model actually support it. Validate installation, authentication, dispatch availability and model/effort support before including a profile. Provide nonempty evidence about the diff and task in the evidence file.

```text
toolu docs route --candidates candidates.json --evidence evidence.txt
```

The result contains `source`, `candidate_id`, `host`, `model`, and `effort`. Preserve it unchanged. Jev uses exported credentials; missing credentials, invalid replies or no suitable route are errors. Recheck readiness immediately before launch and reroute if it changed.

Use the host's native subagent interface only if it can explicitly honor the chosen host, model and effort. Otherwise, when running inside Herdr, read `herdr --skill` and installed `herdr agent` help. Create a sibling pane without focus, start the selected host with its documented native model and effort arguments, and prompt the named agent with a bounded wait. Check the started agent's identity and selected settings; if the installed interface cannot honor or verify the tuple, stop and report the limitation. Inspect blocked or timed-out agents before any retry. Herdr requires its managed environment; do not control a user's focused session from outside it.
