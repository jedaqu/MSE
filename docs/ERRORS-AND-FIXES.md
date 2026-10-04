# Errors and fixes

Public technical error record only.

## M0-001

**Problem:** The first checkpoint prototype copied the base together with the change layer.

**Correction:** Checkpoints now retain only the overlay layer. The base is not copied by checkpoint creation.

**Status:** Corrected before integration.

## M0.1-001

**Problem:** The block write API could panic when an index was outside the modeled block range.

**Correction:** Block addresses are typed with `BlockId`, writes are bounds-checked, and out-of-range writes return `WriteError::OutOfRange` instead of panicking.

**Status:** Corrected before integration.

## M0.2-001

**Problem:** `restore()` accepted checkpoints with a different block count, while `diff()` compared vectors with `zip()` and could silently omit unmatched blocks.

**Correction:** Checkpoints retain their block count. `restore()` validates compatibility before changing the overlay and returns `RestoreError` on mismatch. `diff()` returns `Result` and reports the same incompatibility explicitly.

**Status:** Corrected in M0.2.

## M0.2-002

**Problem:** The first formatting check found a Rust formatting difference in the new `diff()` expression.

**Correction:** Applied `cargo fmt` before the repeated quality gate.

**Status:** Corrected before PR validation.

## M1.5-001

**Problem:** The first M1.5 commit omitted the closing brace for the new `ChangeSet` implementation, leaving the Rust source syntactically invalid.

**Detection:** Static pre-PR audit of the exact branch head.

**Correction:** Closed the `ChangeSet` implementation before the `Engine` declaration.

**Status:** Corrected before PR creation.

## M1.6-001

**Problem:** The first M1.6 implementation head failed the formatting gate.

**Correction:** Applied the exact rustfmt changes without semantic modification.

**Status:** Corrected before replacement validation.

## M1.6-002

**Problem:** The first rustfmt correction still left two extra blank lines.

**Correction:** Removed the remaining extra blank lines without semantic modification.

**Status:** Corrected before replacement validation.

## M1.6-003

**Problem:** The rustfmt gate still required one final blank-line correction.

**Correction:** Removed the remaining spacing difference without semantic modification.

**Status:** Corrected before final validation.

## M1.7-001

**Problem:** The first M1.7 PR head failed formatting because the final test-module closing region contained an extra blank line.

**Correction:** Removed the extra blank line without changing semantics or scope.

**Status:** Corrected before final PR validation.

## M1.7-002

**Problem:** The second M1.7 PR head failed formatting because the Rust source was missing its final newline.

**Correction:** Restored the final newline only.

**Status:** Corrected before final PR validation.

## M1.10-001

**Problem:** A M1.10 validation run failed only the formatting gate. Rustfmt required formatting changes in `src/backend.rs` and `src/persistence.rs`.

**Correction:** Applied the exact rustfmt changes while retaining the intended API-only scope.

**Status:** Corrected before final validation.

## M1.11-001

**Problem:** The first M1.11 implementation validation run failed only the formatting gate.

**Correction:** Applied the exact rustfmt corrections.

**Status:** Corrected before final implementation validation.
