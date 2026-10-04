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

Checkpoints are independent snapshots: later writes, commit, and discard do not
mutate an already-created checkpoint. Restore may be repeated while the
checkpoint remains compatible with the engine's fixed block count.
The ID identifies the immutable checkpoint value in the current process; the
checkpoint carries the state that `restore()` uses. IDs are not serialized or
stable across executions.
