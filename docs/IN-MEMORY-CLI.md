# Initial in-memory CLI

Start a session with:

```text
cargo run --bin mse
```

Enter `help` for the command list. `new <blocks>` creates or resets the
session's engine. `write <index> <byte>` fills one block with the given byte;
`read <index>` reports its first byte. `checkpoint` creates a checkpoint and
prints its session ID; `checkpoints` lists available IDs. Use `inspect
[checkpoint-id]` or `diff <checkpoint-id>` to inspect changes. The remaining
lifecycle commands are `restore <checkpoint-id>`, `discard`, and `commit`.
`quit` or `exit` ends the session.

The CLI starts without an engine, and all engine and checkpoint state is lost
when the process exits. Checkpoint IDs are generated for the current process
and must be copied from the `checkpoint: id=...` output; they are not assumed
to start at `1`. The CLI uses only the Rust standard library and has no file,
database, backend, configuration, plugin, or network support.

Example session:

```text
new 2
write 0 12
checkpoint
# suppose the command reports: checkpoint: id=42
write 1 34
diff 42
restore 42
commit
inspect
quit
```

Replace `42` with the actual checkpoint ID printed by the session.
