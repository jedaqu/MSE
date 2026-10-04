# Errors and fixes

Public technical record only.

## M0-001

**Problem:** The first checkpoint prototype copied the base together with the change layer.

**Correction:** Checkpoints now retain only the overlay layer. The base is not copied by checkpoint creation.

**Status:** Corrected before integration.

## M0.1-001

**Problem:** The block write API could panic when an index was outside the modeled block range.

**Correction:** Block addresses are now typed with `BlockId`, writes are bounds-checked, and out-of-range writes return `WriteError::OutOfRange` instead of panicking.

**Status:** Corrected before integration.

## M0.1 closure

**Integrated commit:** `56b2aa2c16e30a97a215d35d5cbcb18c81557287`

**Scope:** Typed blocks and block IDs, explicit bounds errors, state counters, and boundary regression tests. No production storage integration was introduced.

**Validation:** PR #2 validation passed formatting, tests, and Clippy on the M0.1 head commit `0c529a342f8ee8855bee0ad15fd5dae368fe6726`.

**Post-merge CI evidence:** The available GitHub workflow/status interface returned no post-merge run or combined status for the integrated commit. This is recorded as an evidence limitation, not as a claim of failure.

**Audit:** `main` was inspected after integration. The M0.1 diff from `c8dd2c5c69e92ff4fe83c121e7e07557464eb347` to `56b2aa2c16e30a97a215d35d5cbcb18c81557287` contains only the intended changes in `src/lib.rs` and this public technical ledger entry.

**Status:** Closed at M0.1. Ready for the next milestone after this documentation change is integrated.

## M0.2-001

**Problem:** `restore()` accepted checkpoints with a different block count, while `diff()` compared vectors with `zip()` and could silently omit unmatched blocks.

**Correction:** Checkpoints retain their block count. `restore()` validates compatibility before changing the overlay and returns `RestoreError` on mismatch. `diff()` returns `Result` and reports the same incompatibility explicitly.

**Validation:** Added coverage for compatible and incompatible restore/diff behavior, including state preservation after a rejected restore and read/write after a valid restore.

**Commit:** `883f171` (initial M0.2 implementation; closure update follows in this PR).

**PR:** #4, `M0.2: validate checkpoint state compatibility`.

**CI evidence:** Both `rust` jobs passed on the initial PR commit (`37162199240`, `37162202660`). The closure documentation update is validated separately by the final PR head.

**Status:** Corrected; M0.2 PR CI passed.

## M0.2-002

**Problem:** The first formatting check found a Rust formatting difference in the new `diff()` expression.

**Correction:** Applied `cargo fmt` before the repeated quality gate.

**Status:** Corrected before PR.

## M0.3 lifecycle audit

**Defined behavior:** `write()` replaces the addressed overlay block. `checkpoint()` captures an immutable overlay snapshot. `restore()` replaces the current overlay after compatibility validation. `discard()` clears the complete overlay and leaves base blocks unchanged. `commit()` moves all overlay blocks into base and clears those overlay entries. `diff()` reports overlay entries that differ from the checkpoint. `read()` returns the overlay value when present, otherwise the base value.

**Validation:** Sequence tests cover checkpoint/restore, discard, commit followed by checkpoint and restore, repeated writes, empty diffs, immutable multiple checkpoints, empty engines, repeated lifecycle operations, and existing range errors.

**Commit:** `bbf8726` (initial M0.3 implementation; closure update follows in this PR).

**PR:** #5, `M0.3: specify lifecycle semantics with sequence tests`.

**CI evidence:** Both `rust` jobs passed on the initial PR commit (`37162363093`, `37162366588`). The closure documentation update is validated separately by the final PR head.

**Status:** Audit found no lifecycle implementation defect; M0.3 adds executable coverage for these semantics.

## M0.4 storage boundary decision required

**Audit:** The current base is an in-memory `Vec<Block>`, and `commit()` transfers overlay blocks into that base. A backend boundary used by `commit()` must describe how backend write failures affect the base and overlay.

**Decision required:** Choose whether a backend must apply a commit batch atomically, or whether partial commits are permitted and exposed as progress. This determines the storage trait contract, commit error type, and whether overlay entries are retained or cleared after a failure.

**Reason for stopping:** The in-memory implementation cannot exercise backend write failures. Choosing either failure contract here would establish lifecycle semantics without evidence or an agreed requirement. No storage abstraction or backend was added.

**Validation:** Static inspection of `Engine` and `commit()` found the unresolved failure boundary. Existing M0.3 operation-sequence tests provide behavioral evidence for the current in-memory semantics; they cannot validate a generic backend failure contract.

**Commit:** `73c78bf` (initial M0.4 audit; closure update follows in this PR).

**PR:** #6, `M0.4: record storage boundary decision gate`.

**CI evidence:** Both `rust` jobs passed on the initial PR commit (`37162776860`, `37162779872`). The closure documentation update is validated separately by the final PR head.

**Status:** Pending architecture decision. M0.5 is complete independently; its in-memory hardening does not resolve this backend decision.

## M0.5 hardening and baseline

**Scope:** Harden the existing in-memory core, improve executable boundary coverage, pin the validation toolchain, and establish an informational performance baseline.

**Correction / improvement:** Added adversarial tests for extreme block IDs, rejected-write state preservation, dense diff/restore behavior, and committed-state preservation. Added a release-mode baseline binary covering read, write, checkpoint, diff, restore, and commit paths. Pinned Rust to 1.98.1 so local and CI validation use the same toolchain.

**Boundary:** No persistence, storage abstraction, backend, filesystem, block-device, kernel, driver, or OS-specific integration was introduced. M0.4 backend commit failure semantics remain undecided.

**Validation evidence:** The M0.5 baseline document records a successful final CI run, including formatting, 23 tests, Clippy with warnings denied, and the release baseline. That evidence does not claim post-merge CI or broader backend behavior.

**Status:** Complete. M0.4 backend commit failure semantics remain undecided.

## M1 closure

**Integrated commit:** `aff5d726baa2135138c859a3751cd2474bdce606` (PR #9, merged to `main`).

**Scope:** In-memory product contract, typed process-local checkpoint identities, deterministic state inspection, lifecycle scenario, and initial CLI with checkpoint management.

**Validation:** PR CI run `37170485159` passed on head `63d721925af84cd1d93bc4615cba6574e9df6df3`. Post-merge push CI run `37170617406` passed on merged main SHA `aff5d726baa2135138c859a3751cd2474bdce606`.

**Boundary:** No persistence, storage backend, filesystem or block-device integration, kernel or driver code, OS-specific integration, or networking was introduced.

**Status:** Closed. M1 remains process-local and in-memory.

## M1.5 pending change-set boundary

**Problem:** The in-memory core could expose individual state queries and checkpoint differences, but it did not yet expose the current pending overlay as one deterministic, backend-neutral value.

**Correction:** Added `ChangeSet` and `Engine::pending_changes()` as a read-only snapshot of present overlay entries, plus CLI inspection and executable coverage.

**Intent:** Establish a narrow data boundary for future consumers without deciding persistence or backend commit-failure semantics.

**Boundary:** No storage trait, persistence, filesystem, block device, kernel, driver, OS-specific integration, or networking.

**Status:** Closed. Integrated in PR #11 at `d8961e6187ea8f6a6029a80a1c3692304f7c7717`. PR CI `37170880817` and post-merge CI `37170912660` both passed.

## M1.5-001

**Problem:** The first M1.5 commit omitted the closing brace for the new `ChangeSet` implementation, so the Rust source was syntactically invalid.

**Detection:** Static pre-PR audit of the exact branch head `6cdfc36d15a805d691220c776605b71b93793309`.

**Correction:** Closed the `ChangeSet` implementation before the `Engine` declaration. No scope expansion or semantic change was introduced.

**Status:** Corrected before PR creation. CI must validate the corrected head.


## M1.6 shared object and state-root foundation

**Goal:** Establish the abstraction required to share unchanged immutable data between multiple state generations without duplicating every block payload.

**Correction / improvement:** Added process-local `ObjectStoreId`, store-local `ObjectId`, deduplicating `ObjectStore`, immutable `StateRoot` mappings, explicit store-mismatch errors, and executable coverage for object reuse and root-level copy-on-write behavior.

**Boundary:** The foundation is entirely in-memory. It introduces no persistence, physical-address model, filesystem/block-device integration, kernel/driver code, operating-system-specific integration, networking, or durability semantics.

**Important limitation:** Numeric object identities are store-local. They are not yet content hashes or durable addresses.

**Status:** Closed. Integrated in PR #14 at `0715b87562b9e6dc70b978cd26228f77afa4d4fb`. PR CI `37171893949` and post-merge CI `37171927564` both passed.


## M1.6-001

**Problem:** The first M1.6 implementation head failed the formatting gate. Rustfmt required two spacing changes and multiline formatting for two long assertions.

**Detection:** PR #12 CI run `37171772511`, formatting step only. Tests, Clippy, and the performance baseline were skipped because formatting failed first.

**Correction:** Applied the exact rustfmt changes without semantic modification.

**Status:** Corrected before creating the replacement validation PR.


## M1.6-002

**Problem:** The rustfmt correction in M1.6-001 still left two extra blank lines: one between the import and static declarations, and one between the `ObjectStore` `Default` implementation and the next item.

**Detection:** PR #13 CI run `37171827171`, formatting step only.

**Correction:** Removed the remaining extra blank lines. No semantic change.

**Status:** Corrected before replacement validation PR.


## M1.6-003

**Problem:** The rustfmt gate still required the blank line between the `ObjectStore` `Default` implementation and the following `StateRoot` declaration to be removed.

**Detection:** PR #13 CI run `37171827171`, exact formatting diff after M1.6-002.

**Correction:** Removed that final rustfmt spacing difference. No semantic change.

**Status:** Corrected before replacement validation PR.


## M1.7 commit and recovery contract

**Pre-change audit:** `main` was verified at `3126be9f575bda86f0f3e43176ba00309342fabc`, with zero open pull requests. The exact post-merge push workflow for that SHA was run `37172014757`; its `rust` job `111346718476` passed formatting, tests, Clippy, and the M0.5 baseline.

**Scope:** Formalize transaction identity, logical generations, prepared state, observable commit outcomes, atomic visibility, pending-state retention, durability boundary, and reconciliation/retry rules.

**Implementation:** Added backend-neutral `TransactionId`, `Generation`, `PreparedState`, `PrepareError`, and `CommitOutcome` types with executable contract tests.

**Failure model:** `Committed` means publication is known to have occurred; `Aborted` means publication is known not to have occurred; `Unknown` means the publication result cannot yet be established. Unknown transactions retain the same transaction identity for reconciliation or retry.

**Boundary:** No persistence or backend implementation. The existing in-memory `Engine::commit()` remains unchanged and is still an infallible memory-only operation.

**Status:** Closed. Integrated in PR #16 at `d812001129d6c4a65c1bacec07cdca610a02af57`. PR CI `37174196400` and post-merge CI `37174229249` both passed.


## M1.7-001

**Problem:** The first M1.7 PR head failed the formatting gate because the final test-module closing region contained an extra blank line.

**Detection:** PR #16 CI run `37174135924`, job `111353099913`. The formatting step failed; tests, Clippy, and the performance baseline were skipped.

**Correction:** Removed the extra blank line without changing semantics or scope.

**Status:** Corrected before final PR validation.


## M1.7-002

**Problem:** The second M1.7 PR head failed the formatting gate because the Rust source was missing its final newline.

**Detection:** PR #16 CI run `37174167157`, job `111353193713`. The formatting step failed; tests, Clippy, and the performance baseline were skipped.

**Correction:** Restored the final newline only. No semantic change.

**Status:** Corrected before final PR validation.


## M1.7 closure

**Integrated commit:** `d812001129d6c4a65c1bacec07cdca610a02af57` (PR #16, merged to `main`).

**Scope:** Backend-neutral transaction identity, logical generations, prepared transaction state, observable commit outcomes, atomic visibility rules, pending-state retention, durability boundary, and reconciliation/retry semantics.

**Validation:** Final PR CI run `37174196400` passed on head `3c7d442bdbd8885142f8b6404391655f59dd01f5`. Post-merge push CI run `37174229249` passed on merged main SHA `d812001129d6c4a65c1bacec07cdca610a02af57`.

**Boundary:** No persistent backend, storage trait, journal, filesystem/block-device integration, kernel/driver code, OS-specific adapter, or networking was introduced. `TransactionId` and `Generation` remain process-local/logical identities at this stage.

**Status:** Closed. M1.7 establishes the contract gate for the next backend-neutral persistence/durability boundary.


## M1.8 persistence and durability boundary

**Pre-change checkpoint:** `main` at `579d017a73dcec9ae17f2ef5c80e22609cef86c5`, with zero open pull requests. Exact post-merge CI run `37174334855` completed successfully; rust job `111353700148` passed formatting, tests, Clippy, and the M0.5 performance baseline.

**Scope:** Define the minimum backend-neutral boundary between logical publication and physical durability, without adding a storage implementation.

**Implementation:** Added `DurabilityRequirement`, `DurabilityState`, `DurabilityObservation`, and `DurabilityObservationError`. Refined `CommitOutcome::Unknown` so it preserves both transaction identity and target generation.

**Contract:** Publication and durability are separate acknowledgements. A durable observation is valid only after `Committed` publication is confirmed. `Pending` and `Unknown` durability states never satisfy the durable requirement.

**Boundary:** No storage trait, persistent object store, journal, filesystem/block-device integration, physical addressing, durable transaction encoding, recovery database, kernel/driver code, OS-specific adapter, or networking.

**Status:** Closed. Integrated in PR #18 at `7ea2ab7f594f2a8d0862cb62c7bdb1ff90c90831`. PR CI `37209513008` and post-merge CI `37209545645` both passed.


## M1.8 closure

**Integrated commit:** `7ea2ab7f594f2a8d0862cb62c7bdb1ff90c90831` (PR #18, merged to `main`).

**Scope:** Backend-neutral persistence/durability boundary. `CommitOutcome::Unknown` now retains target generation, and durability requirements/observations are explicitly separated from logical publication.

**Validation:** PR CI run `37209513008` passed on head `daad9629c3095d4c7a0687a7be423b9d8be713ca`. Post-merge push CI run `37209545645` passed on merged main SHA `7ea2ab7f594f2a8d0862cb62c7bdb1ff90c90831`.

**Boundary:** No persistent backend, storage trait, journal, filesystem/block-device integration, physical addressing, durable identifier encoding, recovery store, kernel/driver code, OS-specific adapter, or networking was introduced.

**Status:** Closed. M1.8 establishes the minimum contract needed before the first concrete persistence experiment.

## M1.9 first persistent backend

**Integrated commit:** `c9eaffcf6496c3c1c61c8300b735f0c6c96983f9` (PR #20, merged to `main`).

**Scope:** Experimental append-only host-file persistence with immutable object records, content deduplication, prepared roots, logical generations, COMMIT/DISCARD records, same-transaction reconciliation, recovery of incomplete tails, and explicit filesystem durability acknowledgement.

**Contract:** M1.9 preserves the M1.7/M1.8 boundaries. `Committed` confirms logical publication; `Unknown` preserves the same transaction identity for reconciliation or retry; `Aborted` confirms that the attempted publication did not occur without invalidating the retained prepared state, so the same `TransactionId` may retry until explicit discard.

**Validation:** Final PR CI run `37214459183` passed formatting, tests, Clippy, and the M0.5 performance baseline. Post-merge push CI run `37214956081` passed on integrated main SHA `c9eaffcf6496c3c1c61c8300b735f0c6c96983f9`.

**Boundary:** This milestone remains an experimental host-file persistence backend. No OS-specific adapter, block-device integration, kernel/driver code, networking, or distributed coordination was introduced.

**Public audit:** The implementation branch was reviewed for scope and public-boundary compliance before integration. The change was limited to `src/lib.rs` and the new `src/persistence.rs`.

**Status:** Closed. M1.9 establishes the first persistent backend experiment; the next milestone must build on the existing core contract rather than redefine it.



## M1.10 backend-neutral persistence boundary

**Problem:** M1.9 validated a concrete `FileBackend`, but the public core did not yet expose an explicit backend-neutral persistence boundary for future implementations.

**Correction:** Added `PersistenceBackend` to express logical block access, preparation, publication, reconciliation, discard, generation state and durability observation without exposing host-file details. Adapted `FileBackend` to implement the contract and changed trait-level `prepared()` access to return an owned `PreparedState`.

**Detection:** M1.10 implementation audit before PR #22.

**CI incident:** Run `37215796545` failed only the formatting gate. Rustfmt required formatting changes in `src/backend.rs` and `src/persistence.rs`. No tests, Clippy or baseline execution occurred in that run.

**Correction:** Applied the exact rustfmt changes and retained the intended API-only scope. Final branch validation run `37215849872` passed formatting, tests, Clippy and the M0.5 performance baseline.

**Post-merge validation:** Push run `37215913988` passed on integrated main SHA `d36a2d809484ddde73d9a1fe8a14c5d904fd7392`.

**Status:** Closed.


## M1.11 backend contract test harness

**Problem:** `PersistenceBackend` defined the backend-neutral API, but behavioral guarantees were still duplicated mainly in the concrete `FileBackend` tests.

**Correction:** Added a reusable backend contract harness that exercises the semantic guarantees through `PersistenceBackend` only. `FileBackend` now runs the shared behavioral suite.

**Detection:** M1.11 pre-audit from `main` after M1.10 closure.

**CI incident:** Run `37217706701` failed only the formatting gate; no test, Clippy or baseline execution occurred.

**Correction:** Applied the exact rustfmt formatting corrections. Final implementation run `37217734416` passed formatting, tests, Clippy and the M0.5 baseline.

**Post-merge validation:** Push run `37217776920` passed on integrated `main` SHA `7838748d4234a6881e3148a281f00febc247547a`.

**Scope boundary:** Reopen/recovery lifecycle was intentionally not abstracted in M1.11 because the existing `PersistenceBackend` contract does not define a backend-neutral reconstruction mechanism.

**Status:** Closed.
