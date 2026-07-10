# Architecture

The final architecture has one implementation path per responsibility. The root
[repository map](../../README.md) is the quickest orientation; the canonical design and
acceptance criteria remain in [Golden Architecture Final Plan](../Golden_Architecture_Final_Plan.md).

## Dependency direction

```text
apps/chataigne
  -> host / transport / persistence / IO / script
  -> runtime / processor / condition / statechart / alchemist
  -> graph / context / parameters
  -> values / model

packages/golden-* -> generated protocol and transport interfaces
```

The exact allowed Rust dependencies and forbidden UI imports are enforced from
`dependency-rules.v1.json` by `tools/check_workspace_architecture.py`.

## Runtime planes

- Authoring/control owns project mutations and compilation requests.
- Semantic execution owns immutable runtime generations, direct slots, deterministic
  scheduling, and ordered effects.
- IO and transport own parsing, timestamps, recovery, bounded queues, and client isolation.
- Observation owns keyed, interest-scoped previews; the UI stages them and commits once per
  animation frame.
- Persistence serializes immutable snapshots and never hides inside host bootstrap.

## Evidence and decisions

- `functional-parity.v1.json` maps every preserved capability to its final owner and test.
- `benchmarks/phaseN/` records the qualification evidence delivered by each phase.
- `decisions/` contains the accepted monorepo, graph, runtime-plane, value, and clean-break
  decisions.
- `phase7-product-port.md`, `phase8-persistence-recovery.md`, and
  `phase9-scale-and-deletion.md` describe the final product-facing phases.
- `archive.md` records where deleted pre-monorepo implementations remain available in Git
  history.
