# Phase 7: Chataigne product port

Phase 7 moves product policy into `apps/chataigne` while adding only reusable lifecycle,
scripting, host, and UI registration contracts to Golden packages.

- `golden-io` owns bounded endpoint ingress and event-driven recovery.
- `golden-script` owns validated, language-neutral script surface declarations.
- `golden-host` owns the common desktop/headless launch contract.
- `golden-transport` supports explicit loopback, open-LAN, and authenticated modes with
  bounded clients/payloads plus origin and Host validation.
- `apps/chataigne/backend` owns its complete module catalog, commands, script surfaces,
  dashboards, compiled Spatializer projection, and host policy.
- `apps/chataigne/ui` registers product panels through the public `golden-ui` registry.

Machine-readable evidence is in `benchmarks/phase7/chataigne-product.v1.json`.
