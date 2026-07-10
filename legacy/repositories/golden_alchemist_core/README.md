# golden_alchemist_core

Reusable typed graph and statechart engines for Golden applications.

This repository owns only app-agnostic mechanics:

- `golden_alchemist`: reusable Formula models and surfaces, typed graph
  declarations, compilation, diagnostics, and runtime evaluation.
- `golden_statechart`: hierarchical statechart structure and transition semantics.

Product value types, nodes, processor policy, persistence, and host integration remain in the consuming application.
