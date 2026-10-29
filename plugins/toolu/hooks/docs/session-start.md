# Session Protocol — {{project_name}}

## Behaviour
- Evidence before claims: verify (tests/logs/runtime).
- Failed twice? Stop and change hypothesis.
- No unsolicited changes (gate excepted).
- Follow installed Ponytail's selected mode unless user-disabled; discover it in the host skill catalog, never auto-install.

## Delegation
Inline by default. Delegate only large reads or independent units, with one
question and return shape per agent. Take quiet agents back inline.

## Mandatory
- Do not ship with a failing quality gate; continue work needed to repair it.
- Tests use real data; no mocks hiding integration.
