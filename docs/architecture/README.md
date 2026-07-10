# Architecture contract

The canonical target is [Golden Architecture Final Plan](../Golden_Architecture_Final_Plan.md).
This directory holds the enforceable decisions and evidence used to execute that plan.

Phase 0 owns three artifacts:

- `decisions/`: accepted boundaries that later implementation phases must not weaken;
- `dependency-rules.v1.json`: machine-readable allowed dependency direction;
- `functional-parity.v1.json`: the capability inventory and characterization status.

An architecture phase is complete only when its exit criteria, scoped checks, evidence,
and progress note are committed together in one focused supercommit. A `pending` entry is
an explicit gap, not implicit coverage.
