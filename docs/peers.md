# Retired relay conversations

Relay-backed agent messaging has been removed. Existing peer messages,
credentials and receipts remain on disk; this upgrade does not delete them.
Use the previous binary when access to those records is needed.

Read-only fleet dashboards and their HTTP peers remain supported. See
[upgrade guidance](delegation.md) for the remote worker removal boundary.
