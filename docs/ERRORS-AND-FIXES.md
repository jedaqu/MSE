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

**Status:** Pending architecture decision. M0.5 has not started.

## M0.5 hardening and baseline

**Scope:** Harden the existing in-memory core, improve executable boundary coverage, pin the validation toolchain, and establish an informational performance baseline.

**Correction / improvement:** Added adversarial tests for extreme block IDs, rejected-write state preservation, dense diff/restore behavior, and committed-state preservation. Added a release-mode baseline binary covering read, write, checkpoint, diff, restore, and commit paths. Pinned Rust to 1.98.1 so local and CI validation use the same toolchain.

**Boundary:** No persistence, storage abstraction, backend, filesystem, block-device, kernel, driver, or OS-specific integration was introduced. M0.4 backend commit failure semantics remain undecided.

**Status:** Implementation pending CI and second audit.
