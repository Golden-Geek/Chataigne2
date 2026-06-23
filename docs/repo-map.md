# Repo Map

## Top Level

- `src/`: Chataigne2 app shell and app-owned node tree.
- `src-ui/`: Svelte 5 UI package.
- `submodules/golden_core/`: shared engine workspace.
- `capabilities/`: Tauri capability and permission configuration.
- `gen/schemas/`: generated Tauri-related schemas.

## App Shell

- `src/app/mod.rs`: app entry wiring.
- `src/app/bootstrap.rs`: project lifecycle hooks for the app node enum.
- `src/app/default_project.rs`: default Chataigne project content wired by the lifecycle hook.
- `src/module/`: shared module manager, module base nodes, module constants, command managers, and module-family roots.
- `src/module/command/`: reusable module command contracts, command manager base behavior, and command execution event helpers.
- `src/module/permissions.rs`: authoring permission policy for app-owned module nodes.
- `src/module/reference_filters.rs`: module reference-filter registration and candidate predicates.
- `src/module/script_api.rs`: generic module script plumbing, including callback event emission and app-owned template-directory selection. Module-specific callback names and script methods stay with their module families. See [module-scripting.md](module-scripting.md).
- `src/module/script_templates/`: app-owned JavaScript templates and snippets for module scripts.
- `src/module/common/`: reusable module infrastructure shared across module families, such as network-interface discovery/helpers.
- `src/module/modules/`: concrete module families grouped by domain (`generic/`, `hardware/`, `protocol/`, `software/`).
- `src/module/modules/protocol/osc/`: OSC module stack, including `OscModuleBase`, parameter-hosted receive/output nodes, auto-added OSC value trees, and its async `rosc` transport helpers.

## State Machine And Alchemist

- `src/state_machine/`: reusable Chataigne state-machine crate, including
  processor runtime, formula source DTOs, `ValueSet`, managed formula
  orchestration, InputSet/OutputSet materialization, command intents, and
  protocol generation.
- `src/state_machine_nodes/`: app-owned state-machine tree nodes, including the
  Formula Library, Processor Manager, built-in formula catalog/package loader,
  processor managed-region folders, and app-owned manager bridge ANodes.
- `src/state_machine_nodes/builtin_formulas/`: shipped Action and Mapping
  formula package definitions.
- `submodules/golden_alchemist_core/`: reusable Alchemist graph typing,
  compilation, runtime evaluation, primitive ANodes, ConditionGate,
  filter-pipeline capabilities, shape checking, and managed-region metadata.
- `docs/ALCHEMIST_FORMULA_RUNTIME.md`: architecture contract for processor
  formulas, managed regions, lane evaluation, diagnostics, and runtime intents.

## golden_core Workspace

- `crates/core_facade/`: stable `golden_core` facade crate used by apps.
- `crates/core/`: implementation crate (`golden_engine` package).
- `crates/core/src/`: canonical source tree for engine/runtime implementation.
- `crates/core/src/engine/`: engine state, edit application, runtime scheduling, persistence helpers, UI event log helpers, and tests.
- `crates/core/src/node/`: node traits, built-in node families, dashboard nodes, animation-curve nodes, and typed handles.
- `crates/core/src/parameter/`: parameter values, constraints, control state, and the parameter node type.
- `crates/core/src/script/`: script runtime plus checked-in default script templates.
- `crates/core/docs/source_layout.md`: filesystem and module-ownership rules for `golden_engine`.
- `crates/core_macros/`: proc macros.
- `crates/codegen_support/`: build-time node registry generation support.
- `crates/protocol/`: public UI protocol boundary.
- `crates/persistence/`: public persistence boundary.
- `crates/script/`: public scripting boundary.
- `crates/transport_server/`: built-in HTTP and WebSocket host.
- `crates/host_desktop/`: default desktop/headless launch path and native-dialog integration.
- `crates/core/docs/`: deeper engine/runtime design notes.

## UI Package

- `src-ui/src/lib/golden_ui/components/`: panels and reusable UI components.
- `src-ui/src/lib/golden_ui/store/`: workbench and focused state stores.
- `src-ui/src/lib/golden_ui/store/session/`: focused workbench helpers for selection, warnings,
  descriptions, footer hover, history, logger, and command/file orchestration.
- `src-ui/src/lib/golden_ui/transport/`: UI transport clients.
- `src-ui/src/lib/golden_ui/dockview/`: panel registration and layout persistence.
- `src-ui/src/lib/golden_ui/docs/source_layout.md`: canonical UI package layout and boundary rules.
