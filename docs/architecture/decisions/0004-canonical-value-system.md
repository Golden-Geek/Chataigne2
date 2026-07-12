# ADR 0004: Use One Canonical Value and Conversion System

- Status: Accepted
- Date: 2026-07-11

## Context

Parallel parameter, runtime, Alchemist, protocol, script, persistence, context, and module value
enums create conversion drift and make semantic equality, triggers, and extension validation
boundary-dependent.

## Decision

`golden-values` owns the canonical `Value`, `ValueTypeId`, type descriptors, typed projections,
component paths, conversion rules, equality/change semantics, `ValueSet`, lane keys, trigger-edge
IDs, stable references, compact runtime storage descriptors, and protocol-safe validation.

The value catalog covers the complete working-product surface, including scalar, string/file/enum,
CSS, vectors, color, duration, trigger, array, reference, and extension values. New types extend the
canonical system rather than create parallel enums.

`golden-parameters` layers constraints, control modes, contexts, templates, expressions/scripts,
automation metadata, coalescing behavior, UI hints, and declarations on this value system.
Parameters compile to runtime input routes instead of being interpreted through tree traversal each
tick.

## Consequences

- Parameters, contexts, Alchemist, conditions, module IO, scripts, persistence, and protocol share
  one conversion implementation.
- Authored representations may map into compact runtime slots, but they do not define a second
  semantic value universe.
- Trigger edge identity, conversion failures, numeric finiteness, payload limits, and equality are
  consistent across all callers.

## Compliance

Phase 3 cuts values over as a vertical slice covering parameter controls, module values, scripts,
persistence, protocol, and UI. The old value paths remain until semantic digests, converted
fixtures, and real UI/module workflows prove parity.
