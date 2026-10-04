# Pending change set

M1.5 exposes the engine's current overlay as a deterministic, read-only `ChangeSet`.

## Contract

- `Engine::pending_changes()` snapshots every present overlay entry.
- Entries are ordered by ascending `BlockId`.
- The snapshot is independent from later writes, restore, discard, and commit.
- A committed or discarded engine has an empty pending change set.
- The change set describes current in-memory pending state; it does not persist data and does not define backend commit-failure behavior.

## CLI

During an in-memory CLI session:

```text
changes
```

reports entries as `(block_index, first_byte)` in deterministic order. The first byte is a compact display value for the current CLI and is not the complete block representation.

## Boundary

This phase deliberately does not introduce a storage trait, persistence, filesystem integration, or an atomic-versus-partial backend commit policy.
