# Engineering work method

This repository follows a controlled, traceable, adaptive engineering method.

## Core cycle

1. Establish the real checkpoint.
2. Audit the problem, evidence, scope, dependencies, risks and exit criteria.
3. Implement the minimum sufficient change.
4. Verify the change locally or with the narrowest relevant validation.
5. Perform a second audit of the actual diff.
6. Validate through the appropriate automated checks.
7. Integrate only after the evidence is satisfactory.
8. Validate the integrated state when the risk requires it.
9. Record the result and the remaining boundaries.
10. Re-evaluate before starting the next milestone.

The method scales with risk. Small changes should not acquire unnecessary ceremony; architectural, persistence, recovery, release and security changes require stronger evidence.

## Adaptive execution and waiting

Long-running operations are treated as active states, not as fixed timer loops.

Reference intervals such as 20, 30 or 60 seconds are only examples. Follow-up frequency is chosen from the expected duration, observed progress, risk and likelihood that a new result is available.

While an operation is running:

- avoid redundant status checks;
- continue safe, independent work when useful;
- prepare dependent work without executing it;
- never let parallel work invalidate or confuse the evidence being awaited;
- when a result appears, stop the parallel flow and interpret the result first.

For GitHub Actions, record the run and commit identity, check again when meaningful progress is expected, inspect the result and jobs immediately when complete, then decide the next action from the actual evidence.

A workflow result validates the commit it actually ran against. A later commit requires its own validation.

## Validation tiers

Pull requests carry the change-level validation needed to catch formatting, compilation, test and static-analysis regressions.

Pushes to `main` provide integrated-state validation.

The M0.5 performance baseline is an integration-level check and is therefore run on pushes to `main`, not as a redundant PR gate.

The distinction is intentional:

- PR validation answers: "Is this proposed change acceptable?"
- post-merge validation answers: "Is the integrated main state still valid?"

## Toolchain source of truth

The repository toolchain is defined only by `rust-toolchain.toml`.

CI must consume that project definition rather than repeat the Rust version in the workflow.

## Documentation model

Documentation has one responsibility per document.

Milestones should use one primary record containing:

- checkpoint;
- problem and evidence;
- objective;
- scope and non-scope;
- decisions;
- implementation;
- errors/deviations;
- validation;
- integration;
- post-merge validation;
- limitations;
- closure;
- next entry condition.

Historical milestone documents that already exist may remain as historical records. New milestones should avoid creating multiple documents that repeat the same milestone state.

`ERRORS-AND-FIXES.md` is reserved for actual errors, deviations and corrections. It is not a changelog or milestone ledger.

## Branch hygiene

Branches are working references, not historical archives.

After a change has been integrated and its evidence is closed, obsolete working branches should be removed when they are no longer needed. Commit history remains preserved through the integrated history.

## Public boundary

Only information necessary to understand and use the public repository belongs in public documentation.

Private prompts, conversations, secrets, personal data, private strategy, commercial plans, internal-only names and information from unrelated projects remain outside the public repository.

## Process optimization

The method itself is periodically audited.

The target is not fewer controls. The target is the same required confidence with less duplicated work, fewer redundant checks, fewer repeated explanations, fewer sources of truth and less branch or artifact clutter.
