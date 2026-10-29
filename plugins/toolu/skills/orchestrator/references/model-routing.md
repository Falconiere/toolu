# Model routing

Use the session's configured routing first. This reference supplies defaults
and override semantics when that information is missing. Route by work class,
not prompt length.

| Class | Claude alias | Codex default | Work |
| --- | --- | --- | --- |
| mechanical | haiku | Luna / medium | Exact lookup, formatting, one command |
| exploration | sonnet | Terra / medium | Read-only subsystem evidence |
| implementation | sonnet | Terra / medium | Decided edit and verification |
| review | sonnet | Terra / high | Diff review or audit |
| synthesis | opus | Sol / high | Reconcile findings |
| architecture | opus | Sol / high | Ambiguous or cross-cutting design |

Escalate for hard-to-reverse changes, broad impact or unresolved trade-offs.
De-escalate for bounded work with one verifiable answer. Deciding a public
contract and implementing it are different work classes. On `ESCALATE`, supply
the missing evidence or use a higher tier; do not repeat the same failed brief.

Use available host model IDs and reasoning settings, not an inferred model
name. The [orchestrator](../SKILL.md) owns role selection and host limitations.

## Overrides and profiles

Claude class overrides live under `models` in `toolu.config.json`:

```json
{ "models": { "review": "opus", "mechanical": "sonnet" } }
```

Aliases: `haiku`, `sonnet`, `opus`, `fable`, `inherit`; invalid values warn
and fall back to defaults. `inherit` uses the lead thread's model.
`models.enabled: false` disables session routing injection.

Codex uses `models.codex.<class>` model/effort pairs. Installed custom-agent
profiles take precedence over class routing. Neither a class override nor this
text rewrites an agent's fixed frontmatter. Run `$toolu:setup` after plugin
upgrades to update managed Codex profiles.

Plan steps may store an optional `model` alias; `toolu ledger status` displays
it. Reuse the recorded decision during execution; legacy steps without it use
configured class routing. Keep agent count bounded independently of tier.
