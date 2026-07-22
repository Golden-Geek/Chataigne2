# Functional Parity Ledger Schema

Schema version: 1

The parity ledger is a versioned, machine-readable inventory of independently observable product
capabilities. A broad row such as “UI” or “modules” is invalid. The generated ledger is not created
by this document; its owning implementation slice must validate it against this contract.

## Discovery And Evidence Ownership

The effective Phase 9 ledger is a validated join of two records with deliberately different
ownership:

- `manifests/functional-parity.v1.json` is deterministic generated discovery. It inventories every
  current capability ID and never turns source presence into behavioral proof.
- `manifests/functional-parity-evidence.v1.json` is authored qualification evidence. An entry is
  accepted only when it implements the complete capability-row contract below and its executable
  and manual results are current.

`python tools/migration/phase9_readiness.py --json` rejects duplicate or stale evidence IDs,
incomplete evidence entries, missing inventory coverage, active temporary adapters, and any final
gate without linked passing evidence. Generated discovery is therefore never overwritten by manual
claims, while authored results cannot silently drift away from the inventory.

Path-derived discovery IDs use their Phase 0 logical paths even after monorepo relocation. Run
`python tools/migration/phase9_identity.py --json` to compare the generated inventory with the
immutable Phase 0 checkpoint. The audit fails if any baseline capability disappears and locks the
exact set of post-baseline capability IDs with a digest recorded in the Phase 9 dashboard.

Phase 9 performance evidence is recorded by dedicated runners rather than copied from console
output. `python tools/migration/run_phase9_scale.py` captures the working-tree identity, toolchain,
exact command, structured metrics, raw log hash, and pass/fail state for the 100,000-scalar gate.

## Capability Row

| Field | Required | Contract |
| --- | --- | --- |
| `capability_id` | Yes | Stable, unique, never reused, and suitable for CI selection |
| `product_area` | Yes | One of `workbench`, `graph`, `formula`, `state_machine`, `module`, `script`, `dashboard`, `spatializer`, `persistence`, `networking`, `host`, or `diagnostics` |
| `classification` | Yes | `operational_baseline`, `baseline_scaffolding`, or `planned_functionality`; only the first may be called restored |
| `title` | Yes | Short user-facing capability name |
| `baseline_source` | Yes | Repository/ref plus all relevant source, asset, formula, fixture, or configuration paths |
| `user_workflow` | Yes | Ordered user actions and expected visible/audible/device feedback |
| `runtime_semantics` | Yes | Inputs, outputs, state, ordering, timing, errors, recovery, and delivery/coalescing rules |
| `final_owner` | Yes | Target crate, package, or app directory |
| `evidence` | Yes | One or more stable executable evidence IDs and their runner/type |
| `last_passing_result` | Conditional | Required before a row can pass; see result fields below |
| `manual_evidence` | Yes | Required scenario and current status; use an explicit empty list only when automation fully covers it |
| `migration_state` | Yes | `baseline`, `adapted`, `shadowing`, `cut_over`, or `old_path_removed` |
| `temporary_adapters` | Yes | List of adapter records below; an empty list is explicit |
| `approval` | Conditional | Explicit product-owner approval for intentional behavior or UX changes |
| `dependencies` | Yes | Capability IDs or external prerequisites needed by the evidence |
| `notes` | No | Clarification only; never substitutes for executable evidence |

## Executable Evidence Entry

Each `evidence` item contains:

| Field | Required | Contract |
| --- | --- | --- |
| `evidence_id` | Yes | Stable runner/scenario ID, not merely a source path |
| `kind` | Yes | Unit, semantic digest, integration, Playwright, screenshot, protocol, simulator, hardware, performance, or manual scenario |
| `command` | Yes | Exact command or documented runner selector |
| `platforms` | Yes | Explicit target OS/hardware matrix |
| `fixtures` | Yes | Exact fixture IDs and hashes where applicable |
| `required` | Yes | Whether failure or absence blocks the capability |

## Result Record

Every attempted command or scenario records:

| Field | Required | Contract |
| --- | --- | --- |
| `status` | Yes | `PASS`, `FAIL`, `NOT_RUN`, or `BLOCKED` |
| `commit_sha` | Yes when committed | Commit under test; pre-commit local runs additionally record `tested_tree_sha` so evidence cannot claim a different tree |
| `tested_tree_sha` | Yes | Exact Git tree containing the tested content; avoids self-reference when evidence is committed later |
| `toolchain_fingerprint` | Yes | Rust, Cargo, target host, Node, package manager, OS, and enabled features |
| `started_at` and `finished_at` | Yes when run | UTC timestamps |
| `exit_code` | Yes when run | Exact process result |
| `ignored_or_skipped` | Yes | Counts and reasons, including platform/feature gates |
| `artifact_id` and `artifact_hash` | Yes when artifacts exist | Immutable CI/local evidence reference and digest |
| `measured_result` | Conditional | Semantic digest, screenshot/trace hash, performance distribution, protocol trace, or manual result |
| `blocking_reason` | Required for `BLOCKED` | Failed prerequisite and its result ID |

`NOT_RUN` is not a pass. A compilation failure is `FAIL`; dependent scenarios are `BLOCKED` rather
than silently omitted.

## Temporary Adapter Record

Every adapter, converter, dual-read path, shadow executor, or development-only feature switch must
have:

| Field | Required | Contract |
| --- | --- | --- |
| `adapter_id` | Yes | Stable identifier used by code, tests, and progress reporting |
| `owner` | Yes | Named team, subsystem owner, or accountable maintainer |
| `scope` | Yes | Exact boundaries translated or shadowed, including which path remains authoritative |
| `introduced_phase` | Yes | Phase and supercommit that introduced it |
| `expiry_phase` | Yes | Latest phase in which it may exist |
| `deletion_criteria` | Yes | Executable parity, soak, and cutover conditions required before removal |
| `deletion_issue` | Yes | Tracked issue/work item; “later” is invalid |
| `tests` | Yes | IDs proving translation fidelity, fallback behavior, and failure handling |
| `side_effect_policy` | Yes | How duplicate outputs, triggers, commands, effects, and device traffic are prevented |
| `current_state` | Yes | `planned`, `active`, `shadowing`, `deletion_ready`, or `removed` |
| `removed_in` | Required when removed | Exact commit that deleted the adapter and old path |

Adapters without an owner, scope, expiry phase, deletion criteria, deletion issue, or tests block a
phase close.

## Example Shape

```yaml
schema_version: 1
capabilities:
  - capability_id: graph.connection.create
    product_area: graph
    classification: operational_baseline
    title: Create a graph connection
    baseline_source:
      repository_ref: f0a7c2fe4d192a076c7649c6fe3e6a5ab193a435
      paths: []
    user_workflow:
      steps: []
      expected_feedback: []
    runtime_semantics:
      inputs: []
      outputs: []
      state: []
      ordering: ""
      timing: ""
      errors: []
      recovery: []
    final_owner: crates/golden-graph
    evidence: []
    last_passing_result: null
    manual_evidence: []
    migration_state: baseline
    temporary_adapters: []
    approval: null
    dependencies: []
```

The empty example lists are schema illustration only and are not a real capability row or evidence.

## Staleness and Completion

A passing result becomes stale when the capability implementation, transitive runtime path, UI
workflow, fixture, toolchain, generated protocol, or evidence test changes. CI must re-execute stale
evidence.

A capability is complete only when:

- every required evidence ID has a current `PASS` for its declared matrix;
- required manual evidence is signed and linked;
- any intentional behavior/UX change has explicit approval;
- its migration state and adapter records match reality;
- catalog or manifest presence is backed by the actual user/runtime workflow;
- no unknown or untested required behavior remains.
