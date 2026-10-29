# Delivery model routing

Use the session's configured class/model/effort values. For missing defaults or
override semantics, locate the installed `toolu:orchestrator` skill in the host
catalog and read its model-routing reference. That required dependency owns
the tier table; do not duplicate it or guess a model identifier.

Plan steps may retain a `model` alias. Read it from `toolu ledger status`
during execution rather than deciding the same routing again. For legacy steps
without it, apply the configured work class. An undecided design belongs to
architecture; implementing an agreed step belongs to implementation.

Use only the host's exposed delegation capability and available models. Missing
required routing evidence is a limitation to resolve, not permission to inherit
an arbitrary default.
