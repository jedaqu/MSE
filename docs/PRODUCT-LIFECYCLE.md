# In-memory product lifecycle scenario

Run the deterministic example from the repository root:

```text
cargo run --release --example product_lifecycle
```

It exercises create, write, checkpoint, modify, diff, restore, verification,
commit, later writes, discard, and verification of the committed state. Expected
output:

```text
created: blocks=3
write: block=0 value=10
checkpoint: created
diff: blocks=[0, 1]
restore: block0=10 block1=0
commit: dirty=0 block0=10
discard: dirty=0 blocks=[10, 0, 0]
```

The engine exists only for the process lifetime. The example uses no external
dependencies and does not access storage or operating-system facilities.
