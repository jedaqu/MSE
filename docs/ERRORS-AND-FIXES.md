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
