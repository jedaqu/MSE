# Shared object and state-root model

M1.6 establishes the in-memory identity layer suggested by copy-on-write state systems.

## Core idea

A state does not own a separate copy of every unchanged block. Instead:

```text
StateRoot A ──┐
              ├──► Object X
StateRoot B ──┘

StateRoot A ─────► Object Y
StateRoot B ─────► Object Y2
                      ^
                 only changed object
```

`ObjectStore::intern()` returns the same `ObjectId` for identical block contents. `StateRoot::replace_object()` creates a new root mapping while leaving the previous root unchanged.

## Invariants

- One `ObjectStore` has one process-local identity.
- Identical block contents are interned once per store.
- An object is immutable after insertion.
- A `StateRoot` references objects; it does not contain block payloads.
- Replacing one root entry reuses every unchanged object reference.
- A root cannot be applied to a different object store.
- No physical storage address is exposed by the abstraction.

## Deliberate limitation

The current numeric `ObjectId` values are store-local and process-local. They are identities for the in-memory foundation, not content hashes or durable storage addresses.

The next architectural work can decide whether a persistent backend should map these logical object identities to content-addressed records, extents, or another durable representation.
