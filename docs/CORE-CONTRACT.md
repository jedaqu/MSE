# In-memory core contract

`Engine` owns a fixed-size zero-filled base and an equally sized optional-value
overlay. This contract describes the current in-memory implementation. It does
not define persistence or future backend failure behavior.

| Operation | Contract |
| --- | --- |
| `block_count()` | Returns the fixed number of blocks allocated by `Engine::new`. |
| `read(id)` | Returns the overlay block when present, otherwise the base block; out-of-range IDs return `None`. |
| `write(id, block)` | Replaces that ID's overlay value. It does not change the base. Out-of-range writes return `WriteError` without mutation. |
| `checkpoint()` | Copies the current overlay and block-count shape into an immutable checkpoint with a process-local typed `CheckpointId`. The base contents are not captured. |
| `restore(checkpoint)` | Replaces the overlay when block count and overlay length are compatible. It leaves the base unchanged; incompatible checkpoints return `RestoreError` before mutation. |
| `discard()` | Clears every overlay entry and leaves the base unchanged. |
| `commit()` | Moves all present overlay values into the in-memory base and clears those entries. This describes only the infallible memory operation; it makes no choice about future backend atomicity or partial progress. |
| `dirty_count()` | Counts present overlay entries. A block written with bytes equal to its base still counts as dirty. |
| `pending_changes()` | Returns an immutable in-memory snapshot of present overlay entries in ascending block order. It exposes current pending changes without choosing persistence or backend commit-failure semantics. |
| `diff(checkpoint)` | Returns IDs whose optional overlay entries differ from the checkpoint overlay, in ascending block order. It does not compare base contents or effective read values. Incompatible checkpoints return `DiffError`. |
| `inspect(checkpoint)` | Returns total and dirty counts, whether changes exist, affected IDs in ascending order, and an explicit checkpoint relationship. It does not mutate state. |

## Shared object model

`ObjectStore` interns identical immutable blocks once within a process-local store. `StateRoot` maps logical block positions to `ObjectId` references, so a new root can replace one reference while reusing every unchanged object. Roots and objects are intentionally in-memory and store-local in this phase; no physical-address, persistence, or durability semantics are defined.

Checkpoints are independent snapshots: later writes, commit, and discard do not
mutate an already-created checkpoint. Restore may be repeated while the
checkpoint remains compatible with the engine's fixed block count.
The ID identifies the immutable checkpoint value in the current process; the
checkpoint carries the state that `restore()` uses. IDs are not serialized or
stable across executions.


## M1.7 commit and recovery contract

The repository now defines backend-neutral transaction vocabulary without implementing persistence.

- `TransactionId` identifies one commit attempt.
- `Generation` identifies the logical committed state sequence.
- `PreparedState` freezes the transaction identity, source generation, target generation, and `ChangeSet`.
- `CommitOutcome` exposes only `Committed`, `Aborted`, or `Unknown`.
- Atomic visibility is a logical publication rule: observers see the old committed generation or the new committed generation, never a partially committed generation.
- Physical preparation may involve multiple internal writes, but those writes are not themselves committed visibility.
- An `Unknown` outcome retains the same transaction identity for reconciliation; a retry must reconcile or replay that same transaction rather than silently creating an independent second commit.
- Durability is a separate boundary from logical publication and is not implemented by the current in-memory core.

This contract is intentionally backend-neutral. No storage trait, persistence layer, journal, filesystem, block device, kernel, driver, operating-system-specific integration, or network protocol is introduced by M1.7.


## M1.8 persistence and durability boundary

M1.8 adds only backend-neutral value types for separating logical publication from physical durability.

- `DurabilityRequirement::Publication` means the caller needs a confirmed logical publication.
- `DurabilityRequirement::Durable` means the caller also requires a positive durability acknowledgement.
- `DurabilityState::Pending` means durability has not been established.
- `DurabilityState::Durable` means durability is positively established.
- `DurabilityState::Unknown` means a durability attempt occurred but its final result is not established.
- `DurabilityObservation` binds a durability state to the same `TransactionId` and published `Generation`.
- A durability observation can only be created from `CommitOutcome::Committed`. `Aborted` and `Unknown` publication outcomes do not authorize a durability claim.
- `CommitOutcome::Unknown` now retains the target generation as well as the transaction identity, so reconciliation has a stable logical target even when publication confirmation is lost.

This is a contract boundary only. No persistence trait, journal, filesystem, block device, physical address, durable identifier encoding, recovery store, kernel/driver integration, operating-system adapter, or network coordination is implemented.
