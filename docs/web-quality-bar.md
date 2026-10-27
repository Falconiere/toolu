# Dashboard web quality bar

`cargo xtask gate --only web` checks the dashboard assets in
`crates/epic-orchestrator/assets/dashboard/`. Until that directory exists, the
step prints an absence message and passes. Once it exists, missing tools or any
finding fail the step. Install the exact npm tools with `npm ci`; ast-grep and
Node must also be on `PATH`. To check the clean fixture directly, run
`cargo xtask gate --only web fixtures/web-gate/clean`.

The asset layout is `index.html`, CSS with colors defined only in `tokens.css`,
DOM-free ES modules under `js/logic/`, DOM rendering under `js/view/`, and the
sole `fetch` caller in `js/api.js`. Each `js/logic/<name>.js` needs
`tests/<name>.test.js`. Files under `tests/` are never served. `fonts/*.txt`
may contain OFL license URLs; served HTML, CSS, and JS may not contain external
HTTP URLs.

The pinned tools and their policy files are:

| Check | Policy | What it enforces |
| --- | --- | --- |
| oxlint 1.90.0 | `tooling/conventions/guardrails/web/oxlintrc.json` | Correctness, suspicious and performance categories; house JS rules; 300 code lines per JS file, 50 per function; unsafe DOM APIs and `localStorage` bans. |
| oxfmt 0.75.0 | `oxfmtrc.json` | Formatting of HTML, CSS and JS. |
| html-validate 11.17.0 | `html-validate.json` | Recommended and accessibility presets. Two style rules require oxfmt's lowercase doctype and self-closing void elements; both remain errors. |
| ast-grep 0.51.0 | `sgconfig.yml`, `patterns/*.yml` | Token-only colors, no external URLs, inline script/style/handler bans, `fetch` ownership, DOM-free logic and suppression comments. |
| jscpd 4.0.5 | `jscpd.json` | Zero duplication in JS and CSS, with ten-line repeated blocks detected. |
| Node | `tests/*.test.js` | Built-in `node --test`; failed tests fail the gate. |

`rules.json` caps each HTML and CSS file at 300 nonblank, noncomment code lines,
served HTML/CSS/JS at 150,000 bytes together, and fonts at 200,000 bytes.
The existing Rust guardrails secret scan covers this directory because it is
under `crates/`. The `rust` CI path group includes the web policy and assets;
`cargo xtask gate` runs the web step after the existing jscpd step.

These configs are gate data. A policy change belongs in its own
`chore(gates): …` PR; `cargo xtask check-gate-change` rejects it alongside
unrelated product code. The policy adapts the JS-applicable rules from
[toolu-conventions](https://github.com/Falconiere/toolu-conventions) without
its type-aware TypeScript config or JS plugin. When its plain-browser profile
ships, replace these files after verifying the same fixture behavior.
